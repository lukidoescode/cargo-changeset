//! Integration tests for typed data flow between saga steps.

use std::cell::RefCell;

use changeset_saga::{SagaBuilder, SagaStep};

struct TestContext {
    execution_log: RefCell<Vec<String>>,
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct TestError(String);

struct IdentityStep {
    name: &'static str,
}

impl SagaStep for IdentityStep {
    type Input = String;
    type Output = String;
    type Context = TestContext;
    type Error = TestError;

    fn name(&self) -> &'static str {
        self.name
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        ctx.execution_log
            .borrow_mut()
            .push(format!("{}: received '{}'", self.name, input));
        Ok(input)
    }
}

#[test]
fn single_step_saga_returns_correct_output() -> anyhow::Result<()> {
    let ctx = TestContext {
        execution_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(IdentityStep { name: "only" })
        .build();

    let result = saga.execute(&ctx, "hello".to_string())?;

    assert_eq!(result, "hello");
    assert_eq!(ctx.execution_log.borrow().len(), 1);
    assert_eq!(ctx.execution_log.borrow()[0], "only: received 'hello'");

    Ok(())
}

struct ParseIntStep;

impl SagaStep for ParseIntStep {
    type Input = String;
    type Output = i32;
    type Context = TestContext;
    type Error = TestError;

    fn name(&self) -> &'static str {
        "parse_int"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        ctx.execution_log
            .borrow_mut()
            .push(format!("parse_int: parsing '{input}'"));
        input
            .parse()
            .map_err(|e| TestError(format!("parse error: {e}")))
    }
}

struct DoubleStep;

impl SagaStep for DoubleStep {
    type Input = i32;
    type Output = i32;
    type Context = TestContext;
    type Error = TestError;

    fn name(&self) -> &'static str {
        "double"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        ctx.execution_log
            .borrow_mut()
            .push(format!("double: received {input}"));
        Ok(input * 2)
    }
}

struct FormatWithLabelStep {
    label: &'static str,
}

impl SagaStep for FormatWithLabelStep {
    type Input = i32;
    type Output = (i32, String);
    type Context = TestContext;
    type Error = TestError;

    fn name(&self) -> &'static str {
        "format_with_label"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        ctx.execution_log
            .borrow_mut()
            .push(format!("format_with_label: received {input}"));
        Ok((input, format!("{}: {}", self.label, input)))
    }
}

#[test]
fn multi_step_saga_transforms_types_correctly() -> anyhow::Result<()> {
    let ctx = TestContext {
        execution_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(ParseIntStep)
        .then(DoubleStep)
        .then(FormatWithLabelStep { label: "result" })
        .build();

    let result = saga.execute(&ctx, "21".to_string())?;

    assert_eq!(result, (42, "result: 42".to_string()));

    let log = ctx.execution_log.borrow();
    assert_eq!(log.len(), 3);
    assert_eq!(log[0], "parse_int: parsing '21'");
    assert_eq!(log[1], "double: received 21");
    assert_eq!(log[2], "format_with_label: received 42");

    Ok(())
}

struct AddStep {
    value: i32,
}

impl SagaStep for AddStep {
    type Input = i32;
    type Output = i32;
    type Context = TestContext;
    type Error = TestError;

    fn name(&self) -> &'static str {
        "add"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        let output = input + self.value;
        ctx.execution_log
            .borrow_mut()
            .push(format!("add: {} + {} = {}", input, self.value, output));
        Ok(output)
    }
}

#[test]
fn each_step_receives_previous_step_output() -> anyhow::Result<()> {
    let ctx = TestContext {
        execution_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(AddStep { value: 10 })
        .then(AddStep { value: 5 })
        .then(AddStep { value: 3 })
        .build();

    let result = saga.execute(&ctx, 0)?;

    assert_eq!(result, 18);

    let log = ctx.execution_log.borrow();
    assert_eq!(log.len(), 3);
    assert_eq!(log[0], "add: 0 + 10 = 10");
    assert_eq!(log[1], "add: 10 + 5 = 15");
    assert_eq!(log[2], "add: 15 + 3 = 18");

    Ok(())
}

struct StringToVecStep;

impl SagaStep for StringToVecStep {
    type Input = String;
    type Output = Vec<char>;
    type Context = TestContext;
    type Error = TestError;

    fn name(&self) -> &'static str {
        "string_to_vec"
    }

    fn execute(
        &self,
        _ctx: &Self::Context,
        input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        Ok(input.chars().collect())
    }
}

struct VecLengthStep;

impl SagaStep for VecLengthStep {
    type Input = Vec<char>;
    type Output = usize;
    type Context = TestContext;
    type Error = TestError;

    fn name(&self) -> &'static str {
        "vec_length"
    }

    fn execute(
        &self,
        _ctx: &Self::Context,
        input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        Ok(input.len())
    }
}

struct UsizeToStringStep;

impl SagaStep for UsizeToStringStep {
    type Input = usize;
    type Output = String;
    type Context = TestContext;
    type Error = TestError;

    fn name(&self) -> &'static str {
        "usize_to_string"
    }

    fn execute(
        &self,
        _ctx: &Self::Context,
        input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        Ok(format!("length={input}"))
    }
}

#[test]
fn complex_type_chain_works_correctly() -> anyhow::Result<()> {
    let ctx = TestContext {
        execution_log: RefCell::new(Vec::new()),
    };

    let saga = SagaBuilder::new()
        .first_step(StringToVecStep)
        .then(VecLengthStep)
        .then(UsizeToStringStep)
        .build();

    let result = saga.execute(&ctx, "hello".to_string())?;

    assert_eq!(result, "length=5");

    Ok(())
}
