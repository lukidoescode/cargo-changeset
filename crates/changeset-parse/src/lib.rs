mod error;
mod parse;
mod serialize;

pub use error::{FormatError, FrontMatterError, ValidationError};
pub use parse::parse_changeset;
pub use serialize::serialize_changeset;

pub type Result<T> = std::result::Result<T, FormatError>;
