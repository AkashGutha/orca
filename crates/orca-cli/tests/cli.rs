use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn shows_welcome_banner_without_args() {
    let mut cmd = Command::cargo_bin("orca").unwrap();
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("ORCA"))
        .stdout(predicate::str::contains("Set a goal to begin"))
        .stdout(predicate::str::contains("orca --goal"))
        .stdout(predicate::str::contains("max_iterations"));
}

#[test]
fn validates_config() {
    let dir = tempdir().unwrap();
    let config = dir.path().join("orca.toml");
    std::fs::write(
        &config,
        r#"
max_concurrency = 2

[[commands]]
id = "hello"
program = "echo"
args = ["hello"]
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("orca").unwrap();
    cmd.arg("validate-config").arg("--config").arg(config);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("config is valid"));
}

#[test]
fn validates_config_with_config_file_alias() {
    let dir = tempdir().unwrap();
    let config = dir.path().join("custom.yaml");
    std::fs::write(
        &config,
        r#"
commands:
  - id: hello
    program: echo
    args: ["hello"]
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("orca").unwrap();
    cmd.arg("validate-config").arg("--config-file").arg(config);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("config is valid"));
}

#[test]
fn run_discovers_yaml_config_in_default_config_folder() {
    let dir = tempdir().unwrap();
    let config_dir = dir.path().join("config");
    std::fs::create_dir(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("orca.yaml"),
        r#"
commands:
  - id: hello
    program: echo
    args: ["hello"]
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("orca").unwrap();
    cmd.current_dir(dir.path()).arg("run");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("exit_code=0"));
}

#[test]
fn runs_dependent_commands() {
    let dir = tempdir().unwrap();
    let config = dir.path().join("orca.toml");
    std::fs::write(
        &config,
        r#"
max_concurrency = 2

[[commands]]
id = "first"
program = "echo"
args = ["first"]

[[commands]]
id = "second"
program = "echo"
args = ["second"]
depends_on = ["first"]
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("orca").unwrap();
    cmd.arg("run").arg("--config").arg(config);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("exit_code=0"));
}

#[test]
fn emits_json_summary() {
    let dir = tempdir().unwrap();
    let config = dir.path().join("orca.toml");
    std::fs::write(
        &config,
        r#"
[[commands]]
id = "hello"
program = "echo"
args = ["hello"]
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("orca").unwrap();
    cmd.arg("run").arg("--config").arg(config).arg("--json");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"outcome\": \"success\""));
}

#[test]
fn times_out_commands() {
    let dir = tempdir().unwrap();
    let config = dir.path().join("orca.toml");
    std::fs::write(
        &config,
        r#"
[[commands]]
id = "slow"
program = "sleep"
args = ["2"]
timeout_secs = 1
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("orca").unwrap();
    cmd.arg("run").arg("--config").arg(config);
    cmd.assert()
        .failure()
        .stdout(predicate::str::contains("TimedOut"));
}

#[test]
fn cancels_on_first_failure_by_default() {
    let dir = tempdir().unwrap();
    let config = dir.path().join("orca.toml");
    std::fs::write(
        &config,
        r#"
max_concurrency = 2

[[commands]]
id = "fail"
program = "false"

[[commands]]
id = "slow"
program = "sleep"
args = ["5"]
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("orca").unwrap();
    cmd.arg("run").arg("--config").arg(config);
    cmd.assert()
        .failure()
        .stdout(predicate::str::contains("Cancelled"));
}
