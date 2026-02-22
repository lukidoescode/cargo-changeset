//! Integration tests for saga compensation behavior.

use std::cell::RefCell;

use changeset_saga::{SagaBuilder, SagaError, SagaStep};

struct TestContext {
    compensation_log: RefCell<Vec<String>>,
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct TestError(String);

struct TrackedStep {
    name: &'static str,
    value: i32,
}

impl SagaStep for TrackedStep {
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
            .push(format!("compensate {}: input was {}", self.name, input));
        Ok(())
    }
}

struct FailingStep {
    error_message: String,
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
        Err(TestError(self.error_message.clone()))
    }
}

#[test]
fn compensation_happens_in_lifo_order() {
    let ctx = TestContext {
        compensation_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(TrackedStep {
            name: "step_a",
            value: 10,
        })
        .then(TrackedStep {
            name: "step_b",
            value: 20,
        })
        .then(TrackedStep {
            name: "step_c",
            value: 30,
        })
        .then(FailingStep {
            error_message: "boom".to_string(),
        })
        .build();

    let result = saga.execute(&ctx, 0);

    assert!(result.is_err());

    let log = ctx.compensation_log.borrow();
    assert_eq!(log.len(), 3);
    assert_eq!(log[0], "compensate step_c: input was 30");
    assert_eq!(log[1], "compensate step_b: input was 10");
    assert_eq!(log[2], "compensate step_a: input was 0");
}

#[test]
fn compensation_receives_original_input_not_output() {
    let ctx = TestContext {
        compensation_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(TrackedStep {
            name: "add_100",
            value: 100,
        })
        .then(TrackedStep {
            name: "add_50",
            value: 50,
        })
        .then(FailingStep {
            error_message: "trigger rollback".to_string(),
        })
        .build();

    let result = saga.execute(&ctx, 5);

    assert!(result.is_err());

    let log = ctx.compensation_log.borrow();
    assert_eq!(log[0], "compensate add_50: input was 105");
    assert_eq!(log[1], "compensate add_100: input was 5");
}

struct ReadOnlyStep {
    name: &'static str,
}

impl SagaStep for ReadOnlyStep {
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
        Ok(input * 2)
    }
}

#[test]
fn read_only_steps_have_no_op_compensation() {
    let ctx = TestContext {
        compensation_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(TrackedStep {
            name: "tracked",
            value: 10,
        })
        .then(ReadOnlyStep { name: "read_only" })
        .then(FailingStep {
            error_message: "fail".to_string(),
        })
        .build();

    let result = saga.execute(&ctx, 0);

    assert!(result.is_err());

    let log = ctx.compensation_log.borrow();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0], "compensate tracked: input was 0");
}

#[test]
fn mixed_compensation_and_read_only_steps() {
    let ctx = TestContext {
        compensation_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(TrackedStep {
            name: "tracked_1",
            value: 5,
        })
        .then(ReadOnlyStep {
            name: "read_only_1",
        })
        .then(TrackedStep {
            name: "tracked_2",
            value: 3,
        })
        .then(ReadOnlyStep {
            name: "read_only_2",
        })
        .then(TrackedStep {
            name: "tracked_3",
            value: 7,
        })
        .then(FailingStep {
            error_message: "abort".to_string(),
        })
        .build();

    let result = saga.execute(&ctx, 0);

    assert!(result.is_err());

    let log = ctx.compensation_log.borrow();
    assert_eq!(log.len(), 3);
    assert_eq!(log[0], "compensate tracked_3: input was 26");
    assert_eq!(log[1], "compensate tracked_2: input was 10");
    assert_eq!(log[2], "compensate tracked_1: input was 0");
}

#[test]
fn first_step_failure_triggers_no_compensation() {
    let ctx = TestContext {
        compensation_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(FailingStep {
            error_message: "immediate failure".to_string(),
        })
        .build();

    let result = saga.execute(&ctx, 42);

    assert!(result.is_err());
    assert!(ctx.compensation_log.borrow().is_empty());

    let err = result.expect_err("should be an error");
    match err {
        SagaError::StepFailed { step, .. } => {
            assert_eq!(step, "failing");
        }
        SagaError::CompensationFailed { .. } => {
            panic!("expected StepFailed, got CompensationFailed");
        }
        _ => panic!("unexpected error variant"),
    }
}

struct StringTransformStep {
    name: &'static str,
    suffix: &'static str,
}

impl SagaStep for StringTransformStep {
    type Input = String;
    type Output = String;
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
        Ok(format!("{}{}", input, self.suffix))
    }

    fn compensate(&self, ctx: &Self::Context, input: Self::Input) -> Result<(), Self::Error> {
        ctx.compensation_log
            .borrow_mut()
            .push(format!("compensate {}: input was '{}'", self.name, input));
        Ok(())
    }
}

struct FailingStringStep;

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
        Err(TestError("string step failed".to_string()))
    }
}

#[test]
fn compensation_with_string_inputs_preserves_original_values() {
    let ctx = TestContext {
        compensation_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(StringTransformStep {
            name: "append_a",
            suffix: "_A",
        })
        .then(StringTransformStep {
            name: "append_b",
            suffix: "_B",
        })
        .then(StringTransformStep {
            name: "append_c",
            suffix: "_C",
        })
        .then(FailingStringStep)
        .build();

    let result = saga.execute(&ctx, "start".to_string());

    assert!(result.is_err());

    let log = ctx.compensation_log.borrow();
    assert_eq!(log.len(), 3);
    assert_eq!(log[0], "compensate append_c: input was 'start_A_B'");
    assert_eq!(log[1], "compensate append_b: input was 'start_A'");
    assert_eq!(log[2], "compensate append_a: input was 'start'");
}
