use std::path::Path;

use changeset_core::Changeset;

use crate::error::CliError;

/// Abstraction for reading and parsing changesets.
pub(crate) trait ChangesetReader {
    fn read_changeset(&self, path: &Path) -> Result<Changeset, CliError>;
}
