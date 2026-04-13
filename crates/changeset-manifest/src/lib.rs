mod additional_packages;
mod config;
mod error;
mod external;
mod reader;
mod version_tracking_deps;
mod writer;

pub use additional_packages::{
    AdditionalPackageUpdate, add_additional_package, remove_additional_package,
    update_additional_package,
};
pub use config::{
    ChangelogLocation, ComparisonLinks, InitConfig, MetadataSection, NoneBumpBehavior, TagFormat,
    ZeroVersionBehavior,
};
pub use error::ManifestError;
pub use external::{
    read_external_version_string, restore_external_version, verify_external_version,
    write_external_version,
};
pub use reader::{
    has_inherited_version, has_workspace_package_version, read_document, read_version,
    read_workspace_version,
};
pub use version_tracking_deps::{
    add_version_tracking_dependency_to_additional_package,
    add_version_tracking_dependency_to_crate,
    remove_version_tracking_dependency_from_additional_package,
    remove_version_tracking_dependency_from_crate,
};
pub use writer::{
    remove_workspace_version, update_dependency_version, verify_version, write_metadata_section,
    write_version, write_workspace_version,
};
