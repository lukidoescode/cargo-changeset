//! Integration tests for compensation failure scenarios.

use std::cell::RefCell;

use changeset_saga::{SagaBuilder, SagaError, SagaStep};

struct TestContext {
    compensation_log: RefCell<Vec<String>>,
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct TestError(String);

struct SuccessfulStep {
    name: &'static str,
}

impl SagaStep for SuccessfulStep {
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
        Ok(input + 1)
    }

    fn compensate(&self, ctx: &Self::Context, input: Self::Input) -> Result<(), Self::Error> {
        ctx.compensation_log
            .borrow_mut()
            .push(format!("compensated {}: input={}", self.name, input));
        Ok(())
    }
}

struct FailingCompensationStep {
    name: &'static str,
    error_message: String,
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
        Ok(input + 1)
    }

    fn compensate(&self, ctx: &Self::Context, input: Self::Input) -> Result<(), Self::Error> {
        ctx.compensation_log.borrow_mut().push(format!(
            "failed to compensate {}: input={}",
            self.name, input
        ));
        Err(TestError(self.error_message.clone()))
    }
}

struct TriggerFailureStep;

impl SagaStep for TriggerFailureStep {
    type Input = i32;
    type Output = i32;
    type Context = TestContext;
    type Error = TestError;

    fn name(&self) -> &'static str {
        "trigger"
    }

    fn execute(
        &self,
        _ctx: &Self::Context,
        _input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        Err(TestError("triggered failure".to_string()))
    }
}

struct CustomDescriptionStep;

impl SagaStep for CustomDescriptionStep {
    type Input = i32;
    type Output = i32;
    type Context = TestContext;
    type Error = TestError;

    fn name(&self) -> &'static str {
        "custom_step"
    }

    fn execute(
        &self,
        _ctx: &Self::Context,
        input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        Ok(input)
    }

    fn compensate(&self, _ctx: &Self::Context, _input: Self::Input) -> Result<(), Self::Error> {
        Err(TestError("compensation failed".to_string()))
    }

    fn compensation_description(&self) -> String {
        "rollback custom operation".to_string()
    }
}

