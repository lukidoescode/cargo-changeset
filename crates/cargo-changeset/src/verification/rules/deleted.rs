use super::{VerificationContext, VerificationResult, VerificationRule};
use crate::error::Result;

pub(crate) struct DeletedChangesetsRule {
    allow_deleted: bool,
}

impl DeletedChangesetsRule {
    pub fn new(allow_deleted: bool) -> Self {
        Self { allow_deleted }
    }
}

impl VerificationRule for DeletedChangesetsRule {
    fn check(&self, context: &VerificationContext, result: &mut VerificationResult) -> Result<()> {
        if !self.allow_deleted {
            result
                .deleted_changesets
                .clone_from(&context.deleted_changesets);
        }
        Ok(())
    }
}
