use super::{VerificationContext, VerificationResult, VerificationRule};
use crate::error::Result;
use crate::verification::ChangesetReader;

pub(crate) struct CoverageRule<'a, R: ChangesetReader> {
    reader: &'a R,
}

impl<'a, R: ChangesetReader> CoverageRule<'a, R> {
    pub fn new(reader: &'a R) -> Self {
        Self { reader }
    }
}

impl<R: ChangesetReader> VerificationRule for CoverageRule<'_, R> {
    fn check(&self, context: &VerificationContext, result: &mut VerificationResult) -> Result<()> {
        for path in &context.changeset_files {
            let changeset = self.reader.read_changeset(path)?;
            for release in changeset.releases {
                result.covered_packages.insert(release.name);
            }
        }

        result.uncovered_packages = context
            .affected_packages
            .iter()
            .filter(|pkg| !result.covered_packages.contains(&pkg.name))
            .cloned()
            .collect();

        Ok(())
    }
}
