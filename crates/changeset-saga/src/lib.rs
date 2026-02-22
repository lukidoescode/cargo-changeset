//! Saga pattern for atomic multi-step operations.
//!
//! This crate provides infrastructure for executing multi-step operations
//! with automatic rollback on failure. Each step produces an output that
//! becomes the next step's input, and stores the original input for compensation.

mod audit;
mod builder;
mod cloneable;
mod erased;
mod error;
mod saga;
mod step;

pub use audit::{SagaAuditLog, StepRecord, StepStatus};
pub use builder::SagaBuilder;
pub use error::{CompensationError, SagaError};
pub use saga::Saga;
pub use step::SagaStep;
