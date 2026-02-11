mod error;
mod reader;
mod writer;

pub use error::ManifestError;
pub use reader::{
    has_inherited_version, has_workspace_package_version, read_document, read_version,
};
pub use writer::{remove_workspace_version, verify_version, write_version};
