use std::fmt::Debug;
use std::marker::PhantomData;

use crate::audit::SagaAuditLog;
use crate::cloneable::CloneableAny;
use crate::erased::ErasedStep;
use crate::error::{CompensationError, SagaError};

/// A compiled saga ready for execution.
///
/// Sagas execute a sequence of steps, where each step's output becomes the
/// next step's input. If any step fails, previously completed steps are
/// compensated in reverse order (LIFO).
pub struct Saga<Input, Output, Ctx, Err> {
    steps: Vec<Box<dyn ErasedStep<Ctx, Err>>>,
    _phantom: PhantomData<(Input, Output)>,
}

impl<Input, Output, Ctx, Err> Saga<Input, Output, Ctx, Err>
where
    Input: Clone + Send + 'static,
    Output: Send + 'static,
    Err: Debug,
{
    pub(crate) fn from_steps(steps: Vec<Box<dyn ErasedStep<Ctx, Err>>>) -> Self {
        Self {
            steps,
            _phantom: PhantomData,
        }
    }

    /// Execute the saga, returning the final output on success.
    ///
    /// On failure, compensates all previously completed steps in reverse order.
    ///
    /// # Errors
    ///
    /// Returns `SagaError::StepFailed` if a step fails and all compensations succeed.
    /// Returns `SagaError::CompensationFailed` if a step fails and some compensations also fail.
    pub fn execute(&self, ctx: &Ctx, input: Input) -> Result<Output, SagaError<Err>> {
        let (result, _audit_log) = self.execute_internal(ctx, input);
        result
    }

    /// Execute the saga and return both the result and an audit log.
    ///
    /// The audit log tracks all step executions and compensations.
    pub fn execute_with_audit(
        &self,
        ctx: &Ctx,
        input: Input,
    ) -> (Result<Output, SagaError<Err>>, SagaAuditLog) {
        self.execute_internal(ctx, input)
    }

    fn execute_internal(
        &self,
        ctx: &Ctx,
        input: Input,
    ) -> (Result<Output, SagaError<Err>>, SagaAuditLog) {
        let mut audit_log = SagaAuditLog::new();
        let mut compensation_stack: Vec<(usize, Box<dyn CloneableAny>)> = Vec::new();

        let mut current_input: Box<dyn CloneableAny> = Box::new(input);

        for (index, step) in self.steps.iter().enumerate() {
            audit_log.record_start(step.name());

            let input_clone = current_input.clone_box();

            match step.execute_erased(ctx, current_input) {
                Ok(output) => {
                    let description = step.compensation_description();
                    audit_log.record_success(description);
                    compensation_stack.push((index, input_clone));

                    if index == self.steps.len() - 1 {
                        let typed_output = output
                            .into_any()
                            .downcast::<Output>()
                            .expect("type-state builder guarantees final output type");
                        return (Ok(*typed_output), audit_log);
                    }

                    current_input = output;
                }
                Err(error) => {
                    audit_log.record_failure();
                    let saga_error = self.compensate(
                        ctx,
                        &mut audit_log,
                        compensation_stack,
                        step.name(),
                        error,
                    );
                    return (Err(saga_error), audit_log);
                }
            }
        }

        unreachable!("saga must have at least one step")
    }

    fn compensate(
        &self,
        ctx: &Ctx,
        audit_log: &mut SagaAuditLog,
        mut compensation_stack: Vec<(usize, Box<dyn CloneableAny>)>,
        failed_step: &str,
        step_error: Err,
    ) -> SagaError<Err> {
        let mut compensation_errors = Vec::new();

        while let Some((index, stored_input)) = compensation_stack.pop() {
            let step = &self.steps[index];
            let step_name = step.name();
            let description = step.compensation_description();

            match step.compensate_erased(ctx, stored_input) {
                Ok(()) => {
                    audit_log.record_compensated(step_name);
                }
                Err(error) => {
                    audit_log.record_compensation_failed(step_name);
                    compensation_errors.push(CompensationError {
                        step: step_name.to_string(),
                        description,
                        error,
                    });
                }
            }
        }

        if compensation_errors.is_empty() {
            SagaError::StepFailed {
                step: failed_step.to_string(),
                source: step_error,
            }
        } else {
            SagaError::CompensationFailed {
                failed_step: failed_step.to_string(),
                step_error,
                compensation_errors,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::audit::StepStatus;
    use crate::builder::SagaBuilder;
    use crate::step::SagaStep;

    struct TestContext {
        compensation_log: RefCell<Vec<String>>,
    }

    #[derive(Debug, PartialEq, thiserror::Error)]
    #[error("{0}")]
    struct TestError(String);

    struct AddStep {
        name: &'static str,
        value: i32,
    }

    impl SagaStep for AddStep {
        type Input = i32;
        type Output = i32;
        type Context = TestContext;
        type Error = TestError;

        fn name(&self) -> &'static str {
            self.name
        }

        fn execute(
            &self,
            _ctx: &Self::Context,
            input: Self::Input,
        ) -> Result<Self::Output, Self::Error> {
            Ok(input + self.value)
        }

        fn compensate(&self, ctx: &Self::Context, input: Self::Input) -> Result<(), Self::Error> {
            ctx.compensation_log
                .borrow_mut()
                .push(format!("compensate {} with input {}", self.name, input));
            Ok(())
        }
    }

    struct MultiplyStep {
        factor: i32,
    }

    impl SagaStep for MultiplyStep {
        type Input = i32;
        type Output = i32;
        type Context = TestContext;
        type Error = TestError;

        fn name(&self) -> &'static str {
            "multiply"
        }

        fn execute(
            &self,
            _ctx: &Self::Context,
            input: Self::Input,
        ) -> Result<Self::Output, Self::Error> {
            Ok(input * self.factor)
        }

        fn compensate(&self, ctx: &Self::Context, input: Self::Input) -> Result<(), Self::Error> {
            ctx.compensation_log
                .borrow_mut()
                .push(format!("compensate multiply with input {input}"));
            Ok(())
        }
    }

    struct FailingStep {
        error_msg: String,
    }

    impl SagaStep for FailingStep {
        type Input = i32;
        type Output = i32;
        type Context = TestContext;
        type Error = TestError;

        fn name(&self) -> &'static str {
            "failing"
        }

        fn execute(
            &self,
            _ctx: &Self::Context,
            _input: Self::Input,
        ) -> Result<Self::Output, Self::Error> {
            Err(TestError(self.error_msg.clone()))
        }
    }

    struct FailingCompensationStep {
        name: &'static str,
    }

    impl SagaStep for FailingCompensationStep {
        type Input = i32;
        type Output = i32;
        type Context = TestContext;
        type Error = TestError;

        fn name(&self) -> &'static str {
            self.name
        }

        fn execute(
            &self,
            _ctx: &Self::Context,
            input: Self::Input,
        ) -> Result<Self::Output, Self::Error> {
            Ok(input)
        }

        fn compensate(&self, _ctx: &Self::Context, _input: Self::Input) -> Result<(), Self::Error> {
            Err(TestError(format!("compensation failed for {}", self.name)))
        }
    }

    struct ReadOnlyStep;

    impl SagaStep for ReadOnlyStep {
        type Input = i32;
        type Output = i32;
        type Context = TestContext;
        type Error = TestError;

        fn name(&self) -> &'static str {
            "read_only"
        }

        fn execute(
            &self,
            _ctx: &Self::Context,
            input: Self::Input,
        ) -> Result<Self::Output, Self::Error> {
            Ok(input)
        }
    }

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

        fn compensate(&self, ctx: &Self::Context, input: Self::Input) -> Result<(), Self::Error> {
            ctx.compensation_log
                .borrow_mut()
                .push(format!("compensate int_to_string with input {input}"));
            Ok(())
        }
    }

    struct AppendSuffix {
        suffix: &'static str,
    }

    impl SagaStep for AppendSuffix {
        type Input = String;
        type Output = String;
        type Context = TestContext;
        type Error = TestError;

        fn name(&self) -> &'static str {
            "append_suffix"
        }

        fn execute(
            &self,
            _ctx: &Self::Context,
            input: Self::Input,
        ) -> Result<Self::Output, Self::Error> {
            Ok(format!("{}{}", input, self.suffix))
        }

        fn compensate(&self, ctx: &Self::Context, input: Self::Input) -> Result<(), Self::Error> {
            ctx.compensation_log
                .borrow_mut()
                .push(format!("compensate append_suffix with input {input}"));
            Ok(())
        }
    }

    struct FailingStringStep {
        error_msg: String,
    }

    impl SagaStep for FailingStringStep {
        type Input = String;
        type Output = String;
        type Context = TestContext;
        type Error = TestError;

        fn name(&self) -> &'static str {
            "failing_string"
        }

        fn execute(
            &self,
            _ctx: &Self::Context,
            _input: Self::Input,
        ) -> Result<Self::Output, Self::Error> {
            Err(TestError(self.error_msg.clone()))
        }
    }

    #[test]
    fn multi_step_saga_flows_data_through_steps() -> anyhow::Result<()> {
        let ctx = TestContext {
            compensation_log: RefCell::new(Vec::new()),
        };

        let saga = SagaBuilder::new()
            .first_step(AddStep {
                name: "add_10",
                value: 10,
            })
            .then(MultiplyStep { factor: 3 })
            .then(AddStep {
                name: "add_5",
                value: 5,
            })
            .build();

        let result = saga.execute(&ctx, 5)?;

        assert_eq!(result, 50);
        Ok(())
    }

    #[test]
    fn compensation_happens_in_lifo_order_with_stored_inputs() {
        let ctx = TestContext {
            compensation_log: RefCell::new(Vec::new()),
        };

        let saga = SagaBuilder::new()
            .first_step(AddStep {
                name: "add_10",
                value: 10,
            })
            .then(MultiplyStep { factor: 3 })
            .then(FailingStep {
                error_msg: "boom".to_string(),
            })
            .build();

        let result = saga.execute(&ctx, 5);

        assert!(result.is_err());

        let comp_log = ctx.compensation_log.borrow();
        assert_eq!(comp_log.len(), 2);
        assert_eq!(comp_log[0], "compensate multiply with input 15");
        assert_eq!(comp_log[1], "compensate add_10 with input 5");
    }

    #[test]
    fn read_only_step_uses_default_no_op_compensation() {
        let ctx = TestContext {
            compensation_log: RefCell::new(Vec::new()),
        };

        let saga = SagaBuilder::new()
            .first_step(ReadOnlyStep)
            .then(FailingStep {
                error_msg: "boom".to_string(),
            })
            .build();

        let result = saga.execute(&ctx, 42);

        assert!(result.is_err());
        let comp_log = ctx.compensation_log.borrow();
        assert!(comp_log.is_empty());
    }

    #[test]
    fn first_step_failure_requires_no_compensation() {
        let ctx = TestContext {
            compensation_log: RefCell::new(Vec::new()),
        };

        let saga = SagaBuilder::new()
            .first_step(FailingStep {
                error_msg: "immediate failure".to_string(),
            })
            .build();

        let result = saga.execute(&ctx, 42);

        assert!(result.is_err());
        let err = result.expect_err("should be an error");
        assert!(matches!(err, SagaError::StepFailed { step, .. } if step == "failing"));

        let comp_log = ctx.compensation_log.borrow();
        assert!(comp_log.is_empty());
    }

    #[test]
    fn compensation_failure_returns_compensation_failed_error() {
        let ctx = TestContext {
            compensation_log: RefCell::new(Vec::new()),
        };

        let saga = SagaBuilder::new()
            .first_step(AddStep {
                name: "add_10",
                value: 10,
            })
            .then(FailingCompensationStep {
                name: "will_fail_comp",
            })
            .then(FailingStep {
                error_msg: "trigger compensation".to_string(),
            })
            .build();

        let result = saga.execute(&ctx, 5);

        let err = result.expect_err("should be an error");
        match err {
            SagaError::CompensationFailed {
                failed_step,
                compensation_errors,
                ..
            } => {
                assert_eq!(failed_step, "failing");
                assert_eq!(compensation_errors.len(), 1);
                assert_eq!(compensation_errors[0].step, "will_fail_comp");
            }
            SagaError::StepFailed { .. } => {
                panic!("expected CompensationFailed error");
            }
        }

        let comp_log = ctx.compensation_log.borrow();
        assert_eq!(comp_log.len(), 1);
        assert_eq!(comp_log[0], "compensate add_10 with input 5");
    }

    #[test]
    fn execute_with_audit_returns_audit_log() -> anyhow::Result<()> {
        let ctx = TestContext {
            compensation_log: RefCell::new(Vec::new()),
        };

        let saga = SagaBuilder::new()
            .first_step(AddStep {
                name: "add_10",
                value: 10,
            })
            .then(MultiplyStep { factor: 2 })
            .build();

        let (result, audit_log) = saga.execute_with_audit(&ctx, 5);

        assert!(result.is_ok());
        assert_eq!(result?, 30);

        let records = audit_log.records();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].name, "add_10");
        assert_eq!(records[0].status, StepStatus::Executed);
        assert_eq!(records[1].name, "multiply");
        assert_eq!(records[1].status, StepStatus::Executed);

        Ok(())
    }

    #[test]
    fn audit_log_tracks_compensation_status() {
        let ctx = TestContext {
            compensation_log: RefCell::new(Vec::new()),
        };

        let saga = SagaBuilder::new()
            .first_step(AddStep {
                name: "add_10",
                value: 10,
            })
            .then(FailingCompensationStep {
                name: "will_fail_comp",
            })
            .then(FailingStep {
                error_msg: "trigger compensation".to_string(),
            })
            .build();

        let (result, audit_log) = saga.execute_with_audit(&ctx, 5);

        assert!(result.is_err());

        let records = audit_log.records();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].name, "add_10");
        assert_eq!(records[0].status, StepStatus::Compensated);
        assert_eq!(records[1].name, "will_fail_comp");
        assert_eq!(records[1].status, StepStatus::CompensationFailed);
        assert_eq!(records[2].name, "failing");
        assert_eq!(records[2].status, StepStatus::Failed);
    }

    #[test]
    fn typed_data_flow_across_different_types() -> anyhow::Result<()> {
        let ctx = TestContext {
            compensation_log: RefCell::new(Vec::new()),
        };

        let saga = SagaBuilder::new()
            .first_step(IntToString)
            .then(AppendSuffix { suffix: "_suffix" })
            .build();

        let result = saga.execute(&ctx, 42)?;

        assert_eq!(result, "42_suffix");
        Ok(())
    }

    #[test]
    fn compensation_with_different_types_uses_correct_inputs() {
        let ctx = TestContext {
            compensation_log: RefCell::new(Vec::new()),
        };

        let saga = SagaBuilder::new()
            .first_step(IntToString)
            .then(AppendSuffix { suffix: "_suffix" })
            .then(FailingStringStep {
                error_msg: "boom".to_string(),
            })
            .build();

        let result = saga.execute(&ctx, 42);

        assert!(result.is_err());

        let comp_log = ctx.compensation_log.borrow();
        assert_eq!(comp_log.len(), 2);
        assert_eq!(comp_log[0], "compensate append_suffix with input 42");
        assert_eq!(comp_log[1], "compensate int_to_string with input 42");
    }
}
