use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::agent::AgentOutput;
use crate::errors::AppError;

#[derive(Debug, Clone)]
pub struct ArtifactWorkspace {
    root: PathBuf,
}

impl ArtifactWorkspace {
    pub fn create(base_dir: Option<&Path>) -> Result<Self, AppError> {
        let base = base_dir.unwrap_or_else(|| Path::new("orca-runs"));
        let run_id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let root = base.join(format!("run-{run_id}"));
        fs::create_dir_all(&root).map_err(|source| AppError::WriteArtifact {
            path: root.clone(),
            source,
        })?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn write_agent_output(&self, output: &AgentOutput) -> Result<AgentOutput, AppError> {
        self.write_agent_output_at(None, output)
    }

    pub fn write_agent_output_for_iteration(
        &self,
        iteration: usize,
        output: &AgentOutput,
    ) -> Result<AgentOutput, AppError> {
        self.write_agent_output_at(Some(iteration), output)
    }

    fn write_agent_output_at(
        &self,
        iteration: Option<usize>,
        output: &AgentOutput,
    ) -> Result<AgentOutput, AppError> {
        let artifact_dir = output
            .artifact_dir
            .as_deref()
            .map(str::to_string)
            .unwrap_or_else(|| output.output_contract.replace('_', "-"));
        let phase_dir = match iteration {
            Some(iteration) => self
                .root
                .join(format!("iteration-{iteration}"))
                .join(&artifact_dir),
            None => self.root.join(&artifact_dir),
        };
        fs::create_dir_all(&phase_dir).map_err(|source| AppError::WriteArtifact {
            path: phase_dir.clone(),
            source,
        })?;
        let path = phase_dir.join(format!("{}.md", sanitize(&output.agent_id)));
        fs::write(&path, &output.content).map_err(|source| AppError::WriteArtifact {
            path: path.clone(),
            source,
        })?;
        let mut persisted = output.clone();
        persisted.artifact_path = path.display().to_string();
        Ok(persisted)
    }

    pub fn write_text(&self, relative: &str, content: &str) -> Result<String, AppError> {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| AppError::WriteArtifact {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::write(&path, content).map_err(|source| AppError::WriteArtifact {
            path: path.clone(),
            source,
        })?;
        Ok(path.display().to_string())
    }

    pub fn append_event<T: Serialize>(&self, event: &T) -> Result<(), AppError> {
        let path = self.root.join("events.jsonl");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|source| AppError::WriteArtifact {
                path: path.clone(),
                source,
            })?;
        serde_json::to_writer(&mut file, event).map_err(|source| AppError::WriteArtifact {
            path: path.clone(),
            source: std::io::Error::other(source),
        })?;
        writeln!(file).map_err(|source| AppError::WriteArtifact { path, source })
    }

    pub fn write_json<T: Serialize>(&self, relative: &str, value: &T) -> Result<String, AppError> {
        let json = serde_json::to_string_pretty(value).map_err(std::io::Error::other);
        match json {
            Ok(json) => self.write_text(relative, &json),
            Err(source) => Err(AppError::WriteArtifact {
                path: self.root.join(relative),
                source,
            }),
        }
    }
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::agent::{AgentOutput, NodeKind};

    use super::ArtifactWorkspace;

    #[test]
    fn feature_generation_artifact_uses_feature_generation_directory() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = ArtifactWorkspace::create(Some(dir.path())).unwrap();
        let output = AgentOutput {
            agent_id: "feature-generator".to_string(),
            kind: NodeKind::Agent,
            output_contract: "feature".to_string(),
            phase_label: Some("feature_generation".to_string()),
            artifact_dir: Some("feature-generation".to_string()),
            content: "feature context".to_string(),
            artifact_path: String::new(),
        };

        let persisted = workspace
            .write_agent_output_for_iteration(1, &output)
            .unwrap();

        assert!(
            persisted
                .artifact_path
                .ends_with("iteration-1/feature-generation/feature-generator.md")
        );
    }
}
