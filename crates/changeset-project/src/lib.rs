mod config;
mod error;
mod manifest;
mod mapping;
mod project;

pub const DEFAULT_CHANGESET_DIR: &str = ".changeset";

pub use config::{
    PackageChangesetConfig, RootChangesetConfig, load_changeset_configs, parse_package_config,
    parse_root_config,
};
pub use error::ProjectError;
pub use mapping::{FileMapping, PackageFiles, map_files_to_packages};
pub use project::{CargoProject, ProjectKind, discover_project, ensure_changeset_dir};

pub type Result<T> = std::result::Result<T, ProjectError>;
