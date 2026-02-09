use chrono::NaiveDate;
use semver::Version;

use changeset_core::ChangeCategory;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangelogEntry {
    pub category: ChangeCategory,
    pub description: String,
    pub package: Option<String>,
}

impl ChangelogEntry {
    #[must_use]
    pub fn new(category: ChangeCategory, description: impl Into<String>) -> Self {
        Self {
            category,
            description: description.into(),
            package: None,
        }
    }

    #[must_use]
    pub fn with_package(mut self, package: impl Into<String>) -> Self {
        self.package = Some(package.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionRelease {
    pub version: Version,
    pub date: NaiveDate,
    pub entries: Vec<ChangelogEntry>,
}

impl VersionRelease {
    #[must_use]
    pub fn new(version: Version, date: NaiveDate, entries: Vec<ChangelogEntry>) -> Self {
        Self {
            version,
            date,
            entries,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_entry() {
        let entry = ChangelogEntry::new(ChangeCategory::Fixed, "Fixed a bug");
        assert_eq!(entry.category, ChangeCategory::Fixed);
        assert_eq!(entry.description, "Fixed a bug");
        assert!(entry.package.is_none());
    }

    #[test]
    fn create_entry_with_package() {
        let entry =
            ChangelogEntry::new(ChangeCategory::Added, "Added feature").with_package("my-crate");
        assert_eq!(entry.category, ChangeCategory::Added);
        assert_eq!(entry.description, "Added feature");
        assert_eq!(entry.package.as_deref(), Some("my-crate"));
    }

    #[test]
    fn create_version_release() {
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).expect("valid date");
        let version = Version::new(1, 0, 0);
        let entries = vec![ChangelogEntry::new(
            ChangeCategory::Added,
            "Initial release",
        )];

        let release = VersionRelease::new(version.clone(), date, entries.clone());
        assert_eq!(release.version, version);
        assert_eq!(release.date, date);
        assert_eq!(release.entries, entries);
    }
}
