use crate::cloneable::CloneableAny;
use crate::step::SagaStep;

pub(crate) trait ErasedStep<Ctx, Err> {
    fn name(&self) -> &'static str;

    fn execute_erased(
        &self,
        ctx: &Ctx,
        input: Box<dyn CloneableAny>,
    ) -> Result<Box<dyn CloneableAny>, Err>;

    fn compensate_erased(&self, ctx: &Ctx, input: Box<dyn CloneableAny>) -> Result<(), Err>;

    fn compensation_description(&self) -> String;
}

pub(crate) struct StepWrapper<S> {
    step: S,
}

impl<S> StepWrapper<S> {
    pub(crate) fn new(step: S) -> Self {
        Self { step }
    }
}

impl<S> ErasedStep<S::Context, S::Error> for StepWrapper<S>
where
    S: SagaStep,
{
    fn name(&self) -> &'static str {
        self.step.name()
    }

    fn execute_erased(
        &self,
        ctx: &S::Context,
        input: Box<dyn CloneableAny>,
    ) -> Result<Box<dyn CloneableAny>, S::Error> {
        let typed_input = input
            .into_any()
            .downcast::<S::Input>()
            .expect("type-state builder guarantees correct input type");
        let output = self.step.execute(ctx, *typed_input)?;
        Ok(Box::new(output))
    }

    fn compensate_erased(
        &self,
        ctx: &S::Context,
        input: Box<dyn CloneableAny>,
    ) -> Result<(), S::Error> {
        let typed_input = input
            .into_any()
            .downcast::<S::Input>()
            .expect("type-state builder guarantees correct input type");
        self.step.compensate(ctx, *typed_input)
    }

    fn compensation_description(&self) -> String {
        self.step.compensation_description()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestContext {
        multiplier: i32,
    }

    #[derive(Debug, PartialEq)]
    struct TestError(String);

    struct MultiplyStep;

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
            ctx: &Self::Context,
            input: Self::Input,
        ) -> Result<Self::Output, Self::Error> {
            Ok(input * ctx.multiplier)
        }
    }

    struct FailingStep;

    impl SagaStep for FailingStep {
        type Input = String;
        type Output = ();
        type Context = TestContext;
        type Error = TestError;

        fn name(&self) -> &'static str {
            "failing"
        }

        fn execute(
            &self,
            _ctx: &Self::Context,
            input: Self::Input,
        ) -> Result<Self::Output, Self::Error> {
            Err(TestError(input))
        }
    }

    #[test]
    fn wrapper_delegates_name() {
        let wrapper = StepWrapper::new(MultiplyStep);
        assert_eq!(wrapper.name(), "multiply");
    }

    #[test]
    fn wrapper_executes_with_erased_types() {
        let ctx = TestContext { multiplier: 3 };
        let wrapper = StepWrapper::new(MultiplyStep);

        let input: Box<dyn CloneableAny> = Box::new(7_i32);
        let result = wrapper.execute_erased(&ctx, input);

        let output = result
            .expect("execution should succeed")
            .into_any()
            .downcast::<i32>()
            .expect("output should be i32");
        assert_eq!(*output, 21);
    }

    #[test]
    fn wrapper_compensates_with_erased_types() {
        let ctx = TestContext { multiplier: 3 };
        let wrapper = StepWrapper::new(MultiplyStep);

        let input: Box<dyn CloneableAny> = Box::new(7_i32);
        let result = wrapper.compensate_erased(&ctx, input);

        assert!(result.is_ok());
    }

    #[test]
    fn wrapper_returns_compensation_description() {
        let wrapper = StepWrapper::new(MultiplyStep);
        assert_eq!(wrapper.compensation_description(), "undo multiply");
    }

    #[test]
    fn wrapper_propagates_errors() {
        let ctx = TestContext { multiplier: 1 };
        let wrapper = StepWrapper::new(FailingStep);

        let input: Box<dyn CloneableAny> = Box::new(String::from("test error"));
        let result = wrapper.execute_erased(&ctx, input);

        assert!(result.is_err());
        let err = result.err().expect("should have an error");
        assert_eq!(err, TestError(String::from("test error")));
    }
}
