use std::collections::HashSet;

use super::rules::VerificationRule;
use super::{VerificationContext, VerificationResult};
use crate::Result;

pub struct VerificationEngine<'a> {
    rules: Vec<&'a dyn VerificationRule>,
}

impl<'a> VerificationEngine<'a> {
    #[must_use]
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule(&mut self, rule: &'a dyn VerificationRule) {
        self.rules.push(rule);
    }

    /// # Errors
    ///
    /// Returns an error if any verification rule fails.
    pub fn verify(&self, context: &VerificationContext) -> Result<VerificationResult> {
        let mut result = VerificationResult {
            affected_packages: context.affected_packages.clone(),
            covered_packages: HashSet::new(),
            uncovered_packages: Vec::new(),
            deleted_changesets: Vec::new(),
            project_files: context.project_files.clone(),
            ignored_files: context.ignored_files.clone(),
        };

        for rule in &self.rules {
            rule.check(context, &mut result)?;
        }

        Ok(result)
    }
}

impl Default for VerificationEngine<'_> {
    fn default() -> Self {
        Self::new()
    }
}
