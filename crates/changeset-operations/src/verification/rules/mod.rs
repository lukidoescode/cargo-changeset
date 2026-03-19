mod coverage;
mod deleted;
mod none_bump;

pub use coverage::CoverageRule;
pub use deleted::DeletedChangesetsRule;
pub use none_bump::NoneBumpDisallowedRule;

use super::{VerificationContext, VerificationResult};
use crate::Result;

pub trait VerificationRule {
    /// # Errors
    ///
    /// Returns an error if the rule check cannot be completed.
    fn check(&self, context: &VerificationContext, result: &mut VerificationResult) -> Result<()>;
}