#[test]
fn compensation_failure_still_runs_other_compensations() {
    let ctx = TestContext {
        compensation_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(SuccessfulStep { name: "step_a" })
        .then(FailingCompensationStep {
            name: "step_b",
            error_message: "compensation b failed".to_string(),
        })
        .then(SuccessfulStep { name: "step_c" })
        .then(TriggerFailureStep)
        .build();

    let result = saga.execute(&ctx, 0);

    assert!(result.is_err());

    let log = ctx.compensation_log.borrow();
    assert_eq!(log.len(), 3);
    assert_eq!(log[0], "compensated step_c: input=2");
    assert_eq!(log[1], "failed to compensate step_b: input=1");
    assert_eq!(log[2], "compensated step_a: input=0");
}

#[test]
fn compensation_failed_error_contains_correct_information() {
    let ctx = TestContext {
        compensation_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(SuccessfulStep { name: "step_a" })
        .then(FailingCompensationStep {
            name: "step_b",
            error_message: "comp_b_error".to_string(),
        })
        .then(TriggerFailureStep)
        .build();

    let result = saga.execute(&ctx, 0);

    let err = result.expect_err("should be an error");
    match err {
        SagaError::CompensationFailed {
            failed_step,
            step_error,
            compensation_errors,
        } => {
            assert_eq!(failed_step, "trigger");
            assert_eq!(step_error.to_string(), "triggered failure");
            assert_eq!(compensation_errors.len(), 1);
            assert_eq!(compensation_errors[0].step, "step_b");
            assert_eq!(compensation_errors[0].error.to_string(), "comp_b_error");
        }
        SagaError::StepFailed { .. } => {
            panic!("expected CompensationFailed, got StepFailed");
        }
        _ => panic!("unexpected error variant"),
    }
}

#[test]
fn multiple_compensation_failures_are_all_reported() {
    let ctx = TestContext {
        compensation_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(FailingCompensationStep {
            name: "fail_comp_a",
            error_message: "comp_a_error".to_string(),
        })
        .then(SuccessfulStep { name: "success_b" })
        .then(FailingCompensationStep {
            name: "fail_comp_c",
            error_message: "comp_c_error".to_string(),
        })
        .then(SuccessfulStep { name: "success_d" })
        .then(FailingCompensationStep {
            name: "fail_comp_e",
            error_message: "comp_e_error".to_string(),
        })
        .then(TriggerFailureStep)
        .build();

    let result = saga.execute(&ctx, 0);

    let err = result.expect_err("should be an error");
    match err {
        SagaError::CompensationFailed {
            compensation_errors,
            ..
        } => {
            assert_eq!(compensation_errors.len(), 3);

            let error_steps: Vec<&str> = compensation_errors
                .iter()
                .map(|e| e.step.as_str())
                .collect();
            assert!(error_steps.contains(&"fail_comp_e"));
            assert!(error_steps.contains(&"fail_comp_c"));
            assert!(error_steps.contains(&"fail_comp_a"));

            let error_messages: Vec<&str> = compensation_errors
                .iter()
                .map(|e| e.error.0.as_str())
                .collect();
            assert!(error_messages.contains(&"comp_e_error"));
            assert!(error_messages.contains(&"comp_c_error"));
            assert!(error_messages.contains(&"comp_a_error"));
        }
        SagaError::StepFailed { .. } => {
            panic!("expected CompensationFailed, got StepFailed");
        }
        _ => panic!("unexpected error variant"),
    }

    let log = ctx.compensation_log.borrow();
    assert_eq!(log.len(), 5);
}

#[test]
fn successful_compensations_are_not_in_error_list() {
    let ctx = TestContext {
        compensation_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(SuccessfulStep { name: "success_a" })
        .then(FailingCompensationStep {
            name: "fail_comp_b",
            error_message: "only_this_fails".to_string(),
        })
        .then(SuccessfulStep { name: "success_c" })
        .then(TriggerFailureStep)
        .build();

    let result = saga.execute(&ctx, 0);

    let err = result.expect_err("should be an error");
    match err {
        SagaError::CompensationFailed {
            compensation_errors,
            ..
        } => {
            assert_eq!(compensation_errors.len(), 1);
            assert_eq!(compensation_errors[0].step, "fail_comp_b");
        }
        SagaError::StepFailed { .. } => {
            panic!("expected CompensationFailed, got StepFailed");
        }
        _ => panic!("unexpected error variant"),
    }
}

#[test]
fn compensation_error_description_is_populated() {
    let ctx = TestContext {
        compensation_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(CustomDescriptionStep)
        .then(TriggerFailureStep)
        .build();

    let result = saga.execute(&ctx, 0);

    let err = result.expect_err("should be an error");
    match err {
        SagaError::CompensationFailed {
            compensation_errors,
            ..
        } => {
            assert_eq!(
                compensation_errors[0].description,
                "rollback custom operation"
            );
        }
        SagaError::StepFailed { .. } => {
            panic!("expected CompensationFailed, got StepFailed");
        }
        _ => panic!("unexpected error variant"),
    }
}

#[test]
fn all_compensations_run_even_when_first_fails() {
    let ctx = TestContext {
        compensation_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(SuccessfulStep { name: "step_1" })
        .then(SuccessfulStep { name: "step_2" })
        .then(SuccessfulStep { name: "step_3" })
        .then(FailingCompensationStep {
            name: "step_4",
            error_message: "step_4 comp failed".to_string(),
        })
        .then(TriggerFailureStep)
        .build();

    let result = saga.execute(&ctx, 0);

    assert!(result.is_err());

    let log = ctx.compensation_log.borrow();
    assert_eq!(log.len(), 4);
    assert!(log[0].contains("step_4"));
    assert!(log[1].contains("step_3"));
    assert!(log[2].contains("step_2"));
    assert!(log[3].contains("step_1"));
}
