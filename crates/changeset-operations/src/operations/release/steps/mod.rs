use std::path::PathBuf;

use changeset_core::Changeset;
use changeset_project::{GraduationState, PrereleaseState};
use semver::Version;

#[derive(Debug, Clone)]
pub struct ChangesetFileState {
    pub path: PathBuf,
    pub original_consumed_status: Option<String>,
    pub backup: Option<Changeset>,
}

#[derive(Debug, Clone)]
pub struct ChangelogFileState {
    pub path: PathBuf,
    pub version: Version,
    pub package: Option<String>,
    pub original_content: Option<String>,
    pub file_existed: bool,
}

#[derive(Debug, Clone)]
pub struct PrereleaseStateUpdate {
    pub original: Option<PrereleaseState>,
    pub new_state: PrereleaseState,
}

#[derive(Debug, Clone)]
pub struct GraduationStateUpdate {
    pub original: Option<GraduationState>,
    pub new_state: GraduationState,
}
