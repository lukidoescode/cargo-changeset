use changeset_operations::operations::StatusOutput;

pub(crate) trait StatusFormatter {
    fn format_status(&self, output: &StatusOutput) -> String;
}

pub(crate) struct PlainTextStatusFormatter;

impl PlainTextStatusFormatter {
    fn format_changesets(output: &mut String, status: &StatusOutput) {
        output.push_str(&format!(
            "Pending changesets: {}\n",
            status.changeset_files.len()
        ));
        for file in &status.changeset_files {
            if let Some(name) = file.file_name() {
                output.push_str(&format!("  {}\n", name.to_string_lossy()));
            }
        }
    }

    fn format_projected_releases(output: &mut String, status: &StatusOutput) {
        if status.projected_releases.is_empty() {
            return;
        }

        output.push('\n');
        output.push_str("Projected releases:\n");

        for release in &status.projected_releases {
            let bump_detail = Self::format_bump_detail(status, &release.name);
            let auto_label = if release.auto_bumped {
                " (dependency update)"
            } else {
                ""
            };

            output.push_str(&format!(
                "  {}: {} -> {} ({}){}{}\n",
                release.name,
                release.current_version,
                release.new_version,
                release.bump_type,
                bump_detail,
                auto_label,
            ));
        }
    }

    fn format_bump_detail(status: &StatusOutput, package_name: &str) -> String {
        let Some(bumps) = status.bumps_by_package.get(package_name) else {
            return String::new();
        };

        if bumps.len() <= 1 {
            return String::new();
        }

        let mut sorted_bumps: Vec<_> = bumps.iter().collect();
        sorted_bumps.sort();
        let bump_strs: Vec<_> = sorted_bumps.iter().map(|b| format!("{b}")).collect();
        format!(" (from: {})", bump_strs.join(", "))
    }

    fn format_none_bump_packages(output: &mut String, status: &StatusOutput) {
        if status.none_bump_packages.is_empty() {
            return;
        }

        output.push('\n');
        output.push_str("Packages with no version bump:\n");
        for name in &status.none_bump_packages {
            output.push_str(&format!("  {name} (none)\n"));
        }
    }

    fn format_unchanged_packages(output: &mut String, status: &StatusOutput) {
        if status.unchanged_packages.is_empty() {
            return;
        }

        output.push('\n');
        output.push_str("Packages without changesets:\n");
        for pkg in &status.unchanged_packages {
            output.push_str(&format!("  {} ({})\n", pkg.name(), pkg.version()));
        }
    }

    fn format_unknown_packages(output: &mut String, status: &StatusOutput) {
        if status.unknown_packages.is_empty() {
            return;
        }

        output.push('\n');
        output.push_str("Warning: Unknown packages in changesets:\n");
        for pkg in &status.unknown_packages {
            output.push_str(&format!("  {pkg}\n"));
        }
    }

    fn format_summary(output: &mut String, status: &StatusOutput) {
        output.push('\n');
        output.push_str(&format!(
            "Summary: {} changeset(s), {} package(s) to release\n",
            status.changesets.len(),
            status.projected_releases.len()
        ));
    }

    fn format_inherited_versions_warning(output: &mut String, status: &StatusOutput) {
        if status.packages_with_inherited_versions.is_empty() {
            return;
        }

        output.push('\n');
        output.push_str("Warning: Packages with inherited versions:\n");
        for pkg in &status.packages_with_inherited_versions {
            output.push_str(&format!("  {pkg}\n"));
        }
        output.push_str("  Release will require --convert flag\n");
    }

    fn format_uncovered_dependents(output: &mut String, status: &StatusOutput) {
        if status.uncovered_dependents.is_empty() {
            return;
        }

        output.push('\n');
        output.push_str("Transitive dependents needing changesets:\n");
        for (name, deps) in &status.uncovered_dependents {
            output.push_str(&format!("  {name} (depends on: {})\n", deps.join(", ")));
        }
    }

    fn format_consumed_prerelease_changesets(output: &mut String, status: &StatusOutput) {
        const MAX_DISPLAYED: usize = 10;

        if status.consumed_prerelease_changesets.is_empty() {
            return;
        }

        output.push('\n');
        output.push_str("Consumed pre-release changesets:\n");

        let total = status.consumed_prerelease_changesets.len();
        let display_count = total.min(MAX_DISPLAYED);

        for (path, version) in status
            .consumed_prerelease_changesets
            .iter()
            .take(display_count)
        {
            if let Some(name) = path.file_name() {
                output.push_str(&format!(
                    "  - {} (consumed for {})\n",
                    name.to_string_lossy(),
                    version
                ));
            }
        }

        if total > MAX_DISPLAYED {
            output.push_str(&format!("  ... and {} more\n", total - MAX_DISPLAYED));
        }
    }
}

