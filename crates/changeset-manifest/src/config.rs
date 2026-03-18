#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataSection {
    Workspace,
    Package,
}

impl std::fmt::Display for MetadataSection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Workspace => f.write_str("[workspace.metadata.changeset]"),
            Self::Package => f.write_str("[package.metadata.changeset]"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum TagFormat {
    #[default]
    VersionOnly,
    CratePrefixed,
}

impl TagFormat {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VersionOnly => "version-only",
            Self::CratePrefixed => "crate-prefixed",
        }
    }
}

impl std::fmt::Display for TagFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum ChangelogLocation {
    #[default]
    Root,
    PerPackage,
}

impl ChangelogLocation {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::PerPackage => "per-package",
        }
    }
}

impl std::fmt::Display for ChangelogLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum ComparisonLinks {
    #[default]
    Auto,
    Enabled,
    Disabled,
}

impl ComparisonLinks {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
        }
    }
}

impl std::fmt::Display for ComparisonLinks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum ZeroVersionBehavior {
    #[default]
    EffectiveMinor,
    AutoPromoteOnMajor,
}

impl ZeroVersionBehavior {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EffectiveMinor => "effective-minor",
            Self::AutoPromoteOnMajor => "auto-promote-on-major",
        }
    }
}

impl std::fmt::Display for ZeroVersionBehavior {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Default)]
pub struct InitConfig {
    pub commit: Option<bool>,
    pub tags: Option<bool>,
    pub keep_changesets: Option<bool>,
    pub tag_format: Option<TagFormat>,
    pub changelog: Option<ChangelogLocation>,
    pub comparison_links: Option<ComparisonLinks>,
    pub zero_version_behavior: Option<ZeroVersionBehavior>,
    pub dependency_bump_changelog_template: Option<String>,
    pub base_branch: Option<String>,
}

impl InitConfig {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commit.is_none()
            && self.tags.is_none()
            && self.keep_changesets.is_none()
            && self.tag_format.is_none()
            && self.changelog.is_none()
            && self.comparison_links.is_none()
            && self.zero_version_behavior.is_none()
            && self.dependency_bump_changelog_template.is_none()
            && self.base_branch.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_empty_returns_false_when_only_dependency_bump_changelog_template_set() {
        let config = InitConfig {
            dependency_bump_changelog_template: Some(
                "Updated dependency `{dependency}` to v{version}".to_string(),
            ),
            ..Default::default()
        };
        assert!(!config.is_empty());
    }

    #[test]
    fn is_empty_returns_false_when_only_base_branch_set() {
        let config = InitConfig {
            base_branch: Some("develop".to_string()),
            ..Default::default()
        };
        assert!(!config.is_empty());
    }
}
