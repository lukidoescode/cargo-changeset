use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChangelogLocation {
    #[default]
    Root,
    PerPackage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComparisonLinksSetting {
    #[default]
    Auto,
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ChangelogConfig {
    #[serde(default)]
    pub changelog: ChangelogLocation,
    #[serde(default)]
    pub comparison_links: ComparisonLinksSetting,
    pub comparison_links_template: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = ChangelogConfig::default();
        assert_eq!(config.changelog, ChangelogLocation::Root);
        assert_eq!(config.comparison_links, ComparisonLinksSetting::Auto);
        assert!(config.comparison_links_template.is_none());
    }

    #[test]
    fn deserialize_config() {
        let toml = r#"
            changelog = "per-package"
            comparison-links = "enabled"
            comparison-links-template = "https://example.com/{repository}/compare/{base}...{target}"
        "#;

        let config: ChangelogConfig = toml::from_str(toml).expect("should deserialize");
        assert_eq!(config.changelog, ChangelogLocation::PerPackage);
        assert_eq!(config.comparison_links, ComparisonLinksSetting::Enabled);
        assert_eq!(
            config.comparison_links_template.as_deref(),
            Some("https://example.com/{repository}/compare/{base}...{target}")
        );
    }

    #[test]
    fn deserialize_partial_config() {
        let toml = r#"
            comparison-links = "disabled"
        "#;

        let config: ChangelogConfig = toml::from_str(toml).expect("should deserialize");
        assert_eq!(config.changelog, ChangelogLocation::Root);
        assert_eq!(config.comparison_links, ComparisonLinksSetting::Disabled);
        assert!(config.comparison_links_template.is_none());
    }

    #[test]
    fn deserialize_invalid_changelog_value_fails() {
        let toml = r#"
            changelog = "invalid-value"
        "#;

        let result: Result<ChangelogConfig, _> = toml::from_str(toml);
        assert!(result.is_err());
    }
}
