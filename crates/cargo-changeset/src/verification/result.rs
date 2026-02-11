use std::collections::HashSet;
use std::path::PathBuf;

use changeset_core::PackageInfo;

/// Outcome of verification (output).
pub(crate) struct VerificationResult {
    /// Packages affected by code changes.
    pub affected_packages: Vec<PackageInfo>,
    /// Package names covered by changesets.
    pub covered_packages: HashSet<String>,
    /// Packages missing changeset coverage.
    pub uncovered_packages: Vec<PackageInfo>,
    /// Deleted changesets (if any).
    pub deleted_changesets: Vec<PathBuf>,
    /// Project files changed.
    pub project_files: Vec<PathBuf>,
    /// Ignored files.
    pub ignored_files: Vec<PathBuf>,
}

impl VerificationResult {
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.uncovered_packages.is_empty() && self.deleted_changesets.is_empty()
    }
}
