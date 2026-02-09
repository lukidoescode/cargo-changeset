use std::path::Path;

use crate::entry::VersionRelease;
use crate::error::ChangelogError;
use crate::forge::RepositoryInfo;
use crate::format::{format_version_release, new_changelog};

const HEADER_END_MARKER: &str = "and this project adheres to [Semantic Versioning]";

#[derive(Debug, Clone)]
pub struct Changelog {
    content: String,
}

impl Default for Changelog {
    fn default() -> Self {
        Self::new()
    }
}

impl Changelog {
    #[must_use]
    pub fn new() -> Self {
        Self {
            content: new_changelog(),
        }
    }

    /// # Errors
    ///
    /// Returns `ChangelogError::Read` if the file cannot be read.
    pub fn from_file(path: &Path) -> Result<Self, ChangelogError> {
        let content = std::fs::read_to_string(path).map_err(|source| ChangelogError::Read {
            path: path.to_path_buf(),
            source,
        })?;

        Ok(Self { content })
    }

    /// # Errors
    ///
    /// Returns `ChangelogError::Read` if the file cannot be read.
    /// Returns `ChangelogError::InvalidChangelogFormat` if the file does not contain a valid changelog header.
    pub fn from_file_validated(path: &Path) -> Result<Self, ChangelogError> {
        let changelog = Self::from_file(path)?;

        if !changelog.content.contains("# Changelog") {
            return Err(ChangelogError::InvalidChangelogFormat {
                path: path.to_path_buf(),
            });
        }

        Ok(changelog)
    }

    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn add_release(
        &mut self,
        release: &VersionRelease,
        repo_info: Option<&RepositoryInfo>,
        previous_version: Option<&str>,
    ) {
        let formatted = format_version_release(release);

        let insertion_point = self.find_insertion_point();

        let mut new_content = String::with_capacity(self.content.len() + formatted.len() + 100);

        new_content.push_str(&self.content[..insertion_point]);
        if !new_content.ends_with("\n\n") {
            if new_content.ends_with('\n') {
                new_content.push('\n');
            } else {
                new_content.push_str("\n\n");
            }
        }
        new_content.push_str(&formatted);

        if insertion_point < self.content.len() {
            let rest = &self.content[insertion_point..];
            if !rest.starts_with('\n') {
                new_content.push('\n');
            }
            new_content.push_str(rest);
        }

        if let Some(repo) = repo_info {
            let base_tag = previous_version.map_or("HEAD".to_string(), |v| format!("v{v}"));
            let target_tag = format!("v{}", release.version);
            let comparison_url = repo.comparison_url(&base_tag, &target_tag);

            let link_line = format!("[{}]: {}", release.version, comparison_url);
            if !new_content.contains(&link_line) {
                if !new_content.ends_with('\n') {
                    new_content.push('\n');
                }
                new_content.push('\n');
                new_content.push_str(&link_line);
                new_content.push('\n');
            }
        }

        self.content = new_content;
    }

    /// # Errors
    ///
    /// Returns `ChangelogError::Write` if the file cannot be written.
    pub fn write_to_file(&self, path: &Path) -> Result<(), ChangelogError> {
        std::fs::write(path, &self.content).map_err(|source| ChangelogError::Write {
            path: path.to_path_buf(),
            source,
        })
    }

