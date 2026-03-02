use std::path::{Path, PathBuf};

use changeset_changelog::{ComparisonLinksSetting, RepositoryInfo};

use crate::Result;
use crate::error::OperationError;
use crate::operations::changelog_aggregation::ChangesetAggregator;
use crate::traits::{ChangesetReader, GitStatusProvider};

pub(super) fn load_changesets<RW: ChangesetReader>(
    changeset_io: &RW,
    changeset_dir: &Path,
    changeset_files: &[PathBuf],
) -> Result<(Vec<changeset_core::Changeset>, ChangesetAggregator)> {
    let mut changesets = Vec::new();
    let mut aggregator = ChangesetAggregator::new();

    for path in changeset_files {
        let changeset = changeset_io.read_changeset(path)?;
        aggregator.add_changeset(&changeset);
        changesets.push(changeset);
    }

    let consumed_paths = changeset_io.list_consumed_changesets(changeset_dir)?;
    for path in &consumed_paths {
        let changeset = changeset_io.read_changeset(path)?;
        aggregator.add_changeset(&changeset);
    }

    Ok((changesets, aggregator))
}

// Errors are deliberately suppressed: in Auto mode, a missing or
// inaccessible git remote is not a failure - it just means we cannot
// generate comparison links.
fn detect_repository_info<G: GitStatusProvider>(
    git_provider: &G,
    project_root: &Path,
) -> Option<RepositoryInfo> {
    let url = git_provider.remote_url(project_root).ok()??;
    RepositoryInfo::from_url(&url).ok()
}

pub(super) fn resolve_repo_info<G: GitStatusProvider>(
    git_provider: &G,
    project_root: &Path,
    changelog_config: &changeset_changelog::ChangelogConfig,
) -> Result<Option<RepositoryInfo>> {
    match changelog_config.comparison_links() {
        ComparisonLinksSetting::Disabled => Ok(None),
        ComparisonLinksSetting::Auto => Ok(detect_repository_info(git_provider, project_root)),
        ComparisonLinksSetting::Enabled => {
            let url = git_provider
                .remote_url(project_root)?
                .ok_or(OperationError::ComparisonLinksRequired)?;
            let repo_info = RepositoryInfo::from_url(&url)
                .map_err(|_| OperationError::ComparisonLinksRequired)?;
            Ok(Some(repo_info))
        }
    }
}
