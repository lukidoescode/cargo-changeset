use std::path::Path;

use changeset_operations::operations::InitOperation;
use changeset_operations::providers::FileSystemProjectProvider;

use crate::error::Result;

pub(crate) fn run(start_path: &Path) -> Result<()> {
    let project_provider = FileSystemProjectProvider::new();

    let operation = InitOperation::new(project_provider);
    let output = operation.execute(start_path)?;

    if output.created {
        println!(
            "Created changeset directory at '{}'",
            output.changeset_dir.display()
        );
    } else {
        println!(
            "Changeset directory already exists at '{}'",
            output.changeset_dir.display()
        );
    }

    Ok(())
}
