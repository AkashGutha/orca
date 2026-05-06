use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::agent::{AgentConfig, AgentSpec, ApprovalMode, NodeKind};
use crate::errors::AppError;
use crate::settings::Settings;

pub const DEFAULT_CONFIG_PATH: &str = "config/orca.default.toml";

const DEFAULT_CONFIG_CANDIDATES: &[&str] = &[
    "config/orca.default.toml",
    "config/orca.yaml",
    "config/orca.yml",
];

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AppConfig {
    pub max_concurrency: Option<usize>,
    pub continue_on_error: Option<bool>,
    pub orchestration: Option<OrchestrationConfig>,
    pub agents: Option<toml::Value>,
    #[serde(default)]
    pub commands: Vec<CommandConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommandConfig {
    pub id: String,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub cwd: Option<PathBuf>,
    pub timeout_secs: Option<u64>,
    #[serde(default)]
    pub retries: u32,
    pub retry_delay_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrchestrationConfig {
    pub instruction_dir: Option<PathBuf>,
    pub artifact_dir: Option<PathBuf>,
    #[serde(alias = "max_parallel")]
    pub max_parallel_agents: Option<usize>,
    pub min_successful_planners: Option<usize>,
    pub min_successful_critics: Option<usize>,
    pub max_iterations: Option<usize>,
    pub approval_mode: Option<ApprovalMode>,
    pub defaults: Option<OrchestrationDefaults>,
    #[serde(default)]
    pub backends: Vec<BackendProfile>,
    #[serde(default)]
    pub nodes: Vec<OrchestrationNode>,
    #[serde(default)]
    pub connections: Vec<OrchestrationConnection>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OrchestrationDefaults {
    pub backend: Option<String>,
    pub model: Option<String>,
    pub timeout_secs: Option<u64>,
    pub retries: Option<u32>,
    pub retry_delay_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendProfile {
    pub id: String,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestrationNode {
    pub id: String,
    #[serde(default = "default_node_kind")]
    pub kind: NodeKind,
    pub evaluation: Option<String>,
    #[serde(default = "default_output_contract")]
    pub output_contract: String,
    pub artifact_dir: Option<String>,
    pub phase_label: Option<String>,
    pub instruction: Option<String>,
    pub backend: Option<String>,
    pub model: Option<String>,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub resources: Vec<String>,
    pub timeout_secs: Option<u64>,
    pub retries: Option<u32>,
    pub retry_delay_ms: Option<u64>,
    #[serde(default)]
    pub inputs: Vec<InputSelector>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InputSelector {
    pub source: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrchestrationConnection {
    pub from: String,
    pub to: Vec<String>,
    pub condition: Option<BranchCondition>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BranchCondition {
    True,
    False,
}

impl OrchestrationConfig {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.nodes.is_empty() {
            return Err(AppError::InvalidConfig(
                "orchestration.nodes must contain at least one node".to_string(),
            ));
        }
        validate_unique_backend_ids(&self.backends)?;
        validate_nodes(self)?;
        validate_connections(self)?;
        validate_no_cycles(self)?;
        Ok(())
    }

    pub fn into_agent_config(self) -> Result<AgentConfig, AppError> {
        self.validate()?;
        let OrchestrationConfig {
            instruction_dir,
            artifact_dir,
            max_parallel_agents,
            min_successful_planners,
            min_successful_critics,
            max_iterations,
            approval_mode,
            defaults,
            backends,
            nodes,
            connections,
        } = self;
        let defaults = defaults.unwrap_or_default();
        let default_backend_id = defaults
            .backend
            .clone()
            .unwrap_or_else(|| "copilot".to_string());
        let mut backends: BTreeMap<String, BackendProfile> = backends
            .iter()
            .cloned()
            .map(|backend| (backend.id.clone(), backend))
            .collect();
        backends
            .entry("copilot".to_string())
            .or_insert_with(default_copilot_backend);

        let mut connection_dependencies = BTreeMap::<String, Vec<String>>::new();
        for connection in &connections {
            for target in &connection.to {
                connection_dependencies
                    .entry(target.clone())
                    .or_default()
                    .push(connection.from.clone());
            }
        }
        let nodes_by_id = nodes
            .iter()
            .map(|node| (node.id.clone(), node.clone()))
            .collect::<BTreeMap<_, _>>();

        let specs = nodes
            .into_iter()
            .filter(|node| node.kind == NodeKind::Agent)
            .map(|node| {
                let node_id = node.id.clone();
                let backend_id = node
                    .backend
                    .clone()
                    .unwrap_or_else(|| default_backend_id.clone());
                let backend = backends.get(&backend_id).ok_or_else(|| {
                    AppError::InvalidConfig(format!(
                        "node `{}` references unknown backend `{backend_id}`",
                        node.id
                    ))
                })?;
                let command = node
                    .command
                    .clone()
                    .unwrap_or_else(|| backend.program.clone());
                let model = node
                    .model
                    .clone()
                    .or_else(|| defaults.model.clone())
                    .unwrap_or_else(|| "default".to_string());
                let args = if !node.args.is_empty() {
                    node.args.clone()
                } else {
                    backend
                        .args
                        .iter()
                        .map(|arg| arg.replace("{model}", &model))
                        .collect()
                };
                Ok(AgentSpec {
                    id: node.id,
                    kind: node.kind,
                    output_contract: normalize_contract(&node.output_contract),
                    artifact_dir: node.artifact_dir,
                    phase_label: node.phase_label,
                    instruction: node.instruction,
                    command,
                    args,
                    input_sources: node.inputs.into_iter().map(|input| input.source).collect(),
                    depends_on: expand_executable_dependencies(
                        merge_dependencies(
                            node.depends_on,
                            connection_dependencies.remove(&node_id).unwrap_or_default(),
                        ),
                        &nodes_by_id,
                        &connection_dependencies,
                    ),
                    resources: node.resources,
                    timeout_secs: node.timeout_secs.or(defaults.timeout_secs),
                    retries: node.retries.or(defaults.retries).unwrap_or(0),
                    retry_delay_ms: node.retry_delay_ms.or(defaults.retry_delay_ms),
                })
            })
            .collect::<Result<Vec<_>, AppError>>()?;

        Ok(AgentConfig {
            instruction_dir,
            artifact_dir,
            max_parallel_agents,
            min_successful_planners,
            min_successful_critics,
            max_iterations,
            approval_mode,
            specs,
        })
    }
}

pub fn load_config(path: &Path) -> Result<AppConfig, AppError> {
    let path = resolve_config_path(path);
    let raw = fs::read_to_string(&path).map_err(|source| AppError::ReadConfig {
        path: path.clone(),
        source,
    })?;
    let config = parse_config(&path, &raw)?;
    if config.agents.is_some() {
        return Err(AppError::InvalidConfig(
            "legacy [agents] config is no longer supported; use [orchestration] with [[orchestration.nodes]]".to_string(),
        ));
    }
    Ok(config)
}

pub fn resolve_config_path(path: &Path) -> PathBuf {
    if path.exists() {
        return path.to_path_buf();
    }

    if is_default_config_path(path)
        && let Some(candidate) = DEFAULT_CONFIG_CANDIDATES
            .iter()
            .map(Path::new)
            .find(|candidate| candidate.exists())
    {
        return candidate.to_path_buf();
    }

    path.to_path_buf()
}

fn is_default_config_path(path: &Path) -> bool {
    path == Path::new(DEFAULT_CONFIG_PATH)
}

fn parse_config(path: &Path, raw: &str) -> Result<AppConfig, AppError> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("yaml" | "yml") => serde_yaml::from_str(raw).map_err(|source| AppError::ParseConfig {
            path: path.to_path_buf(),
            message: source.to_string(),
        }),
        _ => toml::from_str(raw).map_err(|source| AppError::ParseConfig {
            path: path.to_path_buf(),
            message: source.to_string(),
        }),
    }
}

pub fn default_orchestration_config() -> OrchestrationConfig {
    let mut config = OrchestrationConfig {
        instruction_dir: None,
        artifact_dir: None,
        max_parallel_agents: Some(8),
        min_successful_planners: Some(2),
        min_successful_critics: Some(1),
        max_iterations: Some(10),
        approval_mode: Some(ApprovalMode::Auto),
        defaults: Some(OrchestrationDefaults {
            backend: Some("copilot".to_string()),
            model: Some("default".to_string()),
            timeout_secs: Some(900),
            retries: Some(0),
            retry_delay_ms: None,
        }),
        backends: vec![default_copilot_backend()],
        nodes: Vec::new(),
        connections: Vec::new(),
    };

    for (id, contract, instruction) in [
        ("feature-generator", "feature", "feature_generation.md"),
        ("planner-a", "plan", "planning.md"),
        ("planner-b", "plan", "planning.md"),
        ("planner-c", "plan", "planning.md"),
        ("planner-d", "plan", "planning.md"),
        ("critic-a", "critique", "critique.md"),
        ("critic-b", "critique", "critique.md"),
        ("critic-c", "critique", "critique.md"),
        ("critic-d", "critique", "critique.md"),
        ("orchestrator", "golden_plan", "orchestration.md"),
        ("kpi-agent", "kpi", "kpi_generation.md"),
        ("kpi-measure-agent", "kpi", "kpi_measurement.md"),
        ("test-planner-a", "test_plan", "test_planning.md"),
        ("test-planner-b", "test_plan", "test_planning.md"),
        ("test-planner-c", "test_plan", "test_planning.md"),
        ("test-planner-d", "test_plan", "test_planning.md"),
        ("golden-test-plan", "test_plan", "orchestration.md"),
        ("test-agent", "test", "test_generation.md"),
        ("work-agent", "implementation", "work.md"),
        ("coverage-agent", "coverage", "feature_coverage.md"),
        ("feedback-agent", "feedback", "feedback.md"),
    ] {
        config
            .nodes
            .push(default_agent_node(id, contract, instruction));
    }

    set_inputs(&mut config, "feature-generator", ["feedback"]);
    for planner in ["planner-a", "planner-b", "planner-c", "planner-d"] {
        set_inputs(&mut config, planner, ["feedback", "feature-generator"]);
    }
    set_inputs(&mut config, "critic-a", ["planner-a"]);
    set_inputs(&mut config, "critic-b", ["planner-b"]);
    set_inputs(&mut config, "critic-c", ["planner-c"]);
    set_inputs(&mut config, "critic-d", ["planner-d"]);
    set_inputs(
        &mut config,
        "orchestrator",
        [
            "feedback",
            "planner-a",
            "planner-b",
            "planner-c",
            "planner-d",
            "critic-a",
            "critic-b",
            "critic-c",
            "critic-d",
        ],
    );
    for planner in [
        "test-planner-a",
        "test-planner-b",
        "test-planner-c",
        "test-planner-d",
    ] {
        set_inputs(&mut config, planner, ["orchestrator"]);
    }
    set_inputs(
        &mut config,
        "golden-test-plan",
        [
            "orchestrator",
            "test-planner-a",
            "test-planner-b",
            "test-planner-c",
            "test-planner-d",
        ],
    );
    set_inputs(&mut config, "kpi-agent", ["orchestrator"]);
    set_inputs(&mut config, "test-agent", ["golden-test-plan"]);
    set_inputs(&mut config, "work-agent", ["orchestrator"]);
    set_inputs(
        &mut config,
        "kpi-measure-agent",
        ["orchestrator", "work-agent", "kpi-agent"],
    );
    set_inputs(
        &mut config,
        "coverage-agent",
        ["orchestrator", "work-agent", "test-agent"],
    );
    set_inputs(&mut config, "feedback-agent", ["gate-feedback"]);

    config.connections = vec![
        connection(
            "feedback",
            [
                "feature-generator",
                "planner-a",
                "planner-b",
                "planner-c",
                "planner-d",
            ],
        ),
        connection(
            "feature-generator",
            ["planner-a", "planner-b", "planner-c", "planner-d"],
        ),
        connection("planner-a", ["critic-a", "orchestrator"]),
        connection("planner-b", ["critic-b", "orchestrator"]),
        connection("planner-c", ["critic-c", "orchestrator"]),
        connection("planner-d", ["critic-d", "orchestrator"]),
        connection("critic-a", ["orchestrator"]),
        connection("critic-b", ["orchestrator"]),
        connection("critic-c", ["orchestrator"]),
        connection("critic-d", ["orchestrator"]),
        connection(
            "orchestrator",
            [
                "test-planner-a",
                "test-planner-b",
                "test-planner-c",
                "test-planner-d",
                "kpi-agent",
                "work-agent",
            ],
        ),
        connection("test-planner-a", ["golden-test-plan"]),
        connection("test-planner-b", ["golden-test-plan"]),
        connection("test-planner-c", ["golden-test-plan"]),
        connection("test-planner-d", ["golden-test-plan"]),
        connection("golden-test-plan", ["test-agent"]),
        connection("work-agent", ["kpi-measure-agent", "coverage-agent"]),
        connection("kpi-agent", ["kpi-measure-agent"]),
        connection("test-agent", ["coverage-agent"]),
    ];
    config
}

fn default_agent_node(id: &str, output_contract: &str, instruction: &str) -> OrchestrationNode {
    OrchestrationNode {
        id: id.to_string(),
        kind: NodeKind::Agent,
        evaluation: None,
        output_contract: output_contract.to_string(),
        artifact_dir: Some(output_contract.replace('_', "-")),
        phase_label: Some(output_contract.to_string()),
        instruction: Some(instruction.to_string()),
        backend: Some("copilot".to_string()),
        model: if output_contract == "implementation" {
            Some("gpt-5.5".to_string())
        } else {
            None
        },
        command: None,
        args: default_copilot_args_for_contract(output_contract),
        depends_on: Vec::new(),
        resources: if matches!(output_contract, "implementation" | "test") {
            vec!["workspace-writer".to_string()]
        } else {
            Vec::new()
        },
        timeout_secs: None,
        retries: None,
        retry_delay_ms: None,
        inputs: Vec::new(),
    }
}

fn set_inputs<const N: usize>(config: &mut OrchestrationConfig, id: &str, sources: [&str; N]) {
    if let Some(node) = config.nodes.iter_mut().find(|node| node.id == id) {
        node.inputs = sources
            .into_iter()
            .map(|source| InputSelector {
                source: source.to_string(),
            })
            .collect();
    }
}

fn default_copilot_backend() -> BackendProfile {
    BackendProfile {
        id: "copilot".to_string(),
        program: "copilot".to_string(),
        args: vec![
            "--silent".to_string(),
            "--no-ask-user".to_string(),
            "-p".to_string(),
            "{prompt}".to_string(),
        ],
    }
}

fn default_copilot_args_for_contract(output_contract: &str) -> Vec<String> {
    let mut args = vec!["--silent".to_string(), "--no-ask-user".to_string()];
    if output_contract == "implementation" {
        args.push("--model".to_string());
        args.push("gpt-5.5".to_string());
    }
    if matches!(output_contract, "implementation" | "test" | "parallel") {
        args.push("--allow-all-tools".to_string());
    }
    args.push("-p".to_string());
    args.push("{prompt}".to_string());
    args
}

fn default_node_kind() -> NodeKind {
    NodeKind::Agent
}

fn connection<const N: usize>(from: &str, to: [&str; N]) -> OrchestrationConnection {
    OrchestrationConnection {
        from: from.to_string(),
        to: to.into_iter().map(str::to_string).collect(),
        condition: None,
    }
}

fn validate_unique_backend_ids(backends: &[BackendProfile]) -> Result<(), AppError> {
    let mut seen = std::collections::HashSet::new();
    for backend in backends {
        if backend.id.trim().is_empty() {
            return Err(AppError::InvalidConfig(
                "backend id must not be empty".to_string(),
            ));
        }
        if backend.program.trim().is_empty() {
            return Err(AppError::InvalidConfig(format!(
                "backend `{}` program must not be empty",
                backend.id
            )));
        }
        if !seen.insert(backend.id.as_str()) {
            return Err(AppError::InvalidConfig(format!(
                "duplicate backend id `{}`",
                backend.id
            )));
        }
    }
    Ok(())
}

fn validate_nodes(config: &OrchestrationConfig) -> Result<(), AppError> {
    let mut seen = std::collections::HashSet::new();
    let backend_ids = config
        .backends
        .iter()
        .map(|backend| backend.id.as_str())
        .chain(std::iter::once("copilot"))
        .collect::<std::collections::HashSet<_>>();
    for node in &config.nodes {
        if node.id.trim().is_empty() {
            return Err(AppError::InvalidConfig(
                "node id must not be empty".to_string(),
            ));
        }
        if node.id.chars().any(char::is_whitespace) {
            return Err(AppError::InvalidConfig(format!(
                "node id `{}` must not contain whitespace",
                node.id
            )));
        }
        if !seen.insert(node.id.as_str()) {
            return Err(AppError::InvalidConfig(format!(
                "duplicate node id `{}`",
                node.id
            )));
        }
        if let Some(command) = &node.command
            && command.trim().is_empty()
        {
            return Err(AppError::InvalidConfig(format!(
                "node `{}` command must not be empty",
                node.id
            )));
        }
        if let Some(backend) = &node.backend
            && !backend_ids.contains(backend.as_str())
        {
            return Err(AppError::InvalidConfig(format!(
                "node `{}` references unknown backend `{backend}`",
                node.id
            )));
        }
        if node.output_contract.trim().is_empty() {
            return Err(AppError::InvalidConfig(format!(
                "node `{}` output_contract must not be empty",
                node.id
            )));
        }
        if node.kind == NodeKind::Branch
            && node
                .evaluation
                .as_deref()
                .is_none_or(|evaluation| evaluation.trim().is_empty())
        {
            return Err(AppError::InvalidConfig(format!(
                "branch node `{}` must set evaluation",
                node.id
            )));
        }
        if node.kind == NodeKind::Agent
            && node
                .instruction
                .as_deref()
                .is_none_or(|instruction| instruction.trim().is_empty())
        {
            return Err(AppError::InvalidConfig(format!(
                "agent node `{}` must set instruction",
                node.id
            )));
        }
    }
    Ok(())
}

fn validate_connections(config: &OrchestrationConfig) -> Result<(), AppError> {
    let nodes_by_id = config
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    for connection in &config.connections {
        let source = nodes_by_id.get(connection.from.as_str()).copied();
        if !is_pseudo_source(&connection.from) && source.is_none() {
            return Err(AppError::InvalidConfig(format!(
                "connection source `{}` does not exist",
                connection.from
            )));
        }
        if connection.condition.is_some()
            && !source.is_some_and(|node| node.kind == NodeKind::Branch)
        {
            return Err(AppError::InvalidConfig(format!(
                "connection from `{}` uses condition but source is not a branch node",
                connection.from
            )));
        }
        if source.is_some_and(|node| node.kind == NodeKind::Branch)
            && connection.condition.is_none()
        {
            return Err(AppError::InvalidConfig(format!(
                "connection from branch node `{}` must set condition",
                connection.from
            )));
        }
        for target in &connection.to {
            if !nodes_by_id.contains_key(target.as_str()) && !is_virtual_target(target) {
                return Err(AppError::InvalidConfig(format!(
                    "connection target `{target}` does not exist"
                )));
            }
        }
    }
    validate_branch_routes(config)?;
    for node in &config.nodes {
        for dependency in &node.depends_on {
            if !is_pseudo_source(dependency) && !nodes_by_id.contains_key(dependency.as_str()) {
                return Err(AppError::InvalidConfig(format!(
                    "node `{}` dependency `{}` does not exist",
                    node.id, dependency
                )));
            }
        }
        for input in &node.inputs {
            if input.source.trim().is_empty() {
                return Err(AppError::InvalidConfig(format!(
                    "node `{}` input source must not be empty",
                    node.id
                )));
            }
            if !is_pseudo_source(&input.source) && !nodes_by_id.contains_key(input.source.as_str())
            {
                return Err(AppError::InvalidConfig(format!(
                    "node `{}` input source `{}` does not exist",
                    node.id, input.source
                )));
            }
        }
    }
    Ok(())
}

fn validate_branch_routes(config: &OrchestrationConfig) -> Result<(), AppError> {
    for branch in config
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Branch)
    {
        let mut has_true = false;
        let mut has_false = false;
        for connection in config
            .connections
            .iter()
            .filter(|connection| connection.from == branch.id)
        {
            match connection.condition {
                Some(BranchCondition::True) => has_true = true,
                Some(BranchCondition::False) => has_false = true,
                None => {}
            }
        }
        if !has_true || !has_false {
            return Err(AppError::InvalidConfig(format!(
                "branch node `{}` must have true and false outgoing connections",
                branch.id
            )));
        }
    }
    Ok(())
}

fn is_pseudo_source(source: &str) -> bool {
    matches!(source, "goal" | "feedback" | "gate-feedback")
}

fn is_virtual_target(target: &str) -> bool {
    matches!(target, "completion-gate" | "approval-gate")
}

fn validate_no_cycles(config: &OrchestrationConfig) -> Result<(), AppError> {
    let mut dependencies = BTreeMap::<&str, Vec<&str>>::new();
    for node in &config.nodes {
        dependencies.entry(&node.id).or_default();
        for dependency in &node.depends_on {
            if !is_pseudo_source(dependency) {
                dependencies
                    .entry(&node.id)
                    .or_default()
                    .push(dependency.as_str());
            }
        }
    }
    for connection in &config.connections {
        if is_pseudo_source(&connection.from) {
            continue;
        }
        for target in &connection.to {
            if !is_virtual_target(target) {
                dependencies
                    .entry(target.as_str())
                    .or_default()
                    .push(connection.from.as_str());
            }
        }
    }

    let mut visiting = std::collections::HashSet::new();
    let mut visited = std::collections::HashSet::new();
    for node in dependencies.keys().copied().collect::<Vec<_>>() {
        visit_node(node, &dependencies, &mut visiting, &mut visited)?;
    }
    Ok(())
}

fn visit_node<'a>(
    node: &'a str,
    dependencies: &BTreeMap<&'a str, Vec<&'a str>>,
    visiting: &mut std::collections::HashSet<&'a str>,
    visited: &mut std::collections::HashSet<&'a str>,
) -> Result<(), AppError> {
    if visited.contains(node) {
        return Ok(());
    }
    if !visiting.insert(node) {
        return Err(AppError::InvalidConfig(format!(
            "orchestration dependency cycle includes `{node}`"
        )));
    }
    for dependency in dependencies.get(node).into_iter().flatten() {
        visit_node(dependency, dependencies, visiting, visited)?;
    }
    visiting.remove(node);
    visited.insert(node);
    Ok(())
}

fn default_output_contract() -> String {
    "generic".to_string()
}

fn normalize_contract(contract: &str) -> String {
    contract.trim().replace('-', "_")
}

fn merge_dependencies(mut explicit: Vec<String>, from_connections: Vec<String>) -> Vec<String> {
    for dependency in from_connections {
        if !explicit.contains(&dependency) {
            explicit.push(dependency);
        }
    }
    explicit
}

fn expand_executable_dependencies(
    dependencies: Vec<String>,
    nodes_by_id: &BTreeMap<String, OrchestrationNode>,
    connection_dependencies: &BTreeMap<String, Vec<String>>,
) -> Vec<String> {
    let mut expanded = Vec::new();
    for dependency in dependencies {
        append_executable_dependency(
            &dependency,
            nodes_by_id,
            connection_dependencies,
            &mut expanded,
            &mut Vec::new(),
        );
    }
    expanded
}

fn append_executable_dependency(
    dependency: &str,
    nodes_by_id: &BTreeMap<String, OrchestrationNode>,
    connection_dependencies: &BTreeMap<String, Vec<String>>,
    output: &mut Vec<String>,
    stack: &mut Vec<String>,
) {
    let Some(node) = nodes_by_id.get(dependency) else {
        return;
    };
    if node.kind == NodeKind::Agent {
        if !output.iter().any(|existing| existing == dependency) {
            output.push(dependency.to_string());
        }
        return;
    }
    if stack.iter().any(|id| id == dependency) {
        return;
    }
    stack.push(dependency.to_string());
    for upstream in node.depends_on.iter().chain(
        connection_dependencies
            .get(dependency)
            .into_iter()
            .flatten(),
    ) {
        append_executable_dependency(
            upstream,
            nodes_by_id,
            connection_dependencies,
            output,
            stack,
        );
    }
    stack.pop();
}

pub fn load_optional_config(path: &Path) -> Result<AppConfig, AppError> {
    let path = resolve_config_path(path);
    if path.exists() {
        load_config(&path)
    } else {
        Ok(AppConfig::default())
    }
}

pub fn load_optional_config_with_settings(
    path: Option<&Path>,
    settings: &Settings,
) -> Result<AppConfig, AppError> {
    let path = settings.resolve_workflow_config(path);
    if path.exists() {
        load_config(&path)
    } else {
        Ok(AppConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::{default_orchestration_config, load_config};

    #[test]
    fn parses_yaml_orchestration_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("orca.yaml");
        std::fs::write(
            &path,
            r#"
orchestration:
  max_parallel_agents: 2
  nodes:
    - id: planner
      output_contract: plan
      instruction: planning.md
      command: echo
"#,
        )
        .unwrap();

        let config = load_config(&path).unwrap();
        assert_eq!(config.orchestration.unwrap().max_parallel_agents, Some(2));
    }

    #[test]
    fn rejects_legacy_agents_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("orca.toml");
        std::fs::write(&path, "[agents]\nmax_iterations = 1\n").unwrap();

        let error = load_config(&path).unwrap_err();
        assert!(error.to_string().contains("legacy [agents]"));
    }

    #[test]
    fn branch_nodes_parse_conditional_routes_and_expand_dependencies() {
        let config: super::AppConfig = toml::from_str(
            r#"
[orchestration]

[[orchestration.nodes]]
id = "source"
instruction = "source.md"

[[orchestration.nodes]]
id = "quality-check"
kind = "branch"
evaluation = "input contains signed_off = true"

[[orchestration.nodes]]
id = "accepted"
instruction = "accepted.md"

[[orchestration.nodes]]
id = "revise"
instruction = "revise.md"

[[orchestration.connections]]
from = "source"
to = ["quality-check"]

[[orchestration.connections]]
from = "quality-check"
condition = "true"
to = ["accepted"]

[[orchestration.connections]]
from = "quality-check"
condition = "false"
to = ["revise"]
"#,
        )
        .unwrap();

        let agents = config.orchestration.unwrap().into_agent_config().unwrap();
        let accepted = agents
            .specs
            .iter()
            .find(|spec| spec.id == "accepted")
            .unwrap();
        let revise = agents
            .specs
            .iter()
            .find(|spec| spec.id == "revise")
            .unwrap();

        assert_eq!(accepted.depends_on, vec!["source"]);
        assert_eq!(revise.depends_on, vec!["source"]);
        assert!(!agents.specs.iter().any(|spec| spec.id == "quality-check"));
    }

    #[test]
    fn branch_nodes_require_true_and_false_routes() {
        let config: super::AppConfig = toml::from_str(
            r#"
[orchestration]

[[orchestration.nodes]]
id = "check"
kind = "branch"
evaluation = "input is complete"

[[orchestration.nodes]]
id = "accepted"
instruction = "accepted.md"

[[orchestration.connections]]
from = "check"
condition = "true"
to = ["accepted"]
"#,
        )
        .unwrap();

        let error = config.orchestration.unwrap().validate().unwrap_err();
        assert!(error.to_string().contains("true and false"));
    }

    #[test]
    fn node_inputs_require_non_empty_sources() {
        let config: super::AppConfig = toml::from_str(
            r#"
[orchestration]

[[orchestration.nodes]]
id = "consumer"
instruction = "consumer.md"
inputs = [{ source = "" }]
"#,
        )
        .unwrap();

        let error = config.orchestration.unwrap().validate().unwrap_err();
        assert!(error.to_string().contains("input source must not be empty"));
    }

    #[test]
    fn default_orchestration_uses_workspace_writer_resources() {
        let config = default_orchestration_config();
        let agents = config.into_agent_config().unwrap();

        assert_eq!(
            agents
                .specs
                .iter()
                .filter(|spec| spec.output_contract == "implementation"
                    && spec.resources.contains(&"workspace-writer".to_string()))
                .count(),
            1
        );
        assert_eq!(
            agents
                .specs
                .iter()
                .filter(|spec| spec.output_contract == "test"
                    && spec.resources.contains(&"workspace-writer".to_string()))
                .count(),
            1
        );
    }

    #[test]
    fn example_default_orchestration_config_is_valid() {
        let config: super::AppConfig =
            toml::from_str(include_str!("../../../config/orca.default.toml")).unwrap();

        config
            .orchestration
            .expect("example should contain orchestration config")
            .into_agent_config()
            .unwrap();
    }

    #[test]
    fn rtl_spec_to_env_config_replaces_work_and_test_agents_without_kpi() {
        let config: super::AppConfig =
            toml::from_str(include_str!("../../../config/rtl-spec-to-env.toml")).unwrap();
        let agents = config
            .orchestration
            .expect("RTL config should contain orchestration config")
            .into_agent_config()
            .unwrap();

        assert!(
            agents
                .specs
                .iter()
                .any(|spec| spec.id == "rtl-design-agent"
                    && spec.output_contract == "implementation")
        );
        assert!(
            agents.specs.iter().any(|spec| {
                spec.id == "uvm-test-bench-agent" && spec.output_contract == "test"
            })
        );
        assert!(
            !agents
                .specs
                .iter()
                .any(|spec| spec.output_contract == "kpi")
        );
    }
}
