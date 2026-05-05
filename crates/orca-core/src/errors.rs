use std::io;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("failed to read config `{path}`: {source}")]
    ReadConfig { path: PathBuf, source: io::Error },

    #[error("failed to parse config `{path}`: {message}")]
    ParseConfig { path: PathBuf, message: String },

    #[error("failed to read settings `{path}`: {source}")]
    ReadSettings { path: PathBuf, source: io::Error },

    #[error("failed to write settings `{path}`: {source}")]
    WriteSettings { path: PathBuf, source: io::Error },

    #[error("failed to parse settings `{path}`: {message}")]
    ParseSettings { path: PathBuf, message: String },

    #[error("invalid config: {0}")]
    InvalidConfig(String),

    #[error("invalid goal request: {0}")]
    InvalidGoal(String),

    #[error("agent `{agent_id}` failed: {message}")]
    AgentFailed { agent_id: String, message: String },

    #[error("failed to read goal `{path}`: {source}")]
    ReadGoal { path: PathBuf, source: io::Error },

    #[error("failed to read instruction `{path}`: {source}")]
    ReadInstruction { path: PathBuf, source: io::Error },

    #[error("failed to write artifact `{path}`: {source}")]
    WriteArtifact { path: PathBuf, source: io::Error },

    #[error("failed to initialize logging: {0}")]
    Logging(String),

    #[error("failed to serialize JSON output: {0}")]
    Json(#[from] serde_json::Error),
}
