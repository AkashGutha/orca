use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct GuiUiConfig {
    #[serde(default)]
    pub gui: GuiConfig,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct GuiConfig {
    pub max_visible_panes: usize,
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            max_visible_panes: 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GuiUiConfig;

    #[test]
    fn parses_gui_namespace_without_core_config_ownership() {
        let config: GuiUiConfig = toml::from_str(
            r#"
            [gui]
            max_visible_panes = 6
            "#,
        )
        .unwrap();

        assert_eq!(config.gui.max_visible_panes, 6);
    }
}
