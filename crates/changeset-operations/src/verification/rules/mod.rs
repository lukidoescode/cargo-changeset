mod coverage;
mod deleted;

pub use coverage::CoverageRule;
pub use deleted::DeletedChangesetsRule;

use super::{VerificationContext, VerificationResult};
use crate::Result;

pub trait VerificationRule {
    /// # Errors
    ///
    /// Returns an error if the rule check cannot be completed.
    fn check(&self, context: &VerificationContext, result: &mut VerificationResult) -> Result<()>;
}
