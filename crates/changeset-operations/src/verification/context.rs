use std::collections::HashSet;
use std::path::PathBuf;

use changeset_core::PackageInfo;

pub struct VerificationContext {
    pub affected_packages: Vec<PackageInfo>,
    pub transitive_dependents: HashSet<String>,
    pub changeset_files: Vec<PathBuf>,
    pub deleted_changesets: Vec<PathBuf>,
    pub project_files: Vec<PathBuf>,
    pub ignored_files: Vec<PathBuf>,
}