    fn find_insertion_point(&self) -> usize {
        if let Some(first_version_pos) = self.content.find("\n## [") {
            return first_version_pos + 1;
        }

        if let Some(header_end) = self.content.find(HEADER_END_MARKER) {
            if let Some(newline_after) = self.content[header_end..].find('\n') {
                return header_end + newline_after + 1;
            }
        }

        self.content.len()
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use semver::Version;

    use changeset_core::ChangeCategory;

    use super::*;
    use crate::entry::ChangelogEntry;

    #[test]
    fn new_changelog_has_header() {
        let changelog = Changelog::new();
        assert!(changelog.content().contains("# Changelog"));
        assert!(changelog.content().contains("Keep a Changelog"));
    }

    #[test]
    fn add_release_creates_version_section() {
        let mut changelog = Changelog::new();
        let entries = vec![ChangelogEntry::new(
            ChangeCategory::Added,
            "Initial release",
        )];

        let release = VersionRelease::new(
            Version::new(1, 0, 0),
            NaiveDate::from_ymd_opt(2025, 1, 15).expect("valid date"),
            entries,
        );

        changelog.add_release(&release, None, None);

        assert!(changelog.content().contains("## [1.0.0] - 2025-01-15"));
        assert!(changelog.content().contains("### Added"));
        assert!(changelog.content().contains("- Initial release"));
    }

    #[test]
    fn add_release_with_comparison_link() {
        let mut changelog = Changelog::new();
        let entries = vec![ChangelogEntry::new(ChangeCategory::Fixed, "Bug fix")];

        let release = VersionRelease::new(
            Version::new(1, 1, 0),
            NaiveDate::from_ymd_opt(2025, 2, 1).expect("valid date"),
            entries,
        );

        let repo_info =
            RepositoryInfo::from_url("https://github.com/owner/repo").expect("valid url");

        changelog.add_release(&release, Some(&repo_info), Some("1.0.0"));

        assert!(
            changelog
                .content()
                .contains("[1.1.0]: https://github.com/owner/repo/compare/v1.0.0...v1.1.0")
        );
    }

    #[test]
    fn multiple_releases_maintain_order() {
        let mut changelog = Changelog::new();

        let release1 = VersionRelease::new(
            Version::new(1, 0, 0),
            NaiveDate::from_ymd_opt(2025, 1, 1).expect("valid date"),
            vec![ChangelogEntry::new(ChangeCategory::Added, "First release")],
        );

        let release2 = VersionRelease::new(
            Version::new(1, 1, 0),
            NaiveDate::from_ymd_opt(2025, 2, 1).expect("valid date"),
            vec![ChangelogEntry::new(ChangeCategory::Fixed, "Bug fix")],
        );

        changelog.add_release(&release1, None, None);
        changelog.add_release(&release2, None, Some("1.0.0"));

        let v110_pos = changelog
            .content()
            .find("## [1.1.0]")
            .expect("1.1.0 exists");
        let v100_pos = changelog
            .content()
            .find("## [1.0.0]")
            .expect("1.0.0 exists");

        assert!(
            v110_pos < v100_pos,
            "Newer versions should appear before older versions"
        );
    }

    #[test]
    fn version_section_appears_after_header() {
        let mut changelog = Changelog::new();

        let release = VersionRelease::new(
            Version::new(1, 0, 0),
            NaiveDate::from_ymd_opt(2025, 1, 1).expect("valid date"),
            vec![ChangelogEntry::new(ChangeCategory::Added, "Feature")],
        );

        changelog.add_release(&release, None, None);

        let header_pos = changelog
            .content()
            .find("# Changelog")
            .expect("Header exists");
        let version_pos = changelog
            .content()
            .find("## [1.0.0]")
            .expect("Version section exists");

        assert!(
            header_pos < version_pos,
            "Version section should appear after header"
        );
    }

    #[test]
    fn from_file_reads_content() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_path = temp_dir.path().join("CHANGELOG.md");

        let content = "# Changelog\n\n## [1.0.0] - 2025-01-01\n\n### Added\n\n- Initial release\n";
        std::fs::write(&file_path, content).expect("write file");

        let changelog = Changelog::from_file(&file_path).expect("read file");
        assert_eq!(changelog.content(), content);
    }

    #[test]
    fn from_file_returns_error_for_missing_file() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_path = temp_dir.path().join("nonexistent.md");

        let result = Changelog::from_file(&file_path);
        assert!(matches!(result, Err(ChangelogError::Read { .. })));
    }

    #[test]
    fn write_to_file_creates_file() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_path = temp_dir.path().join("CHANGELOG.md");

        let changelog = Changelog::new();
        changelog.write_to_file(&file_path).expect("write file");

        let read_content = std::fs::read_to_string(&file_path).expect("read file");
        assert_eq!(read_content, changelog.content());
    }

    #[test]
    fn from_file_validated_accepts_valid_changelog() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_path = temp_dir.path().join("CHANGELOG.md");

        let content = "# Changelog\n\nAll notable changes...\n";
        std::fs::write(&file_path, content).expect("write file");

        let changelog = Changelog::from_file_validated(&file_path).expect("read file");
        assert_eq!(changelog.content(), content);
    }

    #[test]
    fn from_file_validated_rejects_invalid_changelog() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_path = temp_dir.path().join("CHANGELOG.md");

        let content = "This is not a changelog\n";
        std::fs::write(&file_path, content).expect("write file");

        let result = Changelog::from_file_validated(&file_path);
        assert!(matches!(
            result,
            Err(ChangelogError::InvalidChangelogFormat { .. })
        ));
    }
}
