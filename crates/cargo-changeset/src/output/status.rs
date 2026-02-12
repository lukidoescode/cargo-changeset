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

            output.push_str(&format!(
                "  {}: {} -> {} ({:?}){}\n",
                release.name,
                release.current_version,
                release.new_version,
                release.bump_type,
                bump_detail
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
        let bump_strs: Vec<_> = sorted_bumps.iter().map(|b| format!("{b:?}")).collect();
        format!(" (from: {})", bump_strs.join(", "))
    }

    fn format_unchanged_packages(output: &mut String, status: &StatusOutput) {
        if status.unchanged_packages.is_empty() {
            return;
        }

        output.push('\n');
        output.push_str("Packages without changesets:\n");
        for pkg in &status.unchanged_packages {
            output.push_str(&format!("  {} ({})\n", pkg.name, pkg.version));
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
            "Summary: {} changeset(s), {} package(s) affected\n",
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
}

impl StatusFormatter for PlainTextStatusFormatter {
    fn format_status(&self, status: &StatusOutput) -> String {
        let mut output = String::new();

        if status.changesets.is_empty() {
            output.push_str("No pending changesets.\n");
        } else {
            Self::format_changesets(&mut output, status);
            Self::format_projected_releases(&mut output, status);
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
            unchanged_packages: Vec::new(),
            packages_with_inherited_versions: Vec::new(),
            unknown_packages: Vec::new(),
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
        }
    }

    fn make_package_info(name: &str, version: &str) -> PackageInfo {
        PackageInfo {
            name: name.to_string(),
            version: version.parse().expect("valid version"),
            path: PathBuf::from(format!("/mock/{name}")),
        }
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
        status.changesets = vec![Changeset {
            summary: "Fix bug".to_string(),
            releases: vec![PackageRelease {
                name: "my-crate".to_string(),
                bump_type: BumpType::Patch,
            }],
            category: ChangeCategory::Fixed,
        }];
        status.changeset_files = vec![PathBuf::from(".changeset/fix-bug.md")];
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
        assert!(result.contains("my-crate: 1.0.0 -> 1.0.1 (Patch)"));
        assert!(result.contains("Summary: 1 changeset(s), 1 package(s) affected"));
    }

    #[test]
    fn format_multiple_bumps_shows_aggregation() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.changesets = vec![
            Changeset {
                summary: "Fix bug".to_string(),
                releases: vec![PackageRelease {
                    name: "my-crate".to_string(),
                    bump_type: BumpType::Patch,
                }],
                category: ChangeCategory::Fixed,
            },
            Changeset {
                summary: "Add feature".to_string(),
                releases: vec![PackageRelease {
                    name: "my-crate".to_string(),
                    bump_type: BumpType::Minor,
                }],
                category: ChangeCategory::Added,
            },
        ];
        status.changeset_files = vec![
            PathBuf::from(".changeset/fix.md"),
            PathBuf::from(".changeset/feature.md"),
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

        assert!(result.contains("my-crate: 1.0.0 -> 1.1.0 (Minor) (from: Patch, Minor)"));
    }

    #[test]
    fn format_unchanged_packages() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.changesets = vec![Changeset {
            summary: "Fix".to_string(),
            releases: vec![PackageRelease {
                name: "crate-a".to_string(),
                bump_type: BumpType::Patch,
            }],
            category: ChangeCategory::Fixed,
        }];
        status.changeset_files = vec![PathBuf::from(".changeset/fix.md")];
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
        status.changesets = vec![Changeset {
            summary: "Fix".to_string(),
            releases: vec![PackageRelease {
                name: "unknown-crate".to_string(),
                bump_type: BumpType::Patch,
            }],
            category: ChangeCategory::Fixed,
        }];
        status.changeset_files = vec![PathBuf::from(".changeset/fix.md")];
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
        status.changesets = vec![Changeset {
            summary: "Fix".to_string(),
            releases: vec![PackageRelease {
                name: "crate-a".to_string(),
                bump_type: BumpType::Patch,
            }],
            category: ChangeCategory::Fixed,
        }];
        status.changeset_files = vec![PathBuf::from(".changeset/fix.md")];
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
            Changeset {
                summary: "Fix A".to_string(),
                releases: vec![PackageRelease {
                    name: "crate-a".to_string(),
                    bump_type: BumpType::Patch,
                }],
                category: ChangeCategory::Fixed,
            },
            Changeset {
                summary: "Feature B".to_string(),
                releases: vec![PackageRelease {
                    name: "crate-b".to_string(),
                    bump_type: BumpType::Minor,
                }],
                category: ChangeCategory::Added,
            },
        ];
        status.changeset_files = vec![
            PathBuf::from(".changeset/fix-a.md"),
            PathBuf::from(".changeset/feature-b.md"),
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
        assert!(result.contains("crate-a: 1.0.0 -> 1.0.1 (Patch)"));
        assert!(result.contains("crate-b: 2.0.0 -> 2.1.0 (Minor)"));
        assert!(result.contains("Summary: 2 changeset(s), 2 package(s) affected"));
    }

    #[test]
    fn format_changeset_path_without_filename_is_skipped() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.changesets = vec![Changeset {
            summary: "Fix".to_string(),
            releases: vec![PackageRelease {
                name: "my-crate".to_string(),
                bump_type: BumpType::Patch,
            }],
            category: ChangeCategory::Fixed,
        }];
        // PathBuf::from("/") has no file_name()
        status.changeset_files = vec![PathBuf::from("/"), PathBuf::from(".changeset/valid.md")];
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

        // Count should include both, but only valid.md should appear in listing
        assert!(result.contains("Pending changesets: 2"));
        assert!(result.contains("valid.md"));
        // The "/" path should be silently skipped (no crash, no empty line)
        assert!(!result.contains("  \n  valid.md"));
    }

    #[test]
    fn format_all_unknown_packages_shows_summary_with_zero_affected() {
        let formatter = PlainTextStatusFormatter;
        let mut status = empty_status();
        status.changesets = vec![Changeset {
            summary: "Fix unknown".to_string(),
            releases: vec![PackageRelease {
                name: "unknown-crate".to_string(),
                bump_type: BumpType::Patch,
            }],
            category: ChangeCategory::Fixed,
        }];
        status.changeset_files = vec![PathBuf::from(".changeset/fix.md")];
        // No projected_releases since package is unknown
        status.bumps_by_package = {
            let mut map = IndexMap::new();
            map.insert("unknown-crate".to_string(), vec![BumpType::Patch]);
            map
        };
        status.unknown_packages = vec!["unknown-crate".to_string()];

        let result = formatter.format_status(&status);

        assert!(result.contains("Pending changesets: 1"));
        assert!(result.contains("Warning: Unknown packages in changesets:"));
        assert!(result.contains("Summary: 1 changeset(s), 0 package(s) affected"));
    }

    #[test]
    fn format_bump_detail_missing_package_returns_empty() {
        // Directly test the edge case where bumps_by_package doesn't contain the package
        let status = empty_status();

        let result = PlainTextStatusFormatter::format_bump_detail(&status, "nonexistent");

        assert_eq!(result, "");
    }
}
