//! Integration tests for conditional steps that may skip their main logic.

use std::cell::RefCell;

use changeset_saga::{SagaBuilder, SagaStep};

struct TestContext {
    operations_log: RefCell<Vec<String>>,
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct TestError(String);

#[derive(Clone)]
struct WriteRequest {
    data: String,
    should_write: bool,
}

struct ConditionalWriteStep;

impl SagaStep for ConditionalWriteStep {
    type Input = WriteRequest;
    type Output = WriteRequest;
    type Context = TestContext;
    type Error = TestError;

    fn name(&self) -> &'static str {
        "conditional_write"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        if input.should_write {
            ctx.operations_log
                .borrow_mut()
                .push(format!("wrote: {}", input.data));
        } else {
            ctx.operations_log
                .borrow_mut()
                .push("skipped write".to_string());
        }
        Ok(input)
    }

    fn compensate(&self, ctx: &Self::Context, input: Self::Input) -> Result<(), Self::Error> {
        if input.should_write {
            ctx.operations_log
                .borrow_mut()
                .push(format!("compensate write: {}", input.data));
        }
        Ok(())
    }
}

struct TransformDataStep;

impl SagaStep for TransformDataStep {
    type Input = WriteRequest;
    type Output = WriteRequest;
    type Context = TestContext;
    type Error = TestError;

    fn name(&self) -> &'static str {
        "transform_data"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        ctx.operations_log
            .borrow_mut()
            .push("transformed data".to_string());
        Ok(WriteRequest {
            data: format!("transformed_{}", input.data),
            should_write: input.should_write,
        })
    }
}

struct FailStep;

impl SagaStep for FailStep {
    type Input = WriteRequest;
    type Output = WriteRequest;
    type Context = TestContext;
    type Error = TestError;

    fn name(&self) -> &'static str {
        "fail"
    }

    fn execute(
        &self,
        _ctx: &Self::Context,
        _input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        Err(TestError("intentional failure".to_string()))
    }
}

