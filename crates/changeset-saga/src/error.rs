use std::fmt::Debug;

use thiserror::Error;

/// Error from a failed compensation operation.
#[derive(Debug, thiserror::Error)]
#[error("compensation failed for step '{step}': {description}")]
pub struct CompensationError<E> {
    /// Name of the step whose compensation failed.
    pub step: String,
    /// Description of what the compensation was trying to do.
    pub description: String,
    /// The underlying error.
    #[source]
    pub error: E,
}

/// Error from saga execution.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SagaError<E: Debug> {
    /// A step failed and all compensations succeeded.
    #[error("step '{step}' failed")]
    StepFailed {
        /// Name of the step that failed.
        step: String,
        /// The error that caused the step to fail.
        #[source]
        source: E,
    },

    /// A step failed and some compensations also failed.
    #[error("step '{failed_step}' failed, and {} compensation(s) also failed", compensation_errors.len())]
    CompensationFailed {
        /// Name of the step that originally failed.
        failed_step: String,
        /// The error from the failed step.
        step_error: E,
        /// Errors from failed compensations.
        compensation_errors: Vec<CompensationError<E>>,
    },
}
