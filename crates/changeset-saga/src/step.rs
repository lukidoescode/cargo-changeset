/// A step in a saga that can be executed and compensated.
///
/// Each step transforms an input into an output, with the ability to undo
/// its effects if a later step fails. The input is stored for compensation.
///
/// # Type Parameters
///
/// - `Input`: Data received from the previous step (or saga entry point)
/// - `Output`: Data produced for the next step
/// - `Context`: Shared dependencies (injected, not passed between steps)
/// - `Error`: The error type for step failures
pub trait SagaStep: Send + Sync {
    /// Data received from the previous step or saga entry point.
    type Input: Clone + Send + 'static;

    /// Data produced for the next step.
    type Output: Clone + Send + 'static;

    /// Shared context providing dependencies.
    type Context;

    /// Error type for step failures.
    type Error;

    /// Human-readable name for logging and error messages.
    fn name(&self) -> &'static str;

    /// Execute the step, transforming input into output.
    ///
    /// # Errors
    ///
    /// Returns an error if the step fails to complete.
    fn execute(&self, ctx: &Self::Context, input: Self::Input)
    -> Result<Self::Output, Self::Error>;

    /// Compensate (undo) the step's effects.
    ///
    /// Called during rollback when a later step fails. Receives the original
    /// input that was passed to `execute()`.
    ///
    /// The default implementation is a no-op, suitable for read-only steps.
    ///
    /// # Errors
    ///
    /// Returns an error if compensation fails.
    fn compensate(&self, ctx: &Self::Context, input: Self::Input) -> Result<(), Self::Error> {
        let _ = (ctx, input);
        Ok(())
    }

    /// Human-readable description of what compensation will do.
    fn compensation_description(&self) -> String {
        format!("undo {}", self.name())
    }
}
