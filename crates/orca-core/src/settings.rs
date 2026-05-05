use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::DEFAULT_CONFIG_PATH;
use crate::errors::AppError;

pub const DEFAULT_SETTINGS_PATH: &str = "settings.toml";
pub const USER_SETTINGS_RELATIVE_PATH: &str = ".config/orca/settings.toml";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Settings {
    #[serde(default)]
    pub sources: SourceSettings,
    #[serde(default)]
    pub defaults: DefaultRunSettings,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct SourceSettings {
    #[serde(default = "default_agent_sources")]
    pub agents: Vec<PathBuf>,
    #[serde(default = "default_instruction_sources")]
    pub instructions: Vec<PathBuf>,
    #[serde(default = "default_workflow_sources")]
    pub workflows: Vec<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct DefaultRunSettings {
    pub workflow: Option<PathBuf>,
    pub artifact_dir: Option<PathBuf>,
    pub max_parallel_agents: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedSettings {
    pub settings: Settings,
    pub path: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sources: SourceSettings::default(),
            defaults: DefaultRunSettings::default(),
        }
    }
}

impl Default for SourceSettings {
    fn default() -> Self {
        Self {
            agents: default_agent_sources(),
            instructions: default_instruction_sources(),
            workflows: default_workflow_sources(),
        }
    }
}

impl Default for DefaultRunSettings {
    fn default() -> Self {
        Self {
            workflow: Some(PathBuf::from("orca.default.toml")),
            artifact_dir: None,
            max_parallel_agents: None,
        }
    }
}

impl Settings {
    pub fn load_default() -> Result<LoadedSettings, AppError> {
        if let Some(path) = default_settings_candidates()
            .into_iter()
            .find(|candidate| candidate.is_file())
        {
            let settings = Self::load_from_path(&path)?;
            Ok(LoadedSettings {
                settings,
                path: Some(path),
            })
        } else {
            Ok(LoadedSettings {
                settings: Self::default(),
                path: None,
            })
        }
    }

    pub fn load_optional(path: Option<&Path>) -> Result<LoadedSettings, AppError> {
        match path {
            Some(path) => {
                let settings = Self::load_from_path(path)?;
                Ok(LoadedSettings {
                    settings,
                    path: Some(path.to_path_buf()),
                })
            }
            None => Self::load_default(),
        }
    }

    pub fn load_from_path(path: &Path) -> Result<Self, AppError> {
        let raw = fs::read_to_string(path).map_err(|source| AppError::ReadSettings {
            path: path.to_path_buf(),
            source,
        })?;
        let settings: Self = toml::from_str(&raw).map_err(|source| AppError::ParseSettings {
            path: path.to_path_buf(),
            message: source.to_string(),
        })?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn save_to_path(&self, path: &Path) -> Result<(), AppError> {
        self.validate()?;
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|source| AppError::WriteSettings {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let raw = toml::to_string_pretty(self).map_err(|source| AppError::ParseSettings {
            path: path.to_path_buf(),
            message: source.to_string(),
        })?;
        fs::write(path, raw).map_err(|source| AppError::WriteSettings {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn validate(&self) -> Result<(), AppError> {
        validate_sources("agents", &self.sources.agents)?;
        validate_sources("instructions", &self.sources.instructions)?;
        validate_sources("workflows", &self.sources.workflows)?;
        if self.defaults.max_parallel_agents == Some(0) {
            return Err(AppError::InvalidConfig(
                "settings.defaults.max_parallel_agents must be greater than zero".to_string(),
            ));
        }
        Ok(())
    }

    pub fn resolve_workflow_config(&self, requested: Option<&Path>) -> PathBuf {
        let requested = requested
            .filter(|path| !path.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .or_else(|| self.defaults.workflow.clone())
            .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));

        self.resolve_in_sources(&requested, &self.sources.workflows)
    }

    pub fn workflow_configs(&self) -> Vec<PathBuf> {
        let mut configs = self
            .sources
            .workflows
            .iter()
            .flat_map(|source| workflow_configs_in_dir(source))
            .collect::<Vec<_>>();
        configs.sort();
        configs.dedup();
        configs
    }

    pub fn instruction_sources<'a>(
        &'a self,
        explicit_dir: Option<&'a Path>,
        config_dir: Option<&'a Path>,
    ) -> Vec<PathBuf> {
        let mut sources = Vec::new();
        if let Some(dir) = explicit_dir {
            sources.push(dir.to_path_buf());
        }
        sources.extend(self.sources.instructions.iter().cloned());
        if let Some(dir) = config_dir {
            sources.push(dir.to_path_buf());
        }
        sources
    }

    pub fn default_artifact_dir(&self) -> Option<PathBuf> {
        self.defaults.artifact_dir.clone()
    }

    pub fn default_max_parallel_agents(&self) -> Option<usize> {
        self.defaults.max_parallel_agents
    }

    fn resolve_in_sources(&self, path: &Path, sources: &[PathBuf]) -> PathBuf {
        if path.exists() || path.is_absolute() {
            return path.to_path_buf();
        }
        if path
            .parent()
            .is_some_and(|parent| !parent.as_os_str().is_empty())
        {
            return path.to_path_buf();
        }
        sources
            .iter()
            .map(|source| source.join(path))
            .find(|candidate| candidate.exists())
            .unwrap_or_else(|| {
                sources
                    .first()
                    .map(|source| source.join(path))
                    .unwrap_or_else(|| path.to_path_buf())
            })
    }
}

pub fn default_settings_candidates() -> Vec<PathBuf> {
    let mut candidates = vec![PathBuf::from(DEFAULT_SETTINGS_PATH)];
    if let Some(path) = user_settings_path() {
        candidates.push(path);
    }
    candidates
}

pub fn user_settings_path() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("USERPROFILE")
                .filter(|home| !home.is_empty())
                .map(PathBuf::from)
        })
        .map(|home| home.join(USER_SETTINGS_RELATIVE_PATH))
}

fn validate_sources(name: &str, sources: &[PathBuf]) -> Result<(), AppError> {
    for source in sources {
        if source.as_os_str().is_empty() {
            return Err(AppError::InvalidConfig(format!(
                "settings.sources.{name} must not contain empty paths"
            )));
        }
        if source.exists() && !source.is_dir() {
            return Err(AppError::InvalidConfig(format!(
                "settings.sources.{name} `{}` must be a directory",
                source.display()
            )));
        }
    }
    Ok(())
}

fn default_agent_sources() -> Vec<PathBuf> {
    vec![PathBuf::from("agents")]
}

fn default_instruction_sources() -> Vec<PathBuf> {
    vec![PathBuf::from("instructions")]
}

fn default_workflow_sources() -> Vec<PathBuf> {
    vec![PathBuf::from("config")]
}

fn workflow_configs_in_dir(source: &Path) -> Vec<PathBuf> {
    let mut configs = Vec::new();
    let mut pending = vec![source.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if is_workflow_config(&path) {
                configs.push(path);
            }
        }
    }
    configs
}

