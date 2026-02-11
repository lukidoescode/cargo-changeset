mod coverage;
mod deleted;

pub(crate) use coverage::CoverageRule;
pub(crate) use deleted::DeletedChangesetsRule;

use super::{VerificationContext, VerificationResult};
use crate::error::Result;

/// A verification rule that can be applied.
pub(crate) trait VerificationRule {
    fn check(&self, context: &VerificationContext, result: &mut VerificationResult) -> Result<()>;
}
