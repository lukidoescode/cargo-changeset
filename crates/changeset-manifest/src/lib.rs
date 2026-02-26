mod config;
mod error;
mod reader;
mod writer;

pub use config::{
    ChangelogLocation, ComparisonLinks, InitConfig, MetadataSection, TagFormat, ZeroVersionBehavior,
};
pub use error::ManifestError;
pub use reader::{
    has_inherited_version, has_workspace_package_version, read_document, read_version,
    read_workspace_version,
};
pub use writer::{
    remove_workspace_version, update_dependency_version, verify_version, write_metadata_section,
    write_version, write_workspace_version,
};
