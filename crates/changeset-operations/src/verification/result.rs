use std::collections::HashSet;
use std::path::PathBuf;

use changeset_core::PackageInfo;

#[derive(Debug)]
pub struct VerificationResult {
    pub affected_packages: Vec<PackageInfo>,
    pub covered_packages: HashSet<String>,
    pub uncovered_packages: Vec<PackageInfo>,
    pub deleted_changesets: Vec<PathBuf>,
    pub project_files: Vec<PathBuf>,
    pub ignored_files: Vec<PathBuf>,
}

impl VerificationResult {
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.uncovered_packages.is_empty() && self.deleted_changesets.is_empty()
    }
}
