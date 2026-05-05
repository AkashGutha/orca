use std::path::PathBuf;

use crate::agent::AgentSpec;
use crate::errors::AppError;

pub fn resolve_instruction(
    spec: &AgentSpec,
    override_dirs: &[PathBuf],
) -> Result<String, AppError> {
    let name = spec.instruction.as_deref().ok_or_else(|| {
        AppError::InvalidConfig(format!("agent node `{}` must set instruction", spec.id))
    })?;

    for dir in override_dirs {
        let path = dir.join(name);
        if path.is_file() {
            return std::fs::read_to_string(&path)
                .map_err(|source| AppError::ReadInstruction { path, source });
        }
    }

    embedded_instruction(name).map(str::to_string)
}

fn embedded_instruction(name: &str) -> Result<&'static str, AppError> {
    match name {
        "orchestration.md" => Ok(include_str!("instructions/orchestration.md")),
        "feature_generation.md" => Ok(include_str!("instructions/feature_generation.md")),
        "planning.md" => Ok(include_str!("instructions/planning.md")),
        "critique.md" => Ok(include_str!("instructions/critique.md")),
        "kpi_generation.md" => Ok(include_str!("instructions/kpi_generation.md")),
        "kpi_measurement.md" => Ok(include_str!("instructions/kpi_measurement.md")),
        "test_planning.md" => Ok(include_str!("instructions/test_planning.md")),
        "test_generation.md" => Ok(include_str!("instructions/test_generation.md")),
        "parallel_run.md" => Ok(include_str!("instructions/parallel_run.md")),
        "work.md" => Ok(include_str!("instructions/work.md")),
        "feature_coverage.md" => Ok(include_str!("instructions/feature_coverage.md")),
        "feedback.md" => Ok(include_str!("instructions/feedback.md")),
        "copilot_cli_helper.md" => Ok(include_str!("instructions/copilot_cli_helper.md")),
        other => Err(AppError::InvalidConfig(format!(
            "instruction `{other}` was not found in the configured instruction_dir or embedded instructions"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CRITICAL_COPILOT_CLI_FRAGMENTS: &[&str] = &[
        "copilot --silent --no-ask-user -p",
        "--silent",
        "--no-ask-user",
        "-p",
        "--prompt",
        "positional",
        "Invalid command format",
        "--allow-tool='write'",
        "--allow-tool='shell(cargo test:*)'",
        "--allow-tool='shell(git:*)'",
        "--deny-tool='shell(git push)'",
        "--add-dir",
        "--allow-url",
        "--allow-all-tools",
        "--allow-all",
        "--yolo",
        "--allow-all-paths",
        "--allow-all-urls",
        "--model",
        "--reasoning-effort",
        "--enable-reasoning-summaries",
        "--output-format",
        "--name",
        "--resume",
        "--continue",
        "trusted-workflow-only",
        "risk",
    ];

    fn assert_contains_all(content: &str, fragments: &[&str]) {
        for fragment in fragments {
            assert!(
                content.contains(fragment),
                "expected content to contain fragment: {fragment}"
            );
        }
    }

    fn copilot_cli_helper_spec() -> AgentSpec {
        AgentSpec {
            id: "copilot-cli-helper".to_string(),
            kind: crate::agent::NodeKind::Agent,
            output_contract: "helper".to_string(),
            artifact_dir: None,
            phase_label: None,
            instruction: Some("copilot_cli_helper.md".to_string()),
            command: "copilot".to_string(),
            args: Vec::new(),
            input_sources: Vec::new(),
            depends_on: Vec::new(),
            resources: Vec::new(),
            timeout_secs: None,
            retries: 0,
            retry_delay_ms: None,
        }
    }

    fn feature_generation_spec() -> AgentSpec {
        AgentSpec {
            id: "feature-generator".to_string(),
            kind: crate::agent::NodeKind::Agent,
            output_contract: "feature".to_string(),
            artifact_dir: None,
            phase_label: None,
            instruction: Some("feature_generation.md".to_string()),
            command: "copilot".to_string(),
            args: Vec::new(),
            input_sources: Vec::new(),
            depends_on: Vec::new(),
            resources: Vec::new(),
            timeout_secs: None,
            retries: 0,
            retry_delay_ms: None,
        }
    }

    fn assert_command_orders_flag_before_prompt(content: &str, flag: &str) {
        let command = content
            .lines()
            .find(|line| line.contains("copilot --silent --no-ask-user") && line.contains(flag))
            .unwrap_or_else(|| panic!("expected a scoped command containing {flag}"));
        let flag_index = command
            .find(flag)
            .unwrap_or_else(|| panic!("expected command to contain flag {flag}: {command}"));
        let prompt_index = command
            .find(" -p ")
            .unwrap_or_else(|| panic!("expected command to contain -p prompt flag: {command}"));

        assert!(
            flag_index < prompt_index,
            "expected permission flag to appear before -p in command: {command}"
        );
    }

    fn assert_trusted_only_warning_for_broad_access(content: &str) {
        for flag in [
            "--allow-all-tools",
            "--allow-all",
            "--yolo",
            "--allow-all-paths",
            "--allow-all-urls",
        ] {
            let flag_index = content
                .find(flag)
                .unwrap_or_else(|| panic!("expected broad access flag to be documented: {flag}"));
            let warning_start = flag_index.saturating_sub(300);
            let warning_end = (flag_index + 500).min(content.len());
            let nearby_warning = &content[warning_start..warning_end];

            assert!(
                nearby_warning.contains("trusted")
                    && (nearby_warning.contains("risk")
                        || nearby_warning.contains("explicitly accepted")
                        || nearby_warning.contains("not the safest general recommendation")
                        || nearby_warning.contains("only")
                        || nearby_warning.contains("required")),
                "expected broad access flag to be documented as exceptional/trusted-only: {flag}"
            );
        }
    }

    #[test]
    fn resolved_copilot_cli_helper_instruction_contains_safe_invocation_guidance() {
        let instruction = resolve_instruction(&copilot_cli_helper_spec(), &[]).unwrap();

        assert_contains_all(&instruction, CRITICAL_COPILOT_CLI_FRAGMENTS);
    }

    #[test]
    fn resolved_feature_generation_instruction_defines_pre_planning_analysis() {
        let instruction = resolve_instruction(&feature_generation_spec(), &[]).unwrap();

        assert_contains_all(
            &instruction,
            &[
                "pre-planning analysis agent",
                "Feature summary",
                "Concrete requirements",
                "Acceptance criteria",
                "Assumptions and constraints",
                "Edge cases",
                "Out-of-scope items",
                "Risks and unknowns",
                "Do not implement code",
            ],
        );
    }

    #[test]
    fn embedded_copilot_cli_helper_is_self_contained_not_a_skill_pointer() {
        let instruction = resolve_instruction(&copilot_cli_helper_spec(), &[]).unwrap();

        assert!(
            !instruction.contains("SKILL.md"),
            "embedded helper should carry operational guidance instead of deferring to SKILL.md"
        );
        assert_contains_all(
            &instruction,
            &[
                "Default to this automation-safe command shape",
                "Prefer scoped permissions over broad approval",
                "Prefer scoped filesystem and network access",
                "Troubleshooting",
            ],
        );
    }

    #[test]
    fn scoped_permission_examples_keep_permissions_before_prompt() {
        let instruction = resolve_instruction(&copilot_cli_helper_spec(), &[]).unwrap();

        for flag in [
            "--allow-tool='write'",
            "--allow-tool='shell(cargo test:*)'",
            "--allow-tool='shell(git:*)'",
            "--deny-tool='shell(git push)'",
        ] {
            assert_command_orders_flag_before_prompt(&instruction, flag);
        }
    }

    #[test]
    fn broad_access_flags_are_documented_as_exceptional_trusted_options() {
        let instruction = resolve_instruction(&copilot_cli_helper_spec(), &[]).unwrap();

        assert_trusted_only_warning_for_broad_access(&instruction);
    }

    #[test]
    fn copilot_cli_helper_skill_stays_aligned_with_embedded_instruction() {
        let skill = include_str!("../../../skills/copilot-cli-helper/SKILL.md");

        assert_contains_all(skill, CRITICAL_COPILOT_CLI_FRAGMENTS);
        assert_trusted_only_warning_for_broad_access(skill);
        assert_command_orders_flag_before_prompt(skill, "--allow-tool='write'");
        assert!(
            skill.find("--allow-tool='write'").unwrap() < skill.find("--allow-all-tools").unwrap(),
            "skill should present scoped permission examples before trusted broad-access examples"
        );
    }
}