impl StatusFormatter for PlainTextStatusFormatter {
    fn format_status(&self, status: &StatusOutput) -> String {
        let mut output = String::new();

        if status.changesets.is_empty() && status.consumed_prerelease_changesets.is_empty() {
            output.push_str("No pending changesets.\n");
        } else if status.changesets.is_empty() {
            output.push_str("No pending changesets.\n");
            Self::format_consumed_prerelease_changesets(&mut output, status);
        } else {
            Self::format_changesets(&mut output, status);
            Self::format_consumed_prerelease_changesets(&mut output, status);
            Self::format_projected_releases(&mut output, status);
            Self::format_none_bump_packages(&mut output, status);
            Self::format_uncovered_dependents(&mut output, status);
            Self::format_unchanged_packages(&mut output, status);
            Self::format_unknown_packages(&mut output, status);
            Self::format_summary(&mut output, status);
        }

        Self::format_inherited_versions_warning(&mut output, status);

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use changeset_core::{BumpType, ChangeCategory, Changeset, PackageInfo, PackageRelease};
    use changeset_operations::operations::PackageVersion;
    use indexmap::IndexMap;
    use std::path::PathBuf;

    fn empty_status() -> StatusOutput {
        StatusOutput {
            changesets: Vec::new(),
            changeset_files: Vec::new(),
            projected_releases: Vec::new(),
            bumps_by_package: IndexMap::new(),
            none_bump_packages: Vec::new(),
            unchanged_packages: Vec::new(),
            packages_with_inherited_versions: Vec::new(),
            unknown_packages: Vec::new(),
            consumed_prerelease_changesets: Vec::new(),
            uncovered_dependents: Vec::new(),
        }
    }

    fn make_package_version(
        name: &str,
        current: &str,
        new: &str,
        bump: BumpType,
    ) -> PackageVersion {
        PackageVersion {
            name: name.to_string(),
            current_version: current.parse().expect("valid version"),
            new_version: new.parse().expect("valid version"),
            bump_type: bump,
            auto_bumped: false,
        }
    }

    fn make_package_info(name: &str, version: &str) -> PackageInfo {
        PackageInfo::new(
            name.to_string(),
            version.parse().expect("valid version"),
            PathBuf::from(format!("/mock/{name}")),
        )
    }

    fn make_changeset(
        packages: &[(&str, BumpType)],
        category: ChangeCategory,
        summary: &str,
    ) -> Changeset {
        Changeset::new(
            summary.to_string(),
            packages
                .iter()
                .map(|(name, bump)| PackageRelease::new(name.to_string(), *bump))
                .collect(),
            category,
        )
    }

    #[test]
    fn format_no_changesets() {
        let formatter = PlainTextStatusFormatter;
        let status = empty_status();

        let result = formatter.format_status(&status);

        assert_eq!(result, "No pending changesets.\n");
    }

    #[test]
    fn format_no_changesets_with_inherited_versions() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.packages_with_inherited_versions = vec!["crate-a".to_string()];

        let result = formatter.format_status(&status);

        assert!(result.contains("No pending changesets."));
        assert!(result.contains("Warning: Packages with inherited versions:"));
        assert!(result.contains("crate-a"));
        assert!(result.contains("--convert flag"));
    }

    #[test]
    fn format_single_changeset_with_release() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.changesets = vec![make_changeset(
            &[("my-crate", BumpType::Patch)],
            ChangeCategory::Fixed,
            "Fix bug",
        )];
        status.changeset_files = vec![PathBuf::from(".changeset/changesets/fix-bug.md")];
        status.projected_releases = vec![make_package_version(
            "my-crate",
            "1.0.0",
            "1.0.1",
            BumpType::Patch,
        )];
        status.bumps_by_package = {
            let mut map = IndexMap::new();
            map.insert("my-crate".to_string(), vec![BumpType::Patch]);
            map
        };

        let result = formatter.format_status(&status);

