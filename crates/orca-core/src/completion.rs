use serde::{Deserialize, Serialize};

use crate::agent::AgentOutput;

#[derive(Debug, Clone, Serialize)]
pub struct CompletionReport {
    pub kpis_passed: bool,
    pub tests_passed: bool,
    pub feature_coverage_signed_off: bool,
    pub complete: bool,
    pub feedback: String,
}

impl CompletionReport {
    pub fn evaluate(outputs: &[AgentOutput]) -> Self {
        let kpi_measurements = outputs
            .iter()
            .filter(|output| output.output_contract == "kpi")
            .map(|output| parse_pass_fail(&output.content, "passed"))
            .collect::<Vec<_>>();
        let tests = outputs
            .iter()
            .filter(|output| output.output_contract == "test")
            .map(|output| parse_pass_fail(&output.content, "passed"))
            .collect::<Vec<_>>();
        let coverage = outputs
            .iter()
            .filter(|output| output.output_contract == "coverage")
            .map(|output| parse_pass_fail(&output.content, "signed_off"))
            .collect::<Vec<_>>();

        let kpis_passed =
            kpi_measurements.is_empty() || kpi_measurements.iter().all(|result| result.passed);
        let tests_passed = !tests.is_empty() && tests.iter().all(|result| result.passed);
        let feature_coverage_signed_off =
            !coverage.is_empty() && coverage.iter().all(|result| result.passed);
        let complete = kpis_passed && tests_passed && feature_coverage_signed_off;

        let feedback = kpi_measurements
            .iter()
            .chain(tests.iter())
            .chain(coverage.iter())
            .filter(|result| !result.passed || !result.feedback.trim().is_empty())
            .map(|result| result.feedback.clone())
            .collect::<Vec<_>>()
            .join("\n");

        Self {
            kpis_passed,
            tests_passed,
            feature_coverage_signed_off,
            complete,
            feedback,
        }
    }
}

#[derive(Debug, Clone)]
struct GateResult {
    passed: bool,
    feedback: String,
}

#[derive(Debug, Deserialize)]
struct StructuredGateResult {
    passed: Option<bool>,
    signed_off: Option<bool>,
    feedback: Option<String>,
    missing_features: Option<Vec<String>>,
}

fn parse_pass_fail(content: &str, field: &str) -> GateResult {
    if let Some(parsed) = parse_structured_gate_result(content) {
        let passed = if field == "signed_off" {
            parsed.signed_off.or(parsed.passed).unwrap_or(false)
        } else {
            parsed.passed.or(parsed.signed_off).unwrap_or(false)
        };
        let mut feedback = parsed.feedback.unwrap_or_default();
        if let Some(missing) = parsed.missing_features
            && !missing.is_empty()
        {
            if !feedback.is_empty() {
                feedback.push('\n');
            }
            feedback.push_str(&format!("missing features: {}", missing.join(", ")));
        }
        return GateResult { passed, feedback };
    }

    let uppercase = content.to_ascii_uppercase();
    let failed = uppercase.contains("FAIL")
        || uppercase.contains("NOT SIGNED")
        || uppercase.contains("SIGNED_OFF: FALSE")
        || uppercase.contains("SIGNED_OFF\":FALSE")
        || uppercase.contains("SIGNED_OFF\": FALSE")
        || uppercase.contains("SIGNED_OFF = FALSE")
        || uppercase.contains("SIGNED_OFF FALSE");
    let passed = !failed
        && (uppercase.contains("PASS")
            || uppercase.contains("SIGNED_OFF: TRUE")
            || uppercase.contains("SIGNED_OFF\":TRUE")
            || uppercase.contains("SIGNED_OFF\": TRUE")
            || uppercase.contains("SIGNED_OFF = TRUE")
            || uppercase.contains("SIGNED_OFF TRUE"));
    let feedback = if passed {
        String::new()
    } else {
        content.trim().to_string()
    };
    GateResult { passed, feedback }
}

