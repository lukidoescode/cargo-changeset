mod changelog;
mod config;
mod entry;
mod error;
mod forge;
mod format;

pub use changelog::Changelog;
pub use config::{ChangelogConfig, ChangelogLocation, ComparisonLinksSetting};
pub use entry::{ChangelogEntry, VersionRelease};
pub use error::ChangelogError;
pub use forge::{Forge, RepositoryInfo, expand_comparison_template};
pub use format::{
    format_comparison_links, format_entries, format_version_header, format_version_release,
    new_changelog,
};

pub type Result<T> = std::result::Result<T, ChangelogError>;