        assert!(result.contains("Pending changesets: 1"));
        assert!(result.contains("fix-bug.md"));
        assert!(result.contains("Projected releases:"));
        assert!(result.contains("my-crate: 1.0.0 -> 1.0.1 (patch)"));
        assert!(result.contains("Summary: 1 changeset(s), 1 package(s) to release"));
    }

    #[test]
    fn format_multiple_bumps_shows_aggregation() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.changesets = vec![
            make_changeset(
                &[("my-crate", BumpType::Patch)],
                ChangeCategory::Fixed,
                "Fix bug",
            ),
            make_changeset(
                &[("my-crate", BumpType::Minor)],
                ChangeCategory::Added,
                "Add feature",
            ),
        ];
        status.changeset_files = vec![
            PathBuf::from(".changeset/changesets/fix.md"),
            PathBuf::from(".changeset/changesets/feature.md"),
        ];
        status.projected_releases = vec![make_package_version(
            "my-crate",
            "1.0.0",
            "1.1.0",
            BumpType::Minor,
        )];
        status.bumps_by_package = {
            let mut map = IndexMap::new();
            map.insert(
                "my-crate".to_string(),
                vec![BumpType::Patch, BumpType::Minor],
            );
            map
        };

        let result = formatter.format_status(&status);

        assert!(result.contains("my-crate: 1.0.0 -> 1.1.0 (minor) (from: patch, minor)"));
    }

    #[test]
    fn format_unchanged_packages() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.changesets = vec![make_changeset(
            &[("crate-a", BumpType::Patch)],
            ChangeCategory::Fixed,
            "Fix",
        )];
        status.changeset_files = vec![PathBuf::from(".changeset/changesets/fix.md")];
        status.projected_releases = vec![make_package_version(
            "crate-a",
            "1.0.0",
            "1.0.1",
            BumpType::Patch,
        )];
        status.bumps_by_package = {
            let mut map = IndexMap::new();
            map.insert("crate-a".to_string(), vec![BumpType::Patch]);
            map
        };
        status.unchanged_packages = vec![make_package_info("crate-b", "2.0.0")];

        let result = formatter.format_status(&status);

        assert!(result.contains("Packages without changesets:"));
        assert!(result.contains("crate-b (2.0.0)"));
    }

    #[test]
    fn format_unknown_packages() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.changesets = vec![make_changeset(
            &[("unknown-crate", BumpType::Patch)],
            ChangeCategory::Fixed,
            "Fix",
        )];
        status.changeset_files = vec![PathBuf::from(".changeset/changesets/fix.md")];
        status.bumps_by_package = {
            let mut map = IndexMap::new();
            map.insert("unknown-crate".to_string(), vec![BumpType::Patch]);
            map
        };
        status.unknown_packages = vec!["unknown-crate".to_string()];

        let result = formatter.format_status(&status);

        assert!(result.contains("Warning: Unknown packages in changesets:"));
        assert!(result.contains("unknown-crate"));
    }

    #[test]
    fn format_inherited_versions_with_changesets() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.changesets = vec![make_changeset(
            &[("crate-a", BumpType::Patch)],
            ChangeCategory::Fixed,
            "Fix",
        )];
        status.changeset_files = vec![PathBuf::from(".changeset/changesets/fix.md")];
        status.projected_releases = vec![make_package_version(
            "crate-a",
            "1.0.0",
            "1.0.1",
            BumpType::Patch,
        )];
        status.bumps_by_package = {
            let mut map = IndexMap::new();
            map.insert("crate-a".to_string(), vec![BumpType::Patch]);
            map
        };
        status.packages_with_inherited_versions = vec!["crate-a".to_string()];

        let result = formatter.format_status(&status);

        assert!(result.contains("Pending changesets: 1"));
        assert!(result.contains("Warning: Packages with inherited versions:"));
        assert!(result.contains("crate-a"));
        assert!(result.contains("--convert flag"));
    }

    #[test]
    fn format_multiple_packages_multiple_changesets() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.changesets = vec![
            make_changeset(
                &[("crate-a", BumpType::Patch)],
                ChangeCategory::Fixed,
                "Fix A",
            ),
            make_changeset(
                &[("crate-b", BumpType::Minor)],
                ChangeCategory::Added,
                "Feature B",
            ),
        ];
        status.changeset_files = vec![
            PathBuf::from(".changeset/changesets/fix-a.md"),
            PathBuf::from(".changeset/changesets/feature-b.md"),
        ];
        status.projected_releases = vec![
            make_package_version("crate-a", "1.0.0", "1.0.1", BumpType::Patch),
            make_package_version("crate-b", "2.0.0", "2.1.0", BumpType::Minor),
        ];
        status.bumps_by_package = {
            let mut map = IndexMap::new();
            map.insert("crate-a".to_string(), vec![BumpType::Patch]);
            map.insert("crate-b".to_string(), vec![BumpType::Minor]);
            map
        };

        let result = formatter.format_status(&status);

        assert!(result.contains("Pending changesets: 2"));
        assert!(result.contains("crate-a: 1.0.0 -> 1.0.1 (patch)"));
        assert!(result.contains("crate-b: 2.0.0 -> 2.1.0 (minor)"));
        assert!(result.contains("Summary: 2 changeset(s), 2 package(s) to release"));
    }

    #[test]
    fn format_changeset_path_without_filename_is_skipped() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.changesets = vec![make_changeset(
            &[("my-crate", BumpType::Patch)],
            ChangeCategory::Fixed,
            "Fix",
        )];
        status.changeset_files = vec![
            PathBuf::from("/"),
            PathBuf::from(".changeset/changesets/valid.md"),
        ];
        status.projected_releases = vec![make_package_version(
            "my-crate",
            "1.0.0",
            "1.0.1",
            BumpType::Patch,
        )];
        status.bumps_by_package = {
            let mut map = IndexMap::new();
            map.insert("my-crate".to_string(), vec![BumpType::Patch]);
            map
        };

        let result = formatter.format_status(&status);

        assert!(result.contains("Pending changesets: 2"));
        assert!(result.contains("valid.md"));
        assert!(!result.contains("  \n  valid.md"));
    }

    #[test]
    fn format_all_unknown_packages_shows_summary_with_zero_affected() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.changesets = vec![make_changeset(
            &[("unknown-crate", BumpType::Patch)],
            ChangeCategory::Fixed,
            "Fix unknown",
        )];
        status.changeset_files = vec![PathBuf::from(".changeset/changesets/fix.md")];
        status.bumps_by_package = {
            let mut map = IndexMap::new();
            map.insert("unknown-crate".to_string(), vec![BumpType::Patch]);
            map
        };
        status.unknown_packages = vec!["unknown-crate".to_string()];

        let result = formatter.format_status(&status);

        assert!(result.contains("Pending changesets: 1"));
        assert!(result.contains("Warning: Unknown packages in changesets:"));
        assert!(result.contains("Summary: 1 changeset(s), 0 package(s) to release"));
    }

    #[test]
    fn format_bump_detail_missing_package_returns_empty() {
        let status = empty_status();

        let result = PlainTextStatusFormatter::format_bump_detail(&status, "nonexistent");

        assert_eq!(result, "");
    }

    #[test]
    fn format_consumed_prerelease_changesets_section() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.consumed_prerelease_changesets = vec![
            (
                PathBuf::from(".changeset/changesets/fix-bug.md"),
                "1.0.1-alpha.1".to_string(),
            ),
            (
                PathBuf::from(".changeset/changesets/add-feature.md"),
                "1.0.1-alpha.2".to_string(),
            ),
        ];

        let result = formatter.format_status(&status);

        assert!(result.contains("No pending changesets."));
        assert!(result.contains("Consumed pre-release changesets:"));
        assert!(result.contains("- fix-bug.md (consumed for 1.0.1-alpha.1)"));
        assert!(result.contains("- add-feature.md (consumed for 1.0.1-alpha.2)"));
    }

    #[test]
    fn format_consumed_changesets_with_pending_changesets() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.changesets = vec![make_changeset(
            &[("my-crate", BumpType::Patch)],
            ChangeCategory::Fixed,
            "Fix another bug",
        )];
        status.changeset_files = vec![PathBuf::from(".changeset/changesets/fix-another.md")];
        status.projected_releases = vec![make_package_version(
            "my-crate",
            "1.0.1",
            "1.0.2",
            BumpType::Patch,
        )];
        status.bumps_by_package = {
            let mut map = IndexMap::new();
            map.insert("my-crate".to_string(), vec![BumpType::Patch]);
            map
        };
        status.consumed_prerelease_changesets = vec![(
            PathBuf::from(".changeset/changesets/fix-bug.md"),
            "1.0.1-alpha.1".to_string(),
        )];

        let result = formatter.format_status(&status);

        assert!(result.contains("Pending changesets: 1"));
        assert!(result.contains("fix-another.md"));
        assert!(result.contains("Consumed pre-release changesets:"));
        assert!(result.contains("- fix-bug.md (consumed for 1.0.1-alpha.1)"));
        assert!(result.contains("Projected releases:"));
    }

    #[test]
    fn format_consumed_changesets_appears_after_pending_before_projected() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.changesets = vec![make_changeset(
            &[("my-crate", BumpType::Patch)],
            ChangeCategory::Fixed,
            "Fix bug",
        )];
        status.changeset_files = vec![PathBuf::from(".changeset/changesets/fix.md")];
        status.projected_releases = vec![make_package_version(
            "my-crate",
            "1.0.0",
            "1.0.1",
            BumpType::Patch,
        )];
        status.bumps_by_package = {
            let mut map = IndexMap::new();
            map.insert("my-crate".to_string(), vec![BumpType::Patch]);
            map
        };
        status.consumed_prerelease_changesets = vec![(
            PathBuf::from(".changeset/changesets/consumed.md"),
            "1.0.1-alpha.1".to_string(),
        )];

        let result = formatter.format_status(&status);

        let pending_pos = result
            .find("Pending changesets:")
            .expect("should contain pending");
        let consumed_pos = result
            .find("Consumed pre-release changesets:")
            .expect("should contain consumed");
        let projected_pos = result
            .find("Projected releases:")
            .expect("should contain projected");

        assert!(
            pending_pos < consumed_pos,
            "Pending should appear before consumed"
        );
        assert!(
            consumed_pos < projected_pos,
            "Consumed should appear before projected"
        );
    }

    #[test]
    fn format_no_consumed_section_when_empty() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.changesets = vec![make_changeset(
            &[("my-crate", BumpType::Patch)],
            ChangeCategory::Fixed,
            "Fix bug",
        )];
        status.changeset_files = vec![PathBuf::from(".changeset/changesets/fix.md")];
        status.projected_releases = vec![make_package_version(
            "my-crate",
            "1.0.0",
            "1.0.1",
            BumpType::Patch,
        )];
        status.bumps_by_package = {
            let mut map = IndexMap::new();
            map.insert("my-crate".to_string(), vec![BumpType::Patch]);
            map
        };

        let result = formatter.format_status(&status);

        assert!(!result.contains("Consumed pre-release changesets:"));
    }

    #[test]
    fn format_consumed_changesets_truncates_large_lists() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.consumed_prerelease_changesets = (1..=15)
            .map(|i| {
                (
                    PathBuf::from(format!(".changeset/changesets/fix{i}.md")),
                    format!("1.0.1-alpha.{i}"),
                )
            })
            .collect();

        let result = formatter.format_status(&status);

        assert!(result.contains("Consumed pre-release changesets:"));
        assert!(result.contains("fix1.md"));
        assert!(result.contains("fix10.md"));
        assert!(
            !result.contains("fix11.md"),
            "11th entry should be truncated"
        );
        assert!(
            result.contains("... and 5 more"),
            "should show truncation message"
        );
    }

    #[test]
    fn format_consumed_changesets_no_truncation_when_under_limit() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.consumed_prerelease_changesets = (1..=5)
            .map(|i| {
                (
                    PathBuf::from(format!(".changeset/changesets/fix{i}.md")),
                    format!("1.0.1-alpha.{i}"),
                )
            })
            .collect();

        let result = formatter.format_status(&status);

        assert!(result.contains("fix1.md"));
        assert!(result.contains("fix5.md"));
        assert!(
            !result.contains("... and"),
            "should not show truncation for small lists"
        );
    }

    #[test]
    fn format_none_only_packages_section() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.changesets = vec![make_changeset(
            &[("internal-crate", BumpType::None)],
            ChangeCategory::Changed,
            "Refactor",
        )];
        status.changeset_files = vec![PathBuf::from(".changeset/changesets/refactor.md")];
        status.bumps_by_package = {
            let mut map = IndexMap::new();
            map.insert("internal-crate".to_string(), vec![BumpType::None]);
            map
        };
        status.none_bump_packages = vec!["internal-crate".to_string()];

        let result = formatter.format_status(&status);

        assert!(result.contains("Packages with no version bump:"));
        assert!(result.contains("internal-crate (none)"));
        assert!(!result.contains("Projected releases:"));
    }

    #[test]
    fn format_mixed_none_and_patch_packages() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.changesets = vec![
            make_changeset(
                &[("internal-crate", BumpType::None)],
                ChangeCategory::Changed,
                "Refactor",
            ),
            make_changeset(
                &[("public-crate", BumpType::Patch)],
                ChangeCategory::Fixed,
                "Fix",
            ),
        ];
        status.changeset_files = vec![
            PathBuf::from(".changeset/changesets/refactor.md"),
            PathBuf::from(".changeset/changesets/fix.md"),
        ];
        status.projected_releases = vec![make_package_version(
            "public-crate",
            "1.0.0",
            "1.0.1",
            BumpType::Patch,
        )];
        status.bumps_by_package = {
            let mut map = IndexMap::new();
            map.insert("internal-crate".to_string(), vec![BumpType::None]);
            map.insert("public-crate".to_string(), vec![BumpType::Patch]);
            map
        };
        status.none_bump_packages = vec!["internal-crate".to_string()];

        let result = formatter.format_status(&status);

        assert!(result.contains("Projected releases:"));
        assert!(result.contains("public-crate: 1.0.0 -> 1.0.1 (patch)"));
        assert!(result.contains("Packages with no version bump:"));
        assert!(result.contains("internal-crate (none)"));
    }

    #[test]
    fn format_uncovered_dependents_section() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.changesets = vec![make_changeset(
            &[("core", BumpType::Patch)],
            ChangeCategory::Fixed,
            "Fix core",
        )];
        status.changeset_files = vec![PathBuf::from(".changeset/changesets/fix.md")];
        status.projected_releases = vec![make_package_version(
            "core",
            "1.0.0",
            "1.0.1",
            BumpType::Patch,
        )];
        status.bumps_by_package = {
            let mut map = IndexMap::new();
            map.insert("core".to_string(), vec![BumpType::Patch]);
            map
        };
        status.uncovered_dependents = vec![
            ("app".to_string(), vec!["core".to_string()]),
            (
                "cli".to_string(),
                vec!["app".to_string(), "core".to_string()],
            ),
        ];

        let result = formatter.format_status(&status);

        assert!(result.contains("Transitive dependents needing changesets:"));
        assert!(result.contains("app (depends on: core)"));
        assert!(result.contains("cli (depends on: app, core)"));
    }

    #[test]
    fn format_auto_bumped_shows_dependency_update_label() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.changesets = vec![make_changeset(
            &[("core-crate", BumpType::Patch)],
            ChangeCategory::Fixed,
            "Fix core",
        )];
        status.changeset_files = vec![PathBuf::from(".changeset/changesets/fix-core.md")];
        status.projected_releases = vec![
            make_package_version("core-crate", "1.0.0", "1.0.1", BumpType::Patch),
            PackageVersion {
                name: "dependent-crate".to_string(),
                current_version: "2.0.0".parse().expect("valid version"),
                new_version: "2.0.1".parse().expect("valid version"),
                bump_type: BumpType::Patch,
                auto_bumped: true,
            },
        ];
        status.bumps_by_package = {
            let mut map = IndexMap::new();
            map.insert("core-crate".to_string(), vec![BumpType::Patch]);
            map
        };

        let result = formatter.format_status(&status);

        assert!(
            result.contains("dependent-crate: 2.0.0 -> 2.0.1 (patch) (dependency update)"),
            "auto-bumped package should show dependency update label, got: {result}"
        );
        assert!(
            !result.contains("core-crate: 1.0.0 -> 1.0.1 (patch) (dependency update)"),
            "manually bumped package should not show dependency update label"
        );
    }

    #[test]
    fn format_no_uncovered_dependents_when_empty() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.changesets = vec![make_changeset(
            &[("my-crate", BumpType::Patch)],
            ChangeCategory::Fixed,
            "Fix",
        )];
        status.changeset_files = vec![PathBuf::from(".changeset/changesets/fix.md")];
        status.projected_releases = vec![make_package_version(
            "my-crate",
            "1.0.0",
            "1.0.1",
            BumpType::Patch,
        )];
        status.bumps_by_package = {
            let mut map = IndexMap::new();
            map.insert("my-crate".to_string(), vec![BumpType::Patch]);
            map
        };

        let result = formatter.format_status(&status);

        assert!(!result.contains("Transitive dependents needing changesets:"));
    }
}
