//! Integration tests for saga audit logging.

use std::cell::RefCell;

use changeset_saga::{SagaBuilder, SagaStep, StepStatus};

struct TestContext {
    log: RefCell<Vec<String>>,
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct TestError(String);

struct SimpleStep {
    name: &'static str,
}

impl SagaStep for SimpleStep {
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

    fn compensate(&self, ctx: &Self::Context, _input: Self::Input) -> Result<(), Self::Error> {
        ctx.log
            .borrow_mut()
            .push(format!("compensated {}", self.name));
        Ok(())
    }
}

struct FailingStep;

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
        Err(TestError("intentional failure".to_string()))
    }
}

#[test]
fn successful_execution_logs_all_steps_as_executed() -> anyhow::Result<()> {
    let ctx = TestContext {
        log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(SimpleStep { name: "step_a" })
        .then(SimpleStep { name: "step_b" })
        .then(SimpleStep { name: "step_c" })
        .build();

    let (result, audit_log) = saga.execute_with_audit(&ctx, 0);

    assert!(result.is_ok());
    assert_eq!(result?, 3);

    let records = audit_log.records();
    assert_eq!(records.len(), 3);

    assert_eq!(records[0].name, "step_a");
    assert_eq!(records[0].status, StepStatus::Executed);
    assert!(records[0].completed_at.is_some());

    assert_eq!(records[1].name, "step_b");
    assert_eq!(records[1].status, StepStatus::Executed);
    assert!(records[1].completed_at.is_some());

    assert_eq!(records[2].name, "step_c");
    assert_eq!(records[2].status, StepStatus::Executed);
    assert!(records[2].completed_at.is_some());

    Ok(())
}

#[test]
fn failed_execution_logs_failed_step_correctly() {
    let ctx = TestContext {
        log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(SimpleStep { name: "step_a" })
        .then(SimpleStep { name: "step_b" })
        .then(FailingStep)
        .build();

    let (result, audit_log) = saga.execute_with_audit(&ctx, 0);

    assert!(result.is_err());

    let records = audit_log.records();
    assert_eq!(records.len(), 3);

    assert_eq!(records[0].name, "step_a");
    assert_eq!(records[0].status, StepStatus::Compensated);

    assert_eq!(records[1].name, "step_b");
    assert_eq!(records[1].status, StepStatus::Compensated);

    assert_eq!(records[2].name, "failing");
    assert_eq!(records[2].status, StepStatus::Failed);
    assert!(records[2].completed_at.is_some());
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
        Ok(input + 1)
    }

    fn compensate(&self, _ctx: &Self::Context, _input: Self::Input) -> Result<(), Self::Error> {
        Err(TestError("compensation failed".to_string()))
    }
}

struct CustomDescriptionStep;

impl SagaStep for CustomDescriptionStep {
    type Input = i32;
    type Output = i32;
    type Context = TestContext;
    type Error = TestError;

    fn name(&self) -> &'static str {
        "custom"
    }

    fn execute(
        &self,
        _ctx: &Self::Context,
        input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        Ok(input)
    }

    fn compensation_description(&self) -> String {
        "revert custom changes".to_string()
    }
}

#[test]
fn compensation_failure_logged_correctly() {
    let ctx = TestContext {
        log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(SimpleStep { name: "step_a" })
        .then(FailingCompensationStep { name: "step_b" })
        .then(FailingStep)
        .build();

    let (result, audit_log) = saga.execute_with_audit(&ctx, 0);

    assert!(result.is_err());

    let records = audit_log.records();
    assert_eq!(records.len(), 3);

    assert_eq!(records[0].name, "step_a");
    assert_eq!(records[0].status, StepStatus::Compensated);

    assert_eq!(records[1].name, "step_b");
    assert_eq!(records[1].status, StepStatus::CompensationFailed);

    assert_eq!(records[2].name, "failing");
    assert_eq!(records[2].status, StepStatus::Failed);
}

#[test]
fn execute_with_audit_returns_usable_result_and_log() -> anyhow::Result<()> {
    let ctx = TestContext {
        log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(SimpleStep { name: "increment" })
        .build();

    let (result, audit_log) = saga.execute_with_audit(&ctx, 99);

    let value = result?;
    assert_eq!(value, 100);

    assert_eq!(audit_log.records().len(), 1);
    assert_eq!(audit_log.records()[0].name, "increment");

    Ok(())
}

#[test]
fn audit_log_records_compensation_descriptions() {
    let ctx = TestContext {
        log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new().first_step(CustomDescriptionStep).build();

    let (result, audit_log) = saga.execute_with_audit(&ctx, 42);

    assert!(result.is_ok());

    let records = audit_log.records();
    assert_eq!(
        records[0].compensation_description,
        Some("revert custom changes".to_string())
    );
}

#[test]
fn audit_log_timing_is_populated() {
    let ctx = TestContext {
        log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(SimpleStep { name: "timed" })
        .build();

    let (result, audit_log) = saga.execute_with_audit(&ctx, 0);

    assert!(result.is_ok());

    let records = audit_log.records();
    assert_eq!(records.len(), 1);

    let record = &records[0];
    assert!(record.completed_at.is_some());

    let completed_at = record.completed_at.expect("should have completed_at");
    assert!(completed_at >= record.started_at);
}

#[test]
fn audit_log_summary_contains_all_step_names() {
    let ctx = TestContext {
        log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(SimpleStep { name: "first" })
        .then(SimpleStep { name: "second" })
        .then(SimpleStep { name: "third" })
        .build();

    let (_, audit_log) = saga.execute_with_audit(&ctx, 0);

    let summary = audit_log.summary();

    assert!(summary.contains("first"));
    assert!(summary.contains("second"));
    assert!(summary.contains("third"));
}

#[test]
fn audit_log_summary_shows_status_indicators() {
    let ctx = TestContext {
        log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(SimpleStep {
            name: "will_compensate",
        })
        .then(FailingStep)
        .build();

    let (_, audit_log) = saga.execute_with_audit(&ctx, 0);

    let summary = audit_log.summary();

    assert!(summary.contains("will_compensate"));
    assert!(summary.contains("failing"));
}

#[test]
fn single_step_failure_shows_correct_audit() {
    let ctx = TestContext {
        log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new().first_step(FailingStep).build();

    let (result, audit_log) = saga.execute_with_audit(&ctx, 0);

    assert!(result.is_err());

    let records = audit_log.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].name, "failing");
    assert_eq!(records[0].status, StepStatus::Failed);
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
        Ok(input)
    }
}

#[test]
fn read_only_steps_are_logged_in_audit() {
    let ctx = TestContext {
        log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(ReadOnlyStep {
            name: "read_only_a",
        })
        .then(ReadOnlyStep {
            name: "read_only_b",
        })
        .then(FailingStep)
        .build();

    let (result, audit_log) = saga.execute_with_audit(&ctx, 0);

    assert!(result.is_err());

    let records = audit_log.records();
    assert_eq!(records.len(), 3);

    assert_eq!(records[0].name, "read_only_a");
    assert_eq!(records[0].status, StepStatus::Compensated);

    assert_eq!(records[1].name, "read_only_b");
    assert_eq!(records[1].status, StepStatus::Compensated);

    assert_eq!(records[2].name, "failing");
    assert_eq!(records[2].status, StepStatus::Failed);
}
