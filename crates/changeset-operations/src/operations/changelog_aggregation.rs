use std::collections::HashMap;

use changeset_changelog::{ChangelogEntry, VersionRelease};
use changeset_core::Changeset;
use chrono::NaiveDate;
use semver::Version;

pub(crate) struct ChangesetAggregator {
    entries_by_package: HashMap<String, Vec<ChangelogEntry>>,
}

impl ChangesetAggregator {
    pub(crate) fn new() -> Self {
        Self {
            entries_by_package: HashMap::new(),
        }
    }

    pub(crate) fn add_changeset(&mut self, changeset: &Changeset) {
        for release in &changeset.releases {
            let entry = ChangelogEntry::new(changeset.category, &changeset.summary);
            self.entries_by_package
                .entry(release.name.clone())
                .or_default()
                .push(entry);
        }
    }

    pub(crate) fn build_package_release(
        &self,
        name: &str,
        version: &Version,
        date: NaiveDate,
    ) -> Option<VersionRelease> {
        let entries = self.entries_by_package.get(name)?;
        if entries.is_empty() {
            return None;
        }

        Some(VersionRelease::new(version.clone(), date, entries.clone()))
    }

    pub(crate) fn build_root_release(
        &self,
        version: &Version,
        date: NaiveDate,
        packages: &[(String, Version)],
    ) -> Option<VersionRelease> {
        let mut all_entries: Vec<ChangelogEntry> = Vec::new();

        for (package_name, _) in packages {
            if let Some(entries) = self.entries_by_package.get(package_name) {
                for entry in entries {
                    let prefixed_entry = entry.clone().with_package(package_name);
                    all_entries.push(prefixed_entry);
                }
            }
        }

        if all_entries.is_empty() {
            return None;
        }

        Some(VersionRelease::new(version.clone(), date, all_entries))
    }
}

#[cfg(test)]
mod tests {
    use changeset_core::{BumpType, ChangeCategory, PackageRelease};

    use super::*;

    fn make_changeset(packages: &[&str], category: ChangeCategory, summary: &str) -> Changeset {
        Changeset {
            summary: summary.to_string(),
            releases: packages
                .iter()
                .map(|name| PackageRelease {
                    name: name.to_string(),
                    bump_type: BumpType::Patch,
                })
                .collect(),
            category,
            consumed_for_prerelease: None,
            graduate: false,
        }
    }

    fn test_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2025, 1, 15).expect("valid date")
    }

    #[test]
    fn empty_aggregator_returns_none() {
        let aggregator = ChangesetAggregator::new();

        let release =
            aggregator.build_package_release("my-crate", &Version::new(1, 0, 0), test_date());

        assert!(release.is_none());
    }

    #[test]
    fn single_changeset_creates_entry() {
        let mut aggregator = ChangesetAggregator::new();
        let changeset = make_changeset(&["my-crate"], ChangeCategory::Fixed, "Fixed a bug");

        aggregator.add_changeset(&changeset);

        let release = aggregator
            .build_package_release("my-crate", &Version::new(1, 0, 0), test_date())
            .expect("release should exist");

        assert_eq!(release.entries.len(), 1);
        assert_eq!(release.entries[0].category, ChangeCategory::Fixed);
        assert_eq!(release.entries[0].description, "Fixed a bug");
        assert!(release.entries[0].package.is_none());
    }

    #[test]
    fn multiple_changesets_same_package() {
        let mut aggregator = ChangesetAggregator::new();

        aggregator.add_changeset(&make_changeset(
            &["my-crate"],
            ChangeCategory::Fixed,
            "Fix 1",
        ));
        aggregator.add_changeset(&make_changeset(
            &["my-crate"],
            ChangeCategory::Added,
            "Feature 1",
        ));

        let release = aggregator
            .build_package_release("my-crate", &Version::new(1, 0, 0), test_date())
            .expect("release should exist");

        assert_eq!(release.entries.len(), 2);
    }

    #[test]
    fn changeset_affecting_multiple_packages() {
        let mut aggregator = ChangesetAggregator::new();
        let changeset = make_changeset(
            &["crate-a", "crate-b"],
            ChangeCategory::Changed,
            "Updated both",
        );

        aggregator.add_changeset(&changeset);

        let release_a = aggregator
            .build_package_release("crate-a", &Version::new(1, 0, 0), test_date())
            .expect("release should exist");
        let release_b = aggregator
            .build_package_release("crate-b", &Version::new(2, 0, 0), test_date())
            .expect("release should exist");

        assert_eq!(release_a.entries.len(), 1);
        assert_eq!(release_b.entries.len(), 1);
        assert_eq!(release_a.version, Version::new(1, 0, 0));
        assert_eq!(release_b.version, Version::new(2, 0, 0));
    }

    #[test]
    fn categories_preserved() {
        let mut aggregator = ChangesetAggregator::new();

        aggregator.add_changeset(&make_changeset(
            &["my-crate"],
            ChangeCategory::Security,
            "Security fix",
        ));

        let release = aggregator
            .build_package_release("my-crate", &Version::new(1, 0, 0), test_date())
            .expect("release should exist");

        assert_eq!(release.entries[0].category, ChangeCategory::Security);
    }

    #[test]
    fn build_root_release_prefixes_packages() {
        let mut aggregator = ChangesetAggregator::new();

        aggregator.add_changeset(&make_changeset(
            &["crate-a"],
            ChangeCategory::Added,
            "Feature A",
        ));
        aggregator.add_changeset(&make_changeset(
            &["crate-b"],
            ChangeCategory::Fixed,
            "Fix B",
        ));

        let packages = vec![
            ("crate-a".to_string(), Version::new(1, 1, 0)),
            ("crate-b".to_string(), Version::new(2, 0, 1)),
        ];

        let release = aggregator
            .build_root_release(&Version::new(1, 0, 0), test_date(), &packages)
            .expect("release should exist");

        assert_eq!(release.entries.len(), 2);

        let has_crate_a = release
            .entries
            .iter()
            .any(|e| e.package.as_deref() == Some("crate-a"));
        let has_crate_b = release
            .entries
            .iter()
            .any(|e| e.package.as_deref() == Some("crate-b"));

        assert!(has_crate_a, "Should have crate-a entry");
        assert!(has_crate_b, "Should have crate-b entry");
    }

    #[test]
    fn root_release_empty_when_no_entries() {
        let aggregator = ChangesetAggregator::new();
        let packages = vec![("my-crate".to_string(), Version::new(1, 0, 0))];

        let release = aggregator.build_root_release(&Version::new(1, 0, 0), test_date(), &packages);

        assert!(release.is_none());
    }
}
