mod config;
mod error;
mod manifest;
mod mapping;
mod project;
mod release_state;

pub const DEFAULT_CHANGESET_DIR: &str = ".changeset";

/// Subdirectory within the changeset directory where changeset markdown files are stored.
/// Full path: `<project_root>/<changeset_dir>/changesets/`
pub const CHANGESETS_SUBDIR: &str = "changesets";

pub use config::{
    GitConfig, PackageChangesetConfig, RootChangesetConfig, TagFormat, load_changeset_configs,
    parse_package_config, parse_root_config,
};
pub use error::ProjectError;
pub use mapping::{FileMapping, PackageFiles, map_files_to_packages};
pub use project::{CargoProject, ProjectKind, discover_project, ensure_changeset_dir};
pub use release_state::{GraduationState, PrereleaseState};

pub type Result<T> = std::result::Result<T, ProjectError>;
