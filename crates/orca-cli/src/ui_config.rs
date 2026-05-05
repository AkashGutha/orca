use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct CliUiConfig {
    #[serde(default)]
    pub tui: TuiConfig,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct TuiConfig {
    pub max_visible_panes: usize,
    pub compact_header_height: u16,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            max_visible_panes: 4,
            compact_header_height: 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CliUiConfig;

    #[test]
    fn parses_tui_namespace_without_core_config_ownership() {
        let config: CliUiConfig = toml::from_str(
            r#"
            [tui]
            max_visible_panes = 6
            compact_header_height = 4
            "#,
        )
        .unwrap();

        assert_eq!(config.tui.max_visible_panes, 6);
        assert_eq!(config.tui.compact_header_height, 4);
    }
}
