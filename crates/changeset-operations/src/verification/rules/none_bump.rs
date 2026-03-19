use std::collections::HashMap;

use changeset_core::BumpType;

use super::{VerificationContext, VerificationResult, VerificationRule};
use crate::Result;
use crate::traits::ChangesetReader;

pub struct NoneBumpDisallowedRule<'a, R: ChangesetReader> {
    reader: &'a R,
}

impl<'a, R: ChangesetReader> NoneBumpDisallowedRule<'a, R> {
    pub fn new(reader: &'a R) -> Self {
        Self { reader }
    }
}

impl<R: ChangesetReader> VerificationRule for NoneBumpDisallowedRule<'_, R> {
    fn check(&self, context: &VerificationContext, result: &mut VerificationResult) -> Result<()> {
        let mut max_bumps: HashMap<String, BumpType> = HashMap::new();

        for path in &context.changeset_files {
            let changeset = self.reader.read_changeset(path)?;
            for release in changeset.releases {
                let entry = max_bumps.entry(release.name).or_insert(BumpType::None);
                if release.bump_type > *entry {
                    *entry = release.bump_type;
                }
            }
        }

        let mut violations: Vec<String> = max_bumps
            .into_iter()
            .filter(|(_, bump)| bump.is_noop())
            .map(|(name, _)| name)
            .collect();

        violations.sort();
        result.none_bump_violations = violations;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use anyhow::Result;
    use changeset_core::{BumpType, ChangeCategory, Changeset, PackageRelease};

    use super::*;
    use crate::verification::{VerificationContext, VerificationResult};

    fn make_changeset(name: &str, bump: BumpType, summary: &str) -> Changeset {
        Changeset {
            summary: summary.to_string(),
            releases: vec![PackageRelease {
                name: name.to_string(),
                bump_type: bump,
            }],
            category: ChangeCategory::Changed,
            consumed_for_prerelease: None,
            graduate: false,
        }
    }

    fn empty_result() -> VerificationResult {
        VerificationResult {
            affected_packages: Vec::new(),
            transitive_dependents: std::collections::HashSet::new(),
            covered_packages: std::collections::HashSet::new(),
            uncovered_packages: Vec::new(),
            deleted_changesets: Vec::new(),
            none_bump_violations: Vec::new(),
            project_files: Vec::new(),
            ignored_files: Vec::new(),
        }
    }

    fn empty_context() -> VerificationContext {
        VerificationContext {
            affected_packages: Vec::new(),
            transitive_dependents: std::collections::HashSet::new(),
            changeset_files: Vec::new(),
            deleted_changesets: Vec::new(),
            project_files: Vec::new(),
            ignored_files: Vec::new(),
        }
    }

    #[test]
    fn disallow_rule_detects_none_bump_packages() -> Result<()> {
        let reader = crate::mocks::MockChangesetReader::new().with_changeset(
            PathBuf::from("a.md"),
            make_changeset("my-crate", BumpType::None, "Internal change"),
        );

        let rule = NoneBumpDisallowedRule::new(&reader);

        let mut context = empty_context();
        context.changeset_files = vec![PathBuf::from("a.md")];

        let mut result = empty_result();
        rule.check(&context, &mut result)?;

        assert_eq!(result.none_bump_violations, vec!["my-crate".to_string()]);
        assert!(!result.is_success());
        Ok(())
    }

    #[test]
    fn disallow_rule_permits_non_none_packages() -> Result<()> {
        let reader = crate::mocks::MockChangesetReader::new().with_changeset(
            PathBuf::from("a.md"),
            make_changeset("my-crate", BumpType::Patch, "Fix bug"),
        );

        let rule = NoneBumpDisallowedRule::new(&reader);

        let mut context = empty_context();
        context.changeset_files = vec![PathBuf::from("a.md")];

        let mut result = empty_result();
        rule.check(&context, &mut result)?;

        assert!(result.none_bump_violations.is_empty());
        Ok(())
    }

    #[test]
    fn disallow_rule_permits_mixed_bumps_across_changesets() -> Result<()> {
        let reader = crate::mocks::MockChangesetReader::new()
            .with_changeset(
                PathBuf::from("a.md"),
                make_changeset("my-crate", BumpType::None, "Internal"),
            )
            .with_changeset(
                PathBuf::from("b.md"),
                make_changeset("my-crate", BumpType::Patch, "Fix bug"),
            );

        let rule = NoneBumpDisallowedRule::new(&reader);

        let mut context = empty_context();
        context.changeset_files = vec![PathBuf::from("a.md"), PathBuf::from("b.md")];

        let mut result = empty_result();
        rule.check(&context, &mut result)?;

        assert!(result.none_bump_violations.is_empty());
        Ok(())
    }
}
