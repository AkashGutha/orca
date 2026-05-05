use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time;
use tracing::{debug, info, warn};

use crate::model::{CommandOutcome, CommandResult, CommandSpec};
use crate::output::{AgentStatus, OutputStream};

#[derive(Debug, Clone, Default)]
pub struct CommandExecutor;

impl CommandExecutor {
    pub async fn execute(&self, spec: CommandSpec) -> CommandResult {
        let max_attempts = spec.retries.saturating_add(1);
        let mut last = None;

        for attempt in 1..=max_attempts {
            if spec
                .output
                .as_ref()
                .is_some_and(|output| output.stop_requested())
            {
                return cancelled_result(&spec, attempt, String::new(), String::new(), None);
            }

            info!(command_id = %spec.id, attempt, "starting command");
            if let Some(output) = &spec.output {
                output.agent_started(
                    &spec.id,
                    spec.output_label.as_deref().unwrap_or("command"),
                    spec.output_phase.as_deref().unwrap_or("run"),
                    spec.output_model.as_deref(),
                );
            }
            let result = run_once(&spec, attempt).await;
            if let Some(output) = &spec.output {
                output.agent_finished(
                    &spec.id,
                    if result.succeeded() {
                        AgentStatus::Succeeded
                    } else {
                        AgentStatus::Failed
                    },
                );
            }
            if result.succeeded() {
                return result;
            }

            if spec
                .output
                .as_ref()
                .is_some_and(|output| output.stop_requested())
            {
                return result;
            }

            let should_retry = attempt < max_attempts;
            if should_retry {
                warn!(
                    command_id = %spec.id,
                    attempt,
                    outcome = ?result.outcome,
                    "command attempt failed; retrying"
                );
                if let Some(delay_ms) = spec.retry_delay_ms {
                    time::sleep(Duration::from_millis(delay_ms)).await;
                }
            }
            last = Some(result);
        }

        last.expect("at least one command attempt must run")
    }
}

async fn run_once(spec: &CommandSpec, attempt: u32) -> CommandResult {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args);
    command.envs(&spec.env);
    if let Some(cwd) = &spec.cwd {
        command.current_dir(cwd);
    }
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.kill_on_drop(true);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            return CommandResult {
                id: spec.id.clone(),
                outcome: CommandOutcome::SpawnFailed,
                exit_code: None,
                attempts: attempt,
                stdout: String::new(),
                stderr: String::new(),
                error: Some(err.to_string()),
            };
        }
    };

    let stdout = child.stdout.take().map(|stream| {
        tokio::spawn(read_stream(
            spec.id.clone(),
            "stdout",
            spec.output.clone(),
            OutputStream::Stdout,
            BufReader::new(stream),
        ))
    });
    let stderr = child.stderr.take().map(|stream| {
        tokio::spawn(read_stream(
            spec.id.clone(),
            "stderr",
            spec.output.clone(),
            OutputStream::Stderr,
            BufReader::new(stream),
        ))
    });

    let wait_result = wait_for_child(spec, &mut child).await;

    let stdout = join_output(stdout).await;
    let stderr = join_output(stderr).await;

    match wait_result {
        Ok(status) if status.success() => CommandResult {
            id: spec.id.clone(),
            outcome: CommandOutcome::Success,
            exit_code: status.code(),
            attempts: attempt,
            stdout,
            stderr,
            error: None,
        },
        Ok(status) => CommandResult {
            id: spec.id.clone(),
            outcome: CommandOutcome::Failed,
            exit_code: status.code(),
            attempts: attempt,
            stdout,
            stderr,
            error: Some(format!("process exited with status {status}")),
        },
        Err(WaitError::Io(err)) => CommandResult {
            id: spec.id.clone(),
            outcome: CommandOutcome::Failed,
            exit_code: None,
            attempts: attempt,
            stdout,
            stderr,
            error: Some(err.to_string()),
        },
        Err(WaitError::TimedOut(error)) => CommandResult {
            id: spec.id.clone(),
            outcome: CommandOutcome::TimedOut,
            exit_code: None,
            attempts: attempt,
            stdout,
            stderr,
            error: Some(error),
        },
        Err(WaitError::Cancelled(kill_error)) => {
            cancelled_result(spec, attempt, stdout, stderr, kill_error)
        }
    }
}

fn cancelled_result(
    spec: &CommandSpec,
    attempt: u32,
    stdout: String,
    stderr: String,
    kill_error: Option<String>,
) -> CommandResult {
    CommandResult {
        id: spec.id.clone(),
        outcome: CommandOutcome::Cancelled,
        exit_code: None,
        attempts: attempt,
        stdout,
        stderr,
        error: Some(kill_error.unwrap_or_else(|| "cancelled by user request".to_string())),
    }
}

async fn wait_for_child(
    spec: &CommandSpec,
    child: &mut Child,
) -> Result<std::process::ExitStatus, WaitError> {
    let timeout = spec.timeout;
    let timeout_sleep = async {
        if let Some(timeout) = timeout {
            time::sleep(timeout).await;
        } else {
            std::future::pending::<()>().await;
        }
    };
    tokio::pin!(timeout_sleep);

    loop {
        tokio::select! {
            result = child.wait() => {
                return result.map_err(WaitError::Io);
            }
            _ = &mut timeout_sleep => {
                let kill_error = child.kill().await.err().map(|err| err.to_string());
                let _ = child.wait().await;
                return Err(WaitError::TimedOut(kill_error.unwrap_or_else(|| {
                    format!(
                        "timed out after {} seconds",
                        spec.timeout_secs.unwrap_or_default()
                    )
                })));
            }
            _ = time::sleep(Duration::from_millis(100)) => {
                if spec
                    .output
                    .as_ref()
                    .is_some_and(|output| output.stop_requested())
                {
                    let kill_error = child.kill().await.err().map(|err| err.to_string());
                    let _ = child.wait().await;
                    return Err(WaitError::Cancelled(kill_error));
                }
            }
        }
    }
}

enum WaitError {
    Io(std::io::Error),
    TimedOut(String),
    Cancelled(Option<String>),
}

async fn read_stream<R>(
    command_id: String,
    stream_name: &'static str,
    output: Option<crate::output::OutputHandle>,
    output_stream: OutputStream,
    reader: BufReader<R>,
) -> String
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut lines = reader.lines();
    let mut captured = String::new();

    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                debug!(%command_id, stream = stream_name, line = %line);
                if let Some(output) = &output {
                    output.line(&command_id, output_stream, &line);
                }
                captured.push_str(&line);
                captured.push('\n');
            }
            Ok(None) => return captured,
            Err(err) => {
                debug!(%command_id, stream = stream_name, error = %err, "failed to read stream");
                return captured;
            }
        }
    }
}

async fn join_output(handle: Option<tokio::task::JoinHandle<String>>) -> String {
    match handle {
        Some(handle) => handle.await.unwrap_or_default(),
        None => String::new(),
    }
}
