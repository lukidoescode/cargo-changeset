use std::collections::HashSet;
use std::path::PathBuf;

use changeset_core::PackageInfo;
use gset::Getset;

#[derive(Debug, Getset)]
pub struct VerificationResult {
    #[getset(get, vis = "pub")]
    affected_packages: Vec<PackageInfo>,
    #[getset(get, vis = "pub")]
    transitive_dependents: HashSet<String>,
    #[getset(get, vis = "pub")]
    covered_packages: HashSet<String>,
    #[getset(get, vis = "pub")]
    uncovered_packages: Vec<PackageInfo>,
    #[getset(get, vis = "pub")]
    deleted_changesets: Vec<PathBuf>,
    #[getset(get, vis = "pub")]
    none_bump_violations: Vec<String>,
    #[getset(get, vis = "pub")]
    project_files: Vec<PathBuf>,
    #[getset(get, vis = "pub")]
    ignored_files: Vec<PathBuf>,
}

impl VerificationResult {
    pub(crate) fn new(
        affected_packages: Vec<PackageInfo>,
        transitive_dependents: HashSet<String>,
        project_files: Vec<PathBuf>,
        ignored_files: Vec<PathBuf>,
    ) -> Self {
        Self {
            affected_packages,
            transitive_dependents,
            covered_packages: HashSet::new(),
            uncovered_packages: Vec::new(),
            deleted_changesets: Vec::new(),
            none_bump_violations: Vec::new(),
            project_files,
            ignored_files,
        }
    }

    pub(crate) fn insert_covered_package(&mut self, name: String) {
        self.covered_packages.insert(name);
    }

    pub(crate) fn set_uncovered_packages(&mut self, uncovered: Vec<PackageInfo>) {
        self.uncovered_packages = uncovered;
    }

    pub(crate) fn set_deleted_changesets(&mut self, deleted: Vec<PathBuf>) {
        self.deleted_changesets = deleted;
    }

    pub(crate) fn set_none_bump_violations(&mut self, violations: Vec<String>) {
        self.none_bump_violations = violations;
    }

    #[must_use]
    pub fn is_success(&self) -> bool {
        self.uncovered_packages.is_empty()
            && self.deleted_changesets.is_empty()
            && self.none_bump_violations.is_empty()
    }
}
