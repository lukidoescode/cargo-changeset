use std::path::Path;

use changeset_operations::operations::StatusOperation;
use changeset_operations::providers::{
    FileSystemChangesetIO, FileSystemManifestWriter, FileSystemProjectProvider,
};
use changeset_operations::traits::ProjectProvider;

use crate::error::Result;
use crate::output::{PlainTextStatusFormatter, StatusFormatter};

pub(crate) fn run(start_path: &Path) -> Result<()> {
    let project_provider = FileSystemProjectProvider::new();
    let project = project_provider.discover_project(start_path)?;
    let changeset_reader = FileSystemChangesetIO::new(&project.root);
    let inherited_checker = FileSystemManifestWriter::new();

    let operation = StatusOperation::new(project_provider, changeset_reader, inherited_checker);
    let output = operation.execute(start_path)?;

    let formatter = PlainTextStatusFormatter;
    print!("{}", formatter.format_status(&output));

    Ok(())
}
