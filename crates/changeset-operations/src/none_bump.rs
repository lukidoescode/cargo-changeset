use std::collections::HashMap;

use changeset_core::types::{
    BumpType, ChangeCategory, Changeset, NoneBumpBehavior, PackageRelease,
};

use crate::error::{OperationError, Result};

pub(crate) fn find_none_only_packages(changesets: &[Changeset]) -> Vec<String> {
    let mut max_bump: HashMap<&str, BumpType> = HashMap::new();

    for changeset in changesets {
        for release in &changeset.releases {
            let entry = max_bump
                .entry(release.name.as_str())
                .or_insert(BumpType::None);
            if release.bump_type > *entry {
                *entry = release.bump_type;
            }
        }
    }

    let mut result: Vec<String> = max_bump
        .into_iter()
        .filter(|(_, bump)| bump.is_noop())
        .map(|(name, _)| name.to_string())
        .collect();

    result.sort();
    result
}

pub(crate) fn apply_none_bump_behavior(
    changesets: Vec<Changeset>,
    behavior: NoneBumpBehavior,
    promote_message: &str,
) -> Result<Vec<Changeset>> {
    match behavior {
        NoneBumpBehavior::Allow => Ok(changesets),
        NoneBumpBehavior::Disallow => validate_no_none_bumps(changesets),
        _ => Ok(promote_none_to_patch(&changesets, promote_message)),
    }
}

fn validate_no_none_bumps(changesets: Vec<Changeset>) -> Result<Vec<Changeset>> {
    let disallowed = find_none_only_packages(&changesets);
    if disallowed.is_empty() {
        Ok(changesets)
    } else {
        Err(OperationError::NoneBumpDisallowed {
            packages: disallowed,
        })
    }
}

fn promote_none_to_patch(changesets: &[Changeset], message: &str) -> Vec<Changeset> {
    changesets
        .iter()
        .flat_map(|cs| split_changeset(cs, message))
        .collect()
}

