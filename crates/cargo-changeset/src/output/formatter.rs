use crate::verification::VerificationResult;

pub(crate) trait OutputFormatter {
    fn format_success(&self, result: &VerificationResult) -> String;
    fn format_failure(&self, result: &VerificationResult) -> String;
}
