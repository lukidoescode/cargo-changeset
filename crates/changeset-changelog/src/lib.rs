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
pub use forge::{RepositoryInfo, expand_comparison_template};

pub type Result<T> = std::result::Result<T, ChangelogError>;
