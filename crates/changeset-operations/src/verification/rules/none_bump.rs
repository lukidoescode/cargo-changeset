use changeset_core::Changeset;

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
        let changesets: Vec<Changeset> = context
            .changeset_files()
            .iter()
            .map(|path| self.reader.read_changeset(path))
            .collect::<Result<Vec<_>>>()?;

        result.set_none_bump_violations(crate::none_bump::find_none_only_packages(&changesets));

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
        Changeset::new(
            summary.to_string(),
            vec![PackageRelease::new(name.to_string(), bump)],
            ChangeCategory::Changed,
        )
    }

    fn empty_result() -> VerificationResult {
        VerificationResult::new(
            Vec::new(),
            std::collections::HashSet::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    #[test]
    fn disallow_rule_detects_none_bump_packages() -> Result<()> {
        let reader = crate::mocks::MockChangesetReader::new().with_changeset(
            PathBuf::from("a.md"),
            make_changeset("my-crate", BumpType::None, "Internal change"),
        );

        let rule = NoneBumpDisallowedRule::new(&reader);

        let context = VerificationContext::new(
            Vec::new(),
            std::collections::HashSet::new(),
            vec![PathBuf::from("a.md")],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );

        let mut result = empty_result();
        rule.check(&context, &mut result)?;

        assert_eq!(result.none_bump_violations(), &vec!["my-crate".to_string()]);
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

        let context = VerificationContext::new(
            Vec::new(),
            std::collections::HashSet::new(),
            vec![PathBuf::from("a.md")],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );

        let mut result = empty_result();
        rule.check(&context, &mut result)?;

        assert!(result.none_bump_violations().is_empty());
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

        let context = VerificationContext::new(
            Vec::new(),
            std::collections::HashSet::new(),
            vec![PathBuf::from("a.md"), PathBuf::from("b.md")],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );

        let mut result = empty_result();
        rule.check(&context, &mut result)?;

        assert!(result.none_bump_violations().is_empty());
        Ok(())
    }
}