fn parse_structured_gate_result(content: &str) -> Option<StructuredGateResult> {
    let trimmed = content.trim();
    serde_json::from_str::<StructuredGateResult>(trimmed)
        .ok()
        .or_else(|| extract_json_object(trimmed).and_then(|json| serde_json::from_str(json).ok()))
}

fn extract_json_object(content: &str) -> Option<&str> {
    let start = content.find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, ch) in content[start..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let end = start + offset + ch.len_utf8();
                    return Some(&content[start..end]);
                }
            }
            _ => {}
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use crate::agent::{AgentOutput, NodeKind};

    use super::CompletionReport;

    #[test]
    fn requires_all_three_completion_gates() {
        let outputs = vec![
            output("kpi", "kpi", r#"{"passed":true}"#),
            output("tests", "test", r#"{"passed":true}"#),
            output("coverage", "coverage", r#"{"signed_off":true}"#),
        ];

        let report = CompletionReport::evaluate(&outputs);

        assert!(report.complete);
    }

    #[test]
    fn kpi_gate_is_optional_when_workflow_has_no_kpi_outputs() {
        let outputs = vec![
            output("tests", "test", r#"{"passed":true}"#),
            output("coverage", "coverage", r#"{"signed_off":true}"#),
        ];

        let report = CompletionReport::evaluate(&outputs);

        assert!(report.kpis_passed);
        assert!(report.complete);
    }

    #[test]
    fn collects_feedback_from_failed_gate() {
        let outputs = vec![
            output("kpi", "kpi", r#"{"passed":true}"#),
            output(
                "tests",
                "test",
                r#"{"passed":false,"feedback":"add regression tests"}"#,
            ),
            output("coverage", "coverage", r#"{"signed_off":true}"#),
        ];

        let report = CompletionReport::evaluate(&outputs);

        assert!(!report.complete);
        assert!(report.feedback.contains("add regression tests"));
    }

    #[test]
    fn coverage_signed_off_false_in_wrapped_json_keeps_goal_incomplete() {
        let outputs = vec![
            output("kpi", "kpi", r#"{"passed":true}"#),
            output("tests", "test", r#"{"passed":true}"#),
            output(
                "coverage",
                "coverage",
                "Feature coverage result:\n```json\n{\"signed_off\":false,\"missing_features\":[\"login\"],\"feedback\":\"missing login\"}\n```",
            ),
        ];

        let report = CompletionReport::evaluate(&outputs);

        assert!(!report.complete);
        assert!(!report.feature_coverage_signed_off);
        assert!(report.feedback.contains("missing login"));
        assert!(report.feedback.contains("missing features: login"));
    }

    #[test]
    fn signed_off_false_text_fallback_is_not_success() {
        let outputs = vec![
            output("kpi", "kpi", r#"{"passed":true}"#),
            output("tests", "test", r#"{"passed":true}"#),
            output(
                "coverage",
                "coverage",
                "signed_off: false\nmissing feature coverage",
            ),
        ];

        let report = CompletionReport::evaluate(&outputs);

        assert!(!report.complete);
        assert!(!report.feature_coverage_signed_off);
        assert!(report.feedback.contains("missing feature coverage"));
    }

    #[test]
    fn failed_kpi_contract_keeps_goal_incomplete() {
        let outputs = vec![
            output("kpi", "kpi", r#"{"passed":true}"#),
            output(
                "kpi-measure",
                "kpi",
                r#"{"passed":false,"feedback":"latency target missed"}"#,
            ),
            output("tests", "test", r#"{"passed":true}"#),
            output("coverage", "coverage", r#"{"signed_off":true}"#),
        ];

        let report = CompletionReport::evaluate(&outputs);

        assert!(!report.complete);
        assert!(!report.kpis_passed);
        assert!(report.feedback.contains("latency target missed"));
    }

    fn output(id: &str, output_contract: &str, content: &str) -> AgentOutput {
        AgentOutput {
            agent_id: id.to_string(),
            kind: NodeKind::Agent,
            output_contract: output_contract.to_string(),
            phase_label: None,
            artifact_dir: None,
            content: content.to_string(),
            artifact_path: String::new(),
        }
    }
}
