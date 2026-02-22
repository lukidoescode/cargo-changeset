use std::time::Instant;

/// Status of a step in the audit log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StepStatus {
    /// Step executed successfully.
    Executed,
    /// Step failed during execution.
    Failed,
    /// Step was compensated successfully.
    Compensated,
    /// Step compensation failed.
    CompensationFailed,
}

/// Record of a step's execution in the saga.
#[derive(Debug)]
pub struct StepRecord {
    /// Name of the step.
    pub name: String,
    /// Current status.
    pub status: StepStatus,
    /// When the step started executing.
    pub started_at: Instant,
    /// When the step completed (execution or compensation).
    pub completed_at: Option<Instant>,
    /// Description of compensation (if applicable).
    pub compensation_description: Option<String>,
}

/// Audit log tracking all step executions in a saga.
#[derive(Debug, Default)]
pub struct SagaAuditLog {
    records: Vec<StepRecord>,
}

impl SagaAuditLog {
    /// Create a new empty audit log.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a step execution starting.
    pub(crate) fn record_start(&mut self, name: &str) {
        self.records.push(StepRecord {
            name: name.to_string(),
            status: StepStatus::Executed,
            started_at: Instant::now(),
            completed_at: None,
            compensation_description: None,
        });
    }

    /// Mark the last step as failed.
    pub(crate) fn record_failure(&mut self) {
        if let Some(record) = self.records.last_mut() {
            record.status = StepStatus::Failed;
            record.completed_at = Some(Instant::now());
        }
    }

    /// Mark the last step as completed successfully.
    pub(crate) fn record_success(&mut self, compensation_description: String) {
        if let Some(record) = self.records.last_mut() {
            record.status = StepStatus::Executed;
            record.completed_at = Some(Instant::now());
            record.compensation_description = Some(compensation_description);
        }
    }

    /// Record that a step was compensated.
    pub(crate) fn record_compensated(&mut self, step_name: &str) {
        for record in &mut self.records {
            if record.name == step_name {
                record.status = StepStatus::Compensated;
                record.completed_at = Some(Instant::now());
            }
        }
    }

    /// Record that a step's compensation failed.
    pub(crate) fn record_compensation_failed(&mut self, step_name: &str) {
        for record in &mut self.records {
            if record.name == step_name {
                record.status = StepStatus::CompensationFailed;
                record.completed_at = Some(Instant::now());
            }
        }
    }

    /// Get all records in the audit log.
    #[must_use]
    pub fn records(&self) -> &[StepRecord] {
        &self.records
    }

    /// Get a summary of the saga execution for display.
    #[must_use]
    pub fn summary(&self) -> String {
        let mut lines = Vec::new();
        for record in &self.records {
            let status = match record.status {
                StepStatus::Executed => "✓",
                StepStatus::Failed => "✗",
                StepStatus::Compensated => "↩",
                StepStatus::CompensationFailed => "⚠",
            };
            lines.push(format!("{status} {}", record.name));
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_audit_log_is_empty() {
        let log = SagaAuditLog::new();
        assert!(log.records().is_empty());
    }

    #[test]
    fn record_start_adds_step_with_executed_status() {
        let mut log = SagaAuditLog::new();
        log.record_start("test_step");

        assert_eq!(log.records().len(), 1);
        assert_eq!(log.records()[0].name, "test_step");
        assert_eq!(log.records()[0].status, StepStatus::Executed);
        assert!(log.records()[0].completed_at.is_none());
    }

    #[test]
    fn record_failure_updates_last_step() {
        let mut log = SagaAuditLog::new();
        log.record_start("step_1");
        log.record_failure();

        assert_eq!(log.records()[0].status, StepStatus::Failed);
        assert!(log.records()[0].completed_at.is_some());
    }

    #[test]
    fn record_success_updates_last_step_with_description() {
        let mut log = SagaAuditLog::new();
        log.record_start("step_1");
        log.record_success("undo step_1".to_string());

        assert_eq!(log.records()[0].status, StepStatus::Executed);
        assert!(log.records()[0].completed_at.is_some());
        assert_eq!(
            log.records()[0].compensation_description,
            Some("undo step_1".to_string())
        );
    }

    #[test]
    fn record_compensated_updates_matching_step() {
        let mut log = SagaAuditLog::new();
        log.record_start("step_1");
        log.record_success("undo".to_string());
        log.record_start("step_2");
        log.record_success("undo".to_string());
        log.record_compensated("step_1");

        assert_eq!(log.records()[0].status, StepStatus::Compensated);
        assert_eq!(log.records()[1].status, StepStatus::Executed);
    }

    #[test]
    fn record_compensation_failed_updates_matching_step() {
        let mut log = SagaAuditLog::new();
        log.record_start("step_1");
        log.record_success("undo".to_string());
        log.record_compensation_failed("step_1");

        assert_eq!(log.records()[0].status, StepStatus::CompensationFailed);
    }

    #[test]
    fn summary_formats_all_steps() {
        let mut log = SagaAuditLog::new();
        log.record_start("executed_step");
        log.record_success("undo".to_string());
        log.record_start("failed_step");
        log.record_failure();

        let summary = log.summary();
        assert!(summary.contains("✓ executed_step"));
        assert!(summary.contains("✗ failed_step"));
    }

    #[test]
    fn summary_shows_compensated_and_compensation_failed() {
        let mut log = SagaAuditLog::new();
        log.record_start("compensated_step");
        log.record_success("undo".to_string());
        log.record_compensated("compensated_step");

        log.record_start("comp_failed_step");
        log.record_success("undo".to_string());
        log.record_compensation_failed("comp_failed_step");

        let summary = log.summary();
        assert!(summary.contains("↩ compensated_step"));
        assert!(summary.contains("⚠ comp_failed_step"));
    }
}
