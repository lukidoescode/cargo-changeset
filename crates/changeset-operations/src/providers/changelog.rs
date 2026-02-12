use std::path::Path;

use changeset_changelog::{Changelog, RepositoryInfo, VersionRelease};

use crate::Result;
use crate::traits::{ChangelogWriteResult, ChangelogWriter};

pub struct FileSystemChangelogWriter;

impl FileSystemChangelogWriter {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileSystemChangelogWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl ChangelogWriter for FileSystemChangelogWriter {
    fn write_release(
        &self,
        changelog_path: &Path,
        release: &VersionRelease,
        repo_info: Option<&RepositoryInfo>,
        previous_version: Option<&str>,
    ) -> Result<ChangelogWriteResult> {
        let created = !changelog_path.exists();

        let mut changelog = if created {
            Changelog::new()
        } else {
            Changelog::from_file(changelog_path)?
        };

        changelog.add_release(release, repo_info, previous_version);
        changelog.write_to_file(changelog_path)?;

        Ok(ChangelogWriteResult {
            path: changelog_path.to_path_buf(),
            created,
        })
    }

    fn changelog_exists(&self, path: &Path) -> bool {
        path.exists()
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use semver::Version;
    use tempfile::TempDir;

    use changeset_changelog::ChangelogEntry;
    use changeset_core::ChangeCategory;

    use super::*;

    fn create_test_release() -> VersionRelease {
        VersionRelease::new(
            Version::new(1, 0, 0),
            NaiveDate::from_ymd_opt(2025, 1, 15).expect("valid date"),
            vec![ChangelogEntry::new(
                ChangeCategory::Added,
                "Initial release",
            )],
        )
    }

    #[test]
    fn creates_new_changelog_when_missing() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let changelog_path = dir.path().join("CHANGELOG.md");
        let writer = FileSystemChangelogWriter::new();

        let release = create_test_release();
        let result = writer.write_release(&changelog_path, &release, None, None)?;

        assert!(result.created);
        assert!(changelog_path.exists());

        let content = std::fs::read_to_string(&changelog_path)?;
        assert!(content.contains("# Changelog"));
        assert!(content.contains("## [1.0.0] - 2025-01-15"));
        assert!(content.contains("- Initial release"));

        Ok(())
    }

    #[test]
    fn appends_to_existing_changelog() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let changelog_path = dir.path().join("CHANGELOG.md");
        let writer = FileSystemChangelogWriter::new();

        let release1 = create_test_release();
        writer.write_release(&changelog_path, &release1, None, None)?;

        let release2 = VersionRelease::new(
            Version::new(1, 1, 0),
            NaiveDate::from_ymd_opt(2025, 2, 1).expect("valid date"),
            vec![ChangelogEntry::new(ChangeCategory::Fixed, "Bug fix")],
        );
        let result = writer.write_release(&changelog_path, &release2, None, Some("1.0.0"))?;

        assert!(!result.created);

        let content = std::fs::read_to_string(&changelog_path)?;
        assert!(content.contains("## [1.1.0] - 2025-02-01"));
        assert!(content.contains("## [1.0.0] - 2025-01-15"));

        Ok(())
    }

    #[test]
    fn changelog_exists_returns_false_when_missing() {
        let dir = TempDir::new().expect("create temp dir");
        let changelog_path = dir.path().join("CHANGELOG.md");
        let writer = FileSystemChangelogWriter::new();

        assert!(!writer.changelog_exists(&changelog_path));
    }

    #[test]
    fn changelog_exists_returns_true_when_present() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let changelog_path = dir.path().join("CHANGELOG.md");
        std::fs::write(&changelog_path, "# Changelog")?;
        let writer = FileSystemChangelogWriter::new();

        assert!(writer.changelog_exists(&changelog_path));

        Ok(())
    }

    #[test]
    fn adds_comparison_link_with_repo_info() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let changelog_path = dir.path().join("CHANGELOG.md");
        let writer = FileSystemChangelogWriter::new();

        let release = VersionRelease::new(
            Version::new(1, 1, 0),
            NaiveDate::from_ymd_opt(2025, 2, 1).expect("valid date"),
            vec![ChangelogEntry::new(ChangeCategory::Fixed, "Bug fix")],
        );

        let repo_info = RepositoryInfo::from_url("https://github.com/owner/repo")?;
        writer.write_release(&changelog_path, &release, Some(&repo_info), Some("1.0.0"))?;

        let content = std::fs::read_to_string(&changelog_path)?;
        assert!(content.contains("[1.1.0]: https://github.com/owner/repo/compare/v1.0.0...v1.1.0"));

        Ok(())
    }
}
