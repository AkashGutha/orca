use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn runs_role_free_goal_graph() {
    let dir = tempdir().unwrap();
    let config = dir.path().join("orca.toml");
    let artifacts = dir.path().join("runs");
    write_basic_config(&config, &artifacts, "auto");

    let mut cmd = Command::cargo_bin("orca").unwrap();
    cmd.arg("goal")
        .arg("--goal")
        .arg("ship a feature")
        .arg("--config")
        .arg(&config)
        .arg("--json");

    let output = cmd
        .assert()
        .success()
        .stdout(predicate::str::contains("\"completed\": true"))
        .stdout(predicate::str::contains("\"role\"").not())
        .stdout(predicate::str::contains("\"phase\"").not())
        .stdout(predicate::str::contains(
            "\"output_contract\": \"coverage\"",
        ))
        .get_output()
        .stdout
        .clone();

    let summary: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let outputs = summary["outputs"].as_array().unwrap();
    assert!(outputs.iter().any(|output| {
        output["agent_id"] == "planner"
            && output["content"]
                .as_str()
                .unwrap()
                .contains("feature context")
    }));
    assert!(outputs.iter().any(|output| {
        output["agent_id"] == "test-agent"
            && output["content"].as_str().unwrap().contains("golden plan")
    }));

    let run_dir = only_run_dir(&artifacts);
    assert!(
        run_dir
            .join("iteration-1/feature/feature-generator.md")
            .is_file()
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(run_dir.join("manifest.json")).unwrap()).unwrap();
    assert!(
        manifest["outputs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|output| {
                output["agent_id"] == "coverage-agent" && output["output_contract"] == "coverage"
            })
    );
}

#[test]
fn manual_approval_stops_after_golden_plan() {
    let dir = tempdir().unwrap();
    let config = dir.path().join("orca.toml");
    let artifacts = dir.path().join("runs");
    write_basic_config(&config, &artifacts, "manual");

    let mut cmd = Command::cargo_bin("orca").unwrap();
    cmd.arg("goal")
        .arg("--goal")
        .arg("ship a feature")
        .arg("--config")
        .arg(&config);

    cmd.assert()
        .failure()
        .stdout(predicate::str::contains("approved: false"))
        .stdout(predicate::str::contains("orchestrator"))
        .stdout(predicate::str::contains("work-agent").not());
}

