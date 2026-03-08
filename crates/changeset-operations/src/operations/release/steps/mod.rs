use changeset_project::{GraduationState, PrereleaseState};

pub(crate) use super::types::ChangesetFileState;

#[derive(Debug, Clone)]
pub(crate) struct PrereleaseStateUpdate {
    pub(crate) original: Option<PrereleaseState>,
    pub(crate) new_state: PrereleaseState,
}

#[derive(Debug, Clone)]
pub(crate) struct GraduationStateUpdate {
    pub(crate) original: Option<GraduationState>,
    pub(crate) new_state: GraduationState,
}
