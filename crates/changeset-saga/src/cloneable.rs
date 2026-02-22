use std::any::Any;

/// Trait for type-erased values that can be cloned.
///
/// This trait combines `Any` with `Clone` capability, allowing values to be
/// cloned without knowing their concrete type at compile time.
pub(crate) trait CloneableAny: Any + Send {
    /// Clone the value into a new boxed trait object.
    fn clone_box(&self) -> Box<dyn CloneableAny>;

    /// Convert into a boxed `Any` for downcasting.
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send>;
}

impl<T> CloneableAny for T
where
    T: Clone + Send + 'static,
{
    fn clone_box(&self) -> Box<dyn CloneableAny> {
        Box::new(self.clone())
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any + Send> {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_box_creates_independent_copy_for_i32() {
        let original: Box<dyn CloneableAny> = Box::new(42_i32);
        let cloned = original.clone_box();

        let original_value = original
            .into_any()
            .downcast::<i32>()
            .expect("downcast to i32");
        let cloned_value = cloned
            .into_any()
            .downcast::<i32>()
            .expect("downcast to i32");

        assert_eq!(*original_value, 42);
        assert_eq!(*cloned_value, 42);
    }

    #[test]
    fn clone_box_creates_independent_copy_for_string() {
        let original: Box<dyn CloneableAny> = Box::new(String::from("test"));
        let cloned = original.clone_box();

        let original_value = original
            .into_any()
            .downcast::<String>()
            .expect("downcast to String");
        let cloned_value = cloned
            .into_any()
            .downcast::<String>()
            .expect("downcast to String");

        assert_eq!(*original_value, "test");
        assert_eq!(*cloned_value, "test");
    }

    #[test]
    fn into_any_allows_downcasting_to_original_type() {
        let boxed: Box<dyn CloneableAny> = Box::new(123_i64);
        let any = boxed.into_any();

        let value = any.downcast::<i64>().expect("downcast to i64");
        assert_eq!(*value, 123);
    }

    #[test]
    fn into_any_returns_none_for_wrong_type() {
        let boxed: Box<dyn CloneableAny> = Box::new(42_i32);
        let any = boxed.into_any();

        let result = any.downcast::<String>();
        assert!(result.is_err());
    }
}