#[test]
fn connections_schedule_without_injecting_context() {
    let dir = tempdir().unwrap();
    let config = dir.path().join("orca.toml");
    let artifacts = dir.path().join("runs");
    std::fs::write(
        &config,
        format!(
            r#"
[orchestration]
artifact_dir = "{}"
max_parallel_agents = 1
approval_mode = "auto"
max_iterations = 1

[[orchestration.nodes]]
id = "producer"
output_contract = "feature"
instruction = "feature_generation.md"
command = "echo"
args = ["PRODUCER_OUTPUT"]

[[orchestration.nodes]]
id = "consumer"
output_contract = "test"
instruction = "test_generation.md"
command = "echo"
args = ["PASS consumer context={{context}}"]

[[orchestration.nodes]]
id = "coverage"
output_contract = "coverage"
instruction = "feature_coverage.md"
command = "echo"
args = ["{{\"signed_off\":true,\"feedback\":\"\"}}"]
inputs = [{{ source = "consumer" }}]

[[orchestration.connections]]
from = "producer"
to = ["consumer"]

[[orchestration.connections]]
from = "consumer"
to = ["coverage"]
"#,
            artifacts.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("orca").unwrap();
    cmd.arg("goal")
        .arg("--goal")
        .arg("prove edge semantics")
        .arg("--config")
        .arg(&config)
        .arg("--json");

    let output = cmd.assert().success().get_output().stdout.clone();
    let summary: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let consumer = outputs(&summary, "consumer");
    assert!(!consumer.contains("PRODUCER_OUTPUT"));
}

#[test]
fn far_node_input_does_not_require_direct_connection() {
    let dir = tempdir().unwrap();
    let config = dir.path().join("orca.toml");
    let artifacts = dir.path().join("runs");
    std::fs::write(
        &config,
        format!(
            r#"
[orchestration]
artifact_dir = "{}"
max_parallel_agents = 1
approval_mode = "auto"
max_iterations = 1

[[orchestration.nodes]]
id = "far-source"
output_contract = "feature"
instruction = "feature_generation.md"
command = "echo"
args = ["FAR_SOURCE_OUTPUT"]

[[orchestration.nodes]]
id = "scheduler-only"
output_contract = "generic"
instruction = "planning.md"
command = "echo"
args = ["SCHEDULER_ONLY_OUTPUT"]

[[orchestration.nodes]]
id = "consumer"
output_contract = "test"
instruction = "test_generation.md"
command = "echo"
args = ["PASS consumer context={{context}}"]
inputs = [{{ source = "far-source" }}]
depends_on = ["scheduler-only"]

[[orchestration.nodes]]
id = "coverage"
output_contract = "coverage"
instruction = "feature_coverage.md"
command = "echo"
args = ["{{\"signed_off\":true,\"feedback\":\"\"}}"]
inputs = [{{ source = "consumer" }}]

[[orchestration.connections]]
from = "consumer"
to = ["coverage"]
"#,
            artifacts.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("orca").unwrap();
    cmd.arg("goal")
        .arg("--goal")
        .arg("prove far input")
        .arg("--config")
        .arg(&config)
        .arg("--json");

    let output = cmd.assert().success().get_output().stdout.clone();
    let summary: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let consumer = outputs(&summary, "consumer");
    assert!(consumer.contains("FAR_SOURCE_OUTPUT"));
    assert!(!consumer.contains("SCHEDULER_ONLY_OUTPUT"));
}

#[test]
fn retries_with_feedback_when_coverage_fails_first_iteration() {
    let dir = tempdir().unwrap();
    let config = dir.path().join("orca.toml");
    let artifacts = dir.path().join("runs");
    let marker = dir.path().join("coverage-seen");
    write_retry_config(&config, &artifacts, &marker);

    let mut cmd = Command::cargo_bin("orca").unwrap();
    cmd.arg("goal")
        .arg("--goal")
        .arg("ship with retry")
        .arg("--config")
        .arg(&config)
        .arg("--json");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"completed\": true"))
        .stdout(predicate::str::contains("\"iterations\": 2"))
        .stdout(predicate::str::contains(
            "\"output_contract\": \"feedback\"",
        ));

    let run_dir = only_run_dir(&artifacts);
    assert!(
        run_dir
            .join("iteration-1/feedback/feedback-agent.md")
            .is_file()
    );
    let events = std::fs::read_to_string(run_dir.join("events.jsonl")).unwrap();
    assert!(events.contains(r#""iteration":2"#));
    assert!(events.contains("feedback says fix missing feature"));
}

#[test]
fn rtl_design_runs_before_uvm_when_configured_as_dependency() {
    let dir = tempdir().unwrap();
    let config = dir.path().join("orca.toml");
    let artifacts = dir.path().join("runs");
    std::fs::write(
        &config,
        format!(
            r#"
[orchestration]
artifact_dir = "{}"
max_parallel_agents = 3
approval_mode = "auto"
max_iterations = 1

[[orchestration.nodes]]
id = "golden-plan-generator"
output_contract = "golden_plan"
instruction = "orchestration.md"
command = "echo"
args = ["golden"]

[[orchestration.nodes]]
id = "rtl-design-agent"
output_contract = "implementation"
instruction = "work.md"
command = "echo"
args = ["RTL_DONE"]
inputs = [{{ source = "golden-plan-generator" }}]

[[orchestration.nodes]]
id = "uvm-test-bench-agent"
output_contract = "test"
instruction = "test_generation.md"
command = "echo"
args = ["PASS UVM saw {{context}}"]
inputs = [{{ source = "golden-plan-generator" }}, {{ source = "rtl-design-agent" }}]

[[orchestration.nodes]]
id = "coverage-agent"
output_contract = "coverage"
instruction = "feature_coverage.md"
command = "echo"
args = ["{{\"signed_off\":true,\"feedback\":\"\"}}"]
inputs = [{{ source = "uvm-test-bench-agent" }}]

[[orchestration.connections]]
from = "golden-plan-generator"
to = ["rtl-design-agent"]

[[orchestration.connections]]
from = "rtl-design-agent"
to = ["uvm-test-bench-agent"]

[[orchestration.connections]]
from = "uvm-test-bench-agent"
to = ["coverage-agent"]
"#,
            artifacts.display()
        ),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("orca").unwrap();
    cmd.arg("goal")
        .arg("--goal")
        .arg("build rtl")
        .arg("--config")
        .arg(&config)
        .arg("--json");

    let output = cmd.assert().success().get_output().stdout.clone();
    let summary: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let output_items = summary["outputs"].as_array().unwrap();
    let rtl = output_items
        .iter()
        .position(|output| output["agent_id"] == "rtl-design-agent")
        .unwrap();
    let uvm = output_items
        .iter()
        .position(|output| output["agent_id"] == "uvm-test-bench-agent")
        .unwrap();
    assert!(rtl < uvm);
    assert!(outputs(&summary, "uvm-test-bench-agent").contains("RTL_DONE"));
}

#[test]
fn role_field_is_rejected() {
    let dir = tempdir().unwrap();
    let config = dir.path().join("orca.toml");
    std::fs::write(
        &config,
        r#"
[orchestration]
max_iterations = 1

[[orchestration.nodes]]
id = "legacy"
role = "planning"
instruction = "planning.md"
command = "echo"
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("orca").unwrap();
    cmd.arg("goal")
        .arg("--goal")
        .arg("reject role")
        .arg("--config")
        .arg(&config);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("unknown field `role`"));
}

fn write_basic_config(path: &std::path::Path, artifacts: &std::path::Path, approval_mode: &str) {
    std::fs::write(
        path,
        format!(
            r#"
[orchestration]
artifact_dir = "{}"
max_parallel_agents = 3
approval_mode = "{}"
max_iterations = 2

[[orchestration.nodes]]
id = "feature-generator"
output_contract = "feature"
instruction = "feature_generation.md"
command = "echo"
args = ["feature context for {{goal}} with {{context}}"]
inputs = [{{ source = "feedback" }}]

[[orchestration.nodes]]
id = "planner"
output_contract = "plan"
instruction = "planning.md"
command = "echo"
args = ["plan for {{goal}} with {{context}}"]
inputs = [{{ source = "feature-generator" }}]

[[orchestration.nodes]]
id = "orchestrator"
output_contract = "golden_plan"
artifact_dir = "golden-plan"
instruction = "orchestration.md"
command = "echo"
args = ["golden plan from {{context}}"]
inputs = [{{ source = "planner" }}]

[[orchestration.nodes]]
id = "work-agent"
output_contract = "implementation"
artifact_dir = "work"
instruction = "work.md"
command = "echo"
args = ["work from {{context}}"]
inputs = [{{ source = "orchestrator" }}]

[[orchestration.nodes]]
id = "test-agent"
output_contract = "test"
artifact_dir = "tests"
instruction = "test_generation.md"
command = "echo"
args = ["PASS tests from {{context}}"]
inputs = [{{ source = "orchestrator" }}]

[[orchestration.nodes]]
id = "coverage-agent"
output_contract = "coverage"
artifact_dir = "coverage"
instruction = "feature_coverage.md"
command = "echo"
args = ["{{\"signed_off\":true,\"feedback\":\"\"}}"]
inputs = [{{ source = "work-agent" }}, {{ source = "test-agent" }}]

[[orchestration.connections]]
from = "feature-generator"
to = ["planner"]

[[orchestration.connections]]
from = "planner"
to = ["orchestrator"]

[[orchestration.connections]]
from = "orchestrator"
to = ["work-agent", "test-agent"]

[[orchestration.connections]]
from = "work-agent"
to = ["coverage-agent"]

[[orchestration.connections]]
from = "test-agent"
to = ["coverage-agent"]
"#,
            artifacts.display(),
            approval_mode
        ),
    )
    .unwrap();
}

fn write_retry_config(
    path: &std::path::Path,
    artifacts: &std::path::Path,
    marker: &std::path::Path,
) {
    write_basic_config(path, artifacts, "auto");
    let mut content = std::fs::read_to_string(path).unwrap();
    content = content.replace(
        r#"args = ["{\"signed_off\":true,\"feedback\":\"\"}"]"#,
        &format!(
            r#"args = [
  "-c",
  "if [ -f \"$1\" ]; then echo '{{\"signed_off\":true,\"feedback\":\"\"}}'; else touch \"$1\" && printf '{{\"signed_off\":false,\"feedback\":\"missing feature\"}}\n'; fi",
  "coverage-agent",
  "{}",
]"#,
            marker.display()
        ),
    );
    content = content.replace(
        r#"command = "echo"
args = [
  "-c","#,
        r#"command = "sh"
args = [
  "-c","#,
    );
    content.push_str(
        r#"

[[orchestration.nodes]]
id = "feedback-agent"
output_contract = "feedback"
artifact_dir = "feedback"
instruction = "feedback.md"
command = "echo"
args = ["feedback says fix missing feature from {context}"]
inputs = [{ source = "gate-feedback" }]
"#,
    );
    std::fs::write(path, content).unwrap();
}

fn outputs(summary: &serde_json::Value, id: &str) -> String {
    summary["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|output| output["agent_id"] == id)
        .unwrap()["content"]
        .as_str()
        .unwrap()
        .to_string()
}

fn only_run_dir(artifacts: &std::path::Path) -> std::path::PathBuf {
    let runs = std::fs::read_dir(artifacts)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(runs.len(), 1);
    runs.into_iter().next().unwrap()
}
