use std::collections::HashSet;
use std::path::PathBuf;

use changeset_core::PackageInfo;
use gset::Getset;

#[derive(Getset)]
pub struct VerificationContext {
    #[getset(get, vis = "pub(crate)")]
    affected_packages: Vec<PackageInfo>,
    #[getset(get, vis = "pub(crate)")]
    transitive_dependents: HashSet<String>,
    #[getset(get, vis = "pub(crate)")]
    changeset_files: Vec<PathBuf>,
    #[getset(get, vis = "pub(crate)")]
    deleted_changesets: Vec<PathBuf>,
    #[getset(get, vis = "pub(crate)")]
    project_files: Vec<PathBuf>,
    #[getset(get, vis = "pub(crate)")]
    ignored_files: Vec<PathBuf>,
}

impl VerificationContext {
    pub(crate) fn new(
        affected_packages: Vec<PackageInfo>,
        transitive_dependents: HashSet<String>,
        changeset_files: Vec<PathBuf>,
        deleted_changesets: Vec<PathBuf>,
        project_files: Vec<PathBuf>,
        ignored_files: Vec<PathBuf>,
    ) -> Self {
        Self {
            affected_packages,
            transitive_dependents,
            changeset_files,
            deleted_changesets,
            project_files,
            ignored_files,
        }
    }
}