#[test]
fn conditional_step_executes_when_condition_true() -> anyhow::Result<()> {
    let ctx = TestContext {
        operations_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(ConditionalWriteStep)
        .then(TransformDataStep)
        .build();

    let input = WriteRequest {
        data: "test_data".to_string(),
        should_write: true,
    };

    let result = saga.execute(&ctx, input)?;

    assert_eq!(result.data, "transformed_test_data");

    let log = ctx.operations_log.borrow();
    assert_eq!(log.len(), 2);
    assert_eq!(log[0], "wrote: test_data");
    assert_eq!(log[1], "transformed data");

    Ok(())
}

#[test]
fn conditional_step_skips_when_condition_false() -> anyhow::Result<()> {
    let ctx = TestContext {
        operations_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(ConditionalWriteStep)
        .then(TransformDataStep)
        .build();

    let input = WriteRequest {
        data: "test_data".to_string(),
        should_write: false,
    };

    let result = saga.execute(&ctx, input)?;

    assert_eq!(result.data, "transformed_test_data");

    let log = ctx.operations_log.borrow();
    assert_eq!(log.len(), 2);
    assert_eq!(log[0], "skipped write");
    assert_eq!(log[1], "transformed data");

    Ok(())
}

#[test]
fn conditional_step_compensation_respects_condition() {
    let ctx = TestContext {
        operations_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(ConditionalWriteStep)
        .then(TransformDataStep)
        .then(FailStep)
        .build();

    let input = WriteRequest {
        data: "test_data".to_string(),
        should_write: true,
    };

    let result = saga.execute(&ctx, input);

    assert!(result.is_err());

    let log = ctx.operations_log.borrow();
    assert!(log.contains(&"compensate write: test_data".to_string()));
}

#[test]
fn conditional_step_no_compensation_when_skipped() {
    let ctx = TestContext {
        operations_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(ConditionalWriteStep)
        .then(TransformDataStep)
        .then(FailStep)
        .build();

    let input = WriteRequest {
        data: "test_data".to_string(),
        should_write: false,
    };

    let result = saga.execute(&ctx, input);

    assert!(result.is_err());

    let log = ctx.operations_log.borrow();
    assert!(
        !log.iter()
            .any(|entry| entry.starts_with("compensate write"))
    );
}

#[derive(Clone)]
struct MultiConditionInput {
    value: i32,
    enable_logging: bool,
    enable_validation: bool,
    enable_notification: bool,
}

struct OptionalLoggingStep;

impl SagaStep for OptionalLoggingStep {
    type Input = MultiConditionInput;
    type Output = MultiConditionInput;
    type Context = TestContext;
    type Error = TestError;

    fn name(&self) -> &'static str {
        "optional_logging"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        if input.enable_logging {
            ctx.operations_log
                .borrow_mut()
                .push(format!("logged value: {}", input.value));
        }
        Ok(input)
    }
}

struct OptionalValidationStep;

impl SagaStep for OptionalValidationStep {
    type Input = MultiConditionInput;
    type Output = MultiConditionInput;
    type Context = TestContext;
    type Error = TestError;

    fn name(&self) -> &'static str {
        "optional_validation"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        if input.enable_validation {
            if input.value < 0 {
                return Err(TestError("validation failed: negative value".to_string()));
            }
            ctx.operations_log
                .borrow_mut()
                .push("validation passed".to_string());
        }
        Ok(input)
    }
}

struct OptionalNotificationStep;

impl SagaStep for OptionalNotificationStep {
    type Input = MultiConditionInput;
    type Output = MultiConditionInput;
    type Context = TestContext;
    type Error = TestError;

    fn name(&self) -> &'static str {
        "optional_notification"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        if input.enable_notification {
            ctx.operations_log
                .borrow_mut()
                .push("notification sent".to_string());
        }
        Ok(input)
    }
}

#[test]
fn multiple_conditional_steps_with_mixed_conditions() -> anyhow::Result<()> {
    let ctx = TestContext {
        operations_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(OptionalLoggingStep)
        .then(OptionalValidationStep)
        .then(OptionalNotificationStep)
        .build();

    let input = MultiConditionInput {
        value: 42,
        enable_logging: true,
        enable_validation: false,
        enable_notification: true,
    };

    let result = saga.execute(&ctx, input)?;

    assert_eq!(result.value, 42);

    let log = ctx.operations_log.borrow();
    assert_eq!(log.len(), 2);
    assert_eq!(log[0], "logged value: 42");
    assert_eq!(log[1], "notification sent");

    Ok(())
}

#[test]
fn all_conditions_disabled_passes_through_unchanged() -> anyhow::Result<()> {
    let ctx = TestContext {
        operations_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(OptionalLoggingStep)
        .then(OptionalValidationStep)
        .then(OptionalNotificationStep)
        .build();

    let input = MultiConditionInput {
        value: 100,
        enable_logging: false,
        enable_validation: false,
        enable_notification: false,
    };

    let result = saga.execute(&ctx, input)?;

    assert_eq!(result.value, 100);
    assert!(ctx.operations_log.borrow().is_empty());

    Ok(())
}

#[test]
fn conditional_validation_step_can_fail() {
    let ctx = TestContext {
        operations_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(OptionalLoggingStep)
        .then(OptionalValidationStep)
        .then(OptionalNotificationStep)
        .build();

    let input = MultiConditionInput {
        value: -5,
        enable_logging: true,
        enable_validation: true,
        enable_notification: true,
    };

    let result = saga.execute(&ctx, input);

    assert!(result.is_err());

    let log = ctx.operations_log.borrow();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0], "logged value: -5");
}

#[derive(Clone)]
struct CounterInput {
    count: usize,
    max_iterations: usize,
}

struct IterativeStep;

impl SagaStep for IterativeStep {
    type Input = CounterInput;
    type Output = CounterInput;
    type Context = TestContext;
    type Error = TestError;

    fn name(&self) -> &'static str {
        "iterative"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        if input.count < input.max_iterations {
            ctx.operations_log
                .borrow_mut()
                .push(format!("iteration {}", input.count));
        }
        Ok(CounterInput {
            count: input.count + 1,
            max_iterations: input.max_iterations,
        })
    }
}

struct FinalizeStep;

impl SagaStep for FinalizeStep {
    type Input = CounterInput;
    type Output = usize;
    type Context = TestContext;
    type Error = TestError;

    fn name(&self) -> &'static str {
        "finalize"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        ctx.operations_log
            .borrow_mut()
            .push("finalized".to_string());
        Ok(input.count)
    }
}

#[test]
fn steps_can_conditionally_increment_and_stop() -> anyhow::Result<()> {
    let ctx = TestContext {
        operations_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(IterativeStep)
        .then(IterativeStep)
        .then(IterativeStep)
        .then(FinalizeStep)
        .build();

    let input = CounterInput {
        count: 0,
        max_iterations: 2,
    };

    let result = saga.execute(&ctx, input)?;

    assert_eq!(result, 3);

    let log = ctx.operations_log.borrow();
    assert_eq!(log.len(), 3);
    assert_eq!(log[0], "iteration 0");
    assert_eq!(log[1], "iteration 1");
    assert_eq!(log[2], "finalized");

    Ok(())
}
