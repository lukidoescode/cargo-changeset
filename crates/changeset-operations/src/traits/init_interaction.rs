use changeset_git::DEFAULT_BASE_BRANCH;
use changeset_manifest::{
    ChangelogLocation, ComparisonLinks, NoneBumpBehavior, TagFormat, ZeroVersionBehavior,
};

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
    pub base_branch: String,
    pub commit_title_template: Option<String>,
    pub changes_in_body: Option<bool>,
}

impl Default for GitSettingsInput {
    fn default() -> Self {
        Self {
            commit: true,
            tags: true,
            keep_changesets: false,
            tag_format: TagFormat::default(),
            base_branch: String::from(DEFAULT_BASE_BRANCH),
            commit_title_template: None,
            changes_in_body: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ChangelogSettingsInput {
    pub changelog: ChangelogLocation,
    pub comparison_links: ComparisonLinks,
    pub comparison_links_template: Option<String>,
    pub dependency_bump_changelog_template: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct VersionSettingsInput {
    pub zero_version_behavior: Option<ZeroVersionBehavior>,
    pub none_bump_behavior: Option<NoneBumpBehavior>,
    pub none_bump_promote_message_template: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct FilteringSettingsInput {
    pub ignored_files: Vec<String>,
}

pub trait InitInteractionProvider: Send + Sync {
    /// # Errors
    ///
    /// Returns an error if the interaction cannot be completed.
    fn configure_git_settings(&self, context: ProjectContext) -> Result<Option<GitSettingsInput>>;

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

    /// # Errors
    ///
    /// Returns an error if the interaction cannot be completed.
    fn configure_version_settings(&self) -> Result<Option<VersionSettingsInput>>;

    /// # Errors
    ///
    /// Returns an error if the interaction cannot be completed.
    fn configure_filtering_settings(&self) -> Result<Option<FilteringSettingsInput>>;
}