fn is_workflow_config(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension, "toml" | "yaml" | "yml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_search_config_directory_for_default_workflow() {
        let settings = Settings::default();

        assert_eq!(
            settings.resolve_workflow_config(None),
            PathBuf::from("config/orca.default.toml")
        );
    }

    #[test]
    fn explicit_workflow_path_wins_over_source_search() {
        let settings = Settings::default();

        assert_eq!(
            settings.resolve_workflow_config(Some(Path::new("custom/workflow.toml"))),
            PathBuf::from("custom/workflow.toml")
        );
    }

    #[test]
    fn parses_and_saves_settings_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        let settings = Settings {
            sources: SourceSettings {
                agents: vec![dir.path().join("agents")],
                instructions: vec![dir.path().join("instructions")],
                workflows: vec![dir.path().join("workflows")],
            },
            defaults: DefaultRunSettings {
                workflow: Some(PathBuf::from("local.toml")),
                artifact_dir: Some(PathBuf::from("artifacts")),
                max_parallel_agents: Some(6),
            },
        };

        settings.save_to_path(&path).unwrap();
        let parsed = Settings::load_from_path(&path).unwrap();

        assert_eq!(parsed, settings);
    }

    #[test]
    fn rejects_source_paths_that_exist_as_files() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("not-dir");
        fs::write(&file, "").unwrap();
        let settings = Settings {
            sources: SourceSettings {
                agents: vec![file],
                ..SourceSettings::default()
            },
            ..Settings::default()
        };

        let error = settings.validate().unwrap_err();

        assert!(error.to_string().contains("must be a directory"));
    }

    #[test]
    fn collects_workflow_configs_from_ordered_sources() {
        let dir = tempfile::tempdir().unwrap();
        let workflows = dir.path().join("workflows");
        let nested = workflows.join("nested");
        fs::create_dir_all(&nested).unwrap();
        fs::write(workflows.join("a.toml"), "").unwrap();
        fs::write(workflows.join("ignored.md"), "").unwrap();
        fs::write(nested.join("b.yaml"), "").unwrap();
        let settings = Settings {
            sources: SourceSettings {
                workflows: vec![workflows.clone()],
                ..SourceSettings::default()
            },
            ..Settings::default()
        };

        let configs = settings.workflow_configs();

        assert_eq!(
            configs,
            vec![workflows.join("a.toml"), nested.join("b.yaml")]
        );
    }
}