fn split_changeset(changeset: &Changeset, message: &str) -> Vec<Changeset> {
    let none_releases: Vec<&PackageRelease> = changeset
        .releases
        .iter()
        .filter(|r| r.bump_type.is_noop())
        .collect();

    let non_none_releases: Vec<&PackageRelease> = changeset
        .releases
        .iter()
        .filter(|r| !r.bump_type.is_noop())
        .collect();

    match (none_releases.is_empty(), non_none_releases.is_empty()) {
        (true, _) => vec![changeset.clone()],
        (false, true) => vec![Changeset {
            summary: message.to_string(),
            category: ChangeCategory::Changed,
            releases: changeset
                .releases
                .iter()
                .map(|r| PackageRelease {
                    name: r.name.clone(),
                    bump_type: BumpType::Patch,
                })
                .collect(),
            consumed_for_prerelease: changeset.consumed_for_prerelease.clone(),
            graduate: changeset.graduate,
        }],
        (false, false) => {
            let original = Changeset {
                summary: changeset.summary.clone(),
                category: changeset.category,
                releases: non_none_releases
                    .iter()
                    .map(|r| PackageRelease {
                        name: r.name.clone(),
                        bump_type: r.bump_type,
                    })
                    .collect(),
                consumed_for_prerelease: changeset.consumed_for_prerelease.clone(),
                graduate: changeset.graduate,
            };

            let promoted = Changeset {
                summary: message.to_string(),
                category: ChangeCategory::Changed,
                releases: none_releases
                    .iter()
                    .map(|r| PackageRelease {
                        name: r.name.clone(),
                        bump_type: BumpType::Patch,
                    })
                    .collect(),
                consumed_for_prerelease: None,
                graduate: false,
            };

            vec![original, promoted]
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::*;

    fn make_changeset(
        summary: &str,
        category: ChangeCategory,
        releases: Vec<(&str, BumpType)>,
    ) -> Changeset {
        Changeset {
            summary: summary.to_string(),
            category,
            releases: releases
                .into_iter()
                .map(|(name, bump_type)| PackageRelease {
                    name: name.to_string(),
                    bump_type,
                })
                .collect(),
            consumed_for_prerelease: None,
            graduate: false,
        }
    }

    #[test]
    fn allow_passes_through_unchanged() -> Result<()> {
        let changesets = vec![make_changeset(
            "fix a bug",
            ChangeCategory::Fixed,
            vec![("crate-a", BumpType::None)],
        )];
        let expected = changesets.clone();

        let result = apply_none_bump_behavior(changesets, NoneBumpBehavior::Allow, "auto")?;

        assert_eq!(result, expected);
        Ok(())
    }

    #[test]
    fn promote_rewrites_all_none_changeset_to_patch() -> Result<()> {
        let changesets = vec![make_changeset(
            "original summary",
            ChangeCategory::Fixed,
            vec![("crate-a", BumpType::None), ("crate-b", BumpType::None)],
        )];

        let result =
            apply_none_bump_behavior(changesets, NoneBumpBehavior::PromoteToPatch, "auto")?;

        assert_eq!(result.len(), 1);
        assert!(
            result[0]
                .releases
                .iter()
                .all(|r| r.bump_type == BumpType::Patch)
        );
        Ok(())
    }

    #[test]
    fn promote_uses_custom_message_for_all_none_changesets() -> Result<()> {
        let changesets = vec![make_changeset(
            "original summary",
            ChangeCategory::Fixed,
            vec![("crate-a", BumpType::None)],
        )];

        let result = apply_none_bump_behavior(
            changesets,
            NoneBumpBehavior::PromoteToPatch,
            "promoted message",
        )?;

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].summary, "promoted message");
        assert_eq!(result[0].category, ChangeCategory::Changed);
        Ok(())
    }

    #[test]
    fn promote_splits_mixed_changeset_into_two() -> Result<()> {
        let changesets = vec![make_changeset(
            "mixed change",
            ChangeCategory::Fixed,
            vec![("crate-a", BumpType::Patch), ("crate-b", BumpType::None)],
        )];

        let result =
            apply_none_bump_behavior(changesets, NoneBumpBehavior::PromoteToPatch, "auto")?;

        assert_eq!(result.len(), 2);

        let original = &result[0];
        assert_eq!(original.summary, "mixed change");
        assert_eq!(original.category, ChangeCategory::Fixed);
        assert_eq!(original.releases.len(), 1);
        assert_eq!(original.releases[0].name, "crate-a");
        assert_eq!(original.releases[0].bump_type, BumpType::Patch);

        let promoted = &result[1];
        assert_eq!(promoted.summary, "auto");
        assert_eq!(promoted.category, ChangeCategory::Changed);
        assert_eq!(promoted.releases.len(), 1);
        assert_eq!(promoted.releases[0].name, "crate-b");
        assert_eq!(promoted.releases[0].bump_type, BumpType::Patch);
        assert_eq!(promoted.consumed_for_prerelease, None);
        assert!(!promoted.graduate);
        Ok(())
    }

    #[test]
    fn promote_preserves_no_none_changeset_unchanged() -> Result<()> {
        let changesets = vec![make_changeset(
            "a real change",
            ChangeCategory::Added,
            vec![("crate-a", BumpType::Minor)],
        )];
        let expected = changesets.clone();

        let result =
            apply_none_bump_behavior(changesets, NoneBumpBehavior::PromoteToPatch, "auto")?;

        assert_eq!(result, expected);
        Ok(())
    }

    #[test]
    fn disallow_errors_on_none_only_packages() {
        let changesets = vec![make_changeset(
            "internal change",
            ChangeCategory::Changed,
            vec![("crate-a", BumpType::None)],
        )];

        let result = apply_none_bump_behavior(changesets, NoneBumpBehavior::Disallow, "auto");

        assert!(matches!(
            result,
            Err(OperationError::NoneBumpDisallowed { ref packages }) if packages.contains(&"crate-a".to_string())
        ));
    }

    #[test]
    fn disallow_permits_mixed_bumps() -> Result<()> {
        let changesets = vec![
            make_changeset(
                "first",
                ChangeCategory::Changed,
                vec![("crate-a", BumpType::Patch)],
            ),
            make_changeset(
                "second",
                ChangeCategory::Changed,
                vec![("crate-a", BumpType::None)],
            ),
        ];
        let expected = changesets.clone();

        let result = apply_none_bump_behavior(changesets, NoneBumpBehavior::Disallow, "auto")?;

        assert_eq!(result, expected);
        Ok(())
    }

    #[test]
    fn empty_changeset_list_returns_empty_for_all_behaviors() -> Result<()> {
        for behavior in [
            NoneBumpBehavior::Allow,
            NoneBumpBehavior::Disallow,
            NoneBumpBehavior::PromoteToPatch,
        ] {
            let result = apply_none_bump_behavior(vec![], behavior, "auto")?;
            assert!(result.is_empty());
        }
        Ok(())
    }

    #[test]
    fn disallow_errors_with_multiple_none_packages_sorted() {
        let changesets = vec![
            make_changeset(
                "change b",
                ChangeCategory::Changed,
                vec![("zeta-crate", BumpType::None)],
            ),
            make_changeset(
                "change a",
                ChangeCategory::Changed,
                vec![("alpha-crate", BumpType::None)],
            ),
        ];

        let result = apply_none_bump_behavior(changesets, NoneBumpBehavior::Disallow, "auto");

        match result {
            Err(OperationError::NoneBumpDisallowed { packages }) => {
                assert_eq!(packages, vec!["alpha-crate", "zeta-crate"]);
            }
            other => panic!("Expected NoneBumpDisallowed error, got {other:?}"),
        }
    }

    #[test]
    fn find_none_only_packages_returns_empty_for_empty_input() {
        assert!(find_none_only_packages(&[]).is_empty());
    }
}
