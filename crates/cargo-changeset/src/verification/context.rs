use std::path::PathBuf;

use changeset_core::PackageInfo;

/// All data needed to perform verification (input).
pub(crate) struct VerificationContext {
    /// Packages that have code changes in the diff.
    pub affected_packages: Vec<PackageInfo>,
    /// Changeset files that are part of the diff (added/modified/renamed).
    pub changeset_files: Vec<PathBuf>,
    /// Changeset files that were deleted in the diff.
    pub deleted_changesets: Vec<PathBuf>,
    /// Project-level files changed (not in any package).
    pub project_files: Vec<PathBuf>,
    /// Files excluded by ignore patterns.
    pub ignored_files: Vec<PathBuf>,
}
