use changeset_manifest::{ChangelogLocation, ComparisonLinks, TagFormat, ZeroVersionBehavior};

use crate::Result;

#[derive(Debug, Clone, Copy, Default)]
pub struct ProjectContext {
    pub is_single_package: bool,
}

#[derive(Debug, Clone)]
pub struct GitSettingsInput {
    pub commit: bool,
    pub tags: bool,
    pub keep_changesets: bool,
    pub tag_format: TagFormat,
}

impl Default for GitSettingsInput {
    fn default() -> Self {
        Self {
            commit: true,
            tags: true,
            keep_changesets: false,
            tag_format: TagFormat::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ChangelogSettingsInput {
    pub changelog: ChangelogLocation,
    pub comparison_links: ComparisonLinks,
}

#[derive(Debug, Clone, Default)]
pub struct VersionSettingsInput {
    pub zero_version_behavior: ZeroVersionBehavior,
}

pub trait InitInteractionProvider: Send + Sync {
    /// Prompts user to configure git settings. Returns None if user skips this group.
    ///
    /// The `context` parameter provides project information (e.g., whether it's a
    /// single-package project) so the provider can adapt defaults accordingly.
    ///
    /// # Errors
    ///
    /// Returns an error if the interaction cannot be completed.
    fn configure_git_settings(&self, context: ProjectContext) -> Result<Option<GitSettingsInput>>;

    /// Prompts user to configure changelog settings. Returns None if user skips this group.
    ///
    /// For single-package projects, the changelog location question should be skipped
    /// (defaulting to root), but `comparison_links` should still be prompted.
    ///
    /// # Errors
    ///
    /// Returns an error if the interaction cannot be completed.
    fn configure_changelog_settings(
        &self,
        context: ProjectContext,
    ) -> Result<Option<ChangelogSettingsInput>>;

    /// Prompts user to configure version settings. Returns None if user skips this group.
    ///
    /// # Errors
    ///
    /// Returns an error if the interaction cannot be completed.
    fn configure_version_settings(&self) -> Result<Option<VersionSettingsInput>>;
}
