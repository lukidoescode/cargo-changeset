use std::fmt::Debug;
use std::marker::PhantomData;

use crate::erased::{ErasedStep, StepWrapper};
use crate::saga::Saga;
use crate::step::SagaStep;

/// Marker type for a builder with no steps.
pub struct Empty;

/// Marker type for a builder with at least one step.
pub struct HasSteps<LastOutput>(PhantomData<LastOutput>);

/// Type-state builder for constructing type-safe sagas.
///
/// The builder enforces at compile-time that:
/// - Each step's input type matches the previous step's output type
/// - The saga's input type matches the first step's input
/// - The saga's output type matches the last step's output
///
/// # Compile-time Type Safety
///
/// The builder ensures type safety at compile time. Mismatched types will not compile:
///
/// ```compile_fail
/// use changeset_saga::{SagaBuilder, SagaStep};
///
/// struct StepA;
/// impl SagaStep for StepA {
///     type Input = i32;
///     type Output = String;  // Outputs String
///     type Context = ();
///     type Error = ();
///     fn name(&self) -> &'static str { "a" }
///     fn execute(&self, _: &(), input: i32) -> Result<String, ()> {
///         Ok(input.to_string())
///     }
/// }
///
/// struct StepB;
/// impl SagaStep for StepB {
///     type Input = i32;  // Expects i32, not String!
///     type Output = i32;
///     type Context = ();
///     type Error = ();
///     fn name(&self) -> &'static str { "b" }
///     fn execute(&self, _: &(), input: i32) -> Result<i32, ()> {
///         Ok(input * 2)
///     }
/// }
///
/// // This should fail: StepB expects i32 but StepA outputs String
/// let saga = SagaBuilder::new()
///     .first_step(StepA)
///     .then(StepB)  // Compile error here!
///     .build();
/// ```
///
/// An empty saga (without calling `first_step()`) cannot be built:
///
/// ```compile_fail
/// use changeset_saga::SagaBuilder;
///
/// // Cannot build an empty saga - `build()` is only available after `first_step()`
/// let saga = SagaBuilder::<(), (), (), ()>::new().build();
/// ```
pub struct SagaBuilder<Input, Output, Ctx, Err, State> {
    steps: Vec<Box<dyn ErasedStep<Ctx, Err>>>,
    _phantom: PhantomData<(Input, Output, State)>,
}

impl<Ctx, Err> SagaBuilder<(), (), Ctx, Err, Empty> {
    /// Create a new saga builder in the empty state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            _phantom: PhantomData,
        }
    }
}

impl<Ctx, Err> Default for SagaBuilder<(), (), Ctx, Err, Empty> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Ctx, Err> SagaBuilder<(), (), Ctx, Err, Empty> {
    /// Add the first step to the saga.
    ///
    /// This establishes the saga's input type from the step's input type.
    #[must_use]
    pub fn first_step<S>(
        self,
        step: S,
    ) -> SagaBuilder<S::Input, S::Output, Ctx, Err, HasSteps<S::Output>>
    where
        S: SagaStep<Context = Ctx, Error = Err> + 'static,
    {
        let mut steps = self.steps;
        steps.push(Box::new(StepWrapper::new(step)));
        SagaBuilder {
            steps,
            _phantom: PhantomData,
        }
    }
}

impl<Input, CurrentOutput, Ctx, Err>
    SagaBuilder<Input, CurrentOutput, Ctx, Err, HasSteps<CurrentOutput>>
{
    /// Add another step to the saga.
    ///
    /// The step's input type must match the current output type.
    #[must_use]
    pub fn then<S>(self, step: S) -> SagaBuilder<Input, S::Output, Ctx, Err, HasSteps<S::Output>>
    where
        S: SagaStep<Input = CurrentOutput, Context = Ctx, Error = Err> + 'static,
    {
        let mut steps = self.steps;
        steps.push(Box::new(StepWrapper::new(step)));
        SagaBuilder {
            steps,
            _phantom: PhantomData,
        }
    }

    /// Build the saga from the accumulated steps.
    #[must_use]
    pub fn build(self) -> Saga<Input, CurrentOutput, Ctx, Err>
    where
        Input: Clone + Send + 'static,
        CurrentOutput: Send + 'static,
        Err: Debug,
    {
        Saga::from_steps(self.steps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestContext;

    #[derive(Debug, PartialEq)]
    struct TestError(String);

    struct IntToString;

    impl SagaStep for IntToString {
        type Input = i32;
        type Output = String;
        type Context = TestContext;
        type Error = TestError;

        fn name(&self) -> &'static str {
            "int_to_string"
        }

        fn execute(
            &self,
            _ctx: &Self::Context,
            input: Self::Input,
        ) -> Result<Self::Output, Self::Error> {
            Ok(input.to_string())
        }
    }

    struct StringToLen;

    impl SagaStep for StringToLen {
        type Input = String;
        type Output = usize;
        type Context = TestContext;
        type Error = TestError;

        fn name(&self) -> &'static str {
            "string_to_len"
        }

        fn execute(
            &self,
            _ctx: &Self::Context,
            input: Self::Input,
        ) -> Result<Self::Output, Self::Error> {
            Ok(input.len())
        }
    }

    struct DoubleInt;

    impl SagaStep for DoubleInt {
        type Input = i32;
        type Output = i32;
        type Context = TestContext;
        type Error = TestError;

        fn name(&self) -> &'static str {
            "double_int"
        }

        fn execute(
            &self,
            _ctx: &Self::Context,
            input: Self::Input,
        ) -> Result<Self::Output, Self::Error> {
            Ok(input * 2)
        }
    }

    #[test]
    fn builder_creates_single_step_saga() {
        let _saga: Saga<i32, String, TestContext, TestError> =
            SagaBuilder::new().first_step(IntToString).build();
    }

    #[test]
    fn builder_chains_steps_with_matching_types() {
        let _saga: Saga<i32, usize, TestContext, TestError> = SagaBuilder::new()
            .first_step(IntToString)
            .then(StringToLen)
            .build();
    }

    #[test]
    fn builder_allows_multiple_steps_with_same_type() {
        let _saga: Saga<i32, i32, TestContext, TestError> = SagaBuilder::new()
            .first_step(DoubleInt)
            .then(DoubleInt)
            .then(DoubleInt)
            .build();
    }
}
