use std::collections::BTreeMap;
use std::fmt::Write;

use chrono::NaiveDate;
use semver::Version;

use changeset_core::ChangeCategory;

use crate::entry::{ChangelogEntry, VersionRelease};
use crate::forge::RepositoryInfo;

const CHANGELOG_HEADER: &str = r"# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
";

#[must_use]
pub fn new_changelog() -> String {
    CHANGELOG_HEADER.to_string()
}

#[must_use]
pub fn format_entries(entries: &[ChangelogEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }

    let mut by_category: BTreeMap<ChangeCategory, Vec<&ChangelogEntry>> = BTreeMap::new();

    for entry in entries {
        by_category.entry(entry.category).or_default().push(entry);
    }

    let mut output = String::new();

    let category_order = [
        ChangeCategory::Added,
        ChangeCategory::Changed,
        ChangeCategory::Deprecated,
        ChangeCategory::Removed,
        ChangeCategory::Fixed,
        ChangeCategory::Security,
    ];

    for category in category_order {
        if let Some(category_entries) = by_category.get(&category) {
            output.push_str("\n### ");
            output.push_str(&category.to_string());
            output.push('\n');

            for entry in category_entries {
                output.push_str("\n- ");
                if let Some(ref package) = entry.package {
                    output.push_str("**");
                    output.push_str(package);
                    output.push_str("**: ");
                }
                output.push_str(&entry.description);
            }
            output.push('\n');
        }
    }

    output
}

#[must_use]
pub fn format_version_header(version: &Version, date: NaiveDate) -> String {
    format!("## [{version}] - {date}")
}

#[must_use]
pub fn format_version_release(release: &VersionRelease) -> String {
    let mut output = format_version_header(&release.version, release.date);
    output.push_str(&format_entries(&release.entries));
    output
}

#[must_use]
pub fn format_comparison_links(
    versions: &[(Version, Option<&str>)],
    repo_info: &RepositoryInfo,
) -> String {
    let mut output = String::new();

    for (version, previous) in versions {
        let target_tag = format!("v{version}");
        let base_tag = previous.map_or_else(|| "HEAD".to_string(), ToString::to_string);
        let url = repo_info.comparison_url(&base_tag, &target_tag);
        let _ = writeln!(output, "[{version}]: {url}");
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_changelog_has_proper_header() {
        let changelog = new_changelog();
        assert!(changelog.contains("# Changelog"));
        assert!(changelog.contains("Keep a Changelog"));
        assert!(changelog.contains("Semantic Versioning"));
    }

    #[test]
    fn format_empty_entries() {
        let entries: Vec<ChangelogEntry> = vec![];
        let formatted = format_entries(&entries);
        assert!(formatted.is_empty());
    }

    #[test]
    fn format_single_entry() {
        let entries = vec![ChangelogEntry::new(ChangeCategory::Added, "New feature")];
        let formatted = format_entries(&entries);
        assert!(formatted.contains("### Added"));
        assert!(formatted.contains("- New feature"));
    }

    #[test]
    fn format_entries_grouped_by_category() {
        let entries = vec![
            ChangelogEntry::new(ChangeCategory::Fixed, "Bug fix"),
            ChangelogEntry::new(ChangeCategory::Added, "New feature"),
            ChangelogEntry::new(ChangeCategory::Fixed, "Another bug fix"),
        ];
        let formatted = format_entries(&entries);

        assert!(formatted.contains("### Added"));
        assert!(formatted.contains("### Fixed"));

        let added_pos = formatted.find("### Added").expect("Added section exists");
        let fixed_pos = formatted.find("### Fixed").expect("Fixed section exists");
        assert!(
            added_pos < fixed_pos,
            "Added should come before Fixed per Keep a Changelog order"
        );
    }

    #[test]
    fn format_entry_with_package() {
        let entries =
            vec![ChangelogEntry::new(ChangeCategory::Changed, "Updated API").with_package("core")];
        let formatted = format_entries(&entries);
        assert!(formatted.contains("- **core**: Updated API"));
    }

    #[test]
    fn format_version_header_correct() {
        let version = Version::new(1, 2, 3);
        let date = NaiveDate::from_ymd_opt(2025, 3, 15).expect("valid date");
        let header = format_version_header(&version, date);
        assert_eq!(header, "## [1.2.3] - 2025-03-15");
    }

    #[test]
    fn format_complete_version_release() {
        let version = Version::new(1, 0, 0);
        let date = NaiveDate::from_ymd_opt(2025, 1, 1).expect("valid date");
        let entries = vec![
            ChangelogEntry::new(ChangeCategory::Added, "Initial release"),
            ChangelogEntry::new(ChangeCategory::Security, "Fixed vulnerability"),
        ];
        let release = VersionRelease::new(version, date, entries);

        let formatted = format_version_release(&release);
        assert!(formatted.contains("## [1.0.0] - 2025-01-01"));
        assert!(formatted.contains("### Added"));
        assert!(formatted.contains("### Security"));
    }

    #[test]
    fn categories_in_keep_a_changelog_order() {
        let entries = vec![
            ChangelogEntry::new(ChangeCategory::Security, "Security fix"),
            ChangelogEntry::new(ChangeCategory::Removed, "Removed feature"),
            ChangelogEntry::new(ChangeCategory::Deprecated, "Deprecated API"),
            ChangelogEntry::new(ChangeCategory::Fixed, "Bug fix"),
            ChangelogEntry::new(ChangeCategory::Changed, "Changed behavior"),
            ChangelogEntry::new(ChangeCategory::Added, "New feature"),
        ];

        let formatted = format_entries(&entries);

        let positions: Vec<usize> = [
            "### Added",
            "### Changed",
            "### Deprecated",
            "### Removed",
            "### Fixed",
            "### Security",
        ]
        .iter()
        .filter_map(|s| formatted.find(s))
        .collect();

        for window in positions.windows(2) {
            assert!(
                window[0] < window[1],
                "Categories should be in Keep a Changelog order"
            );
        }
    }
}
