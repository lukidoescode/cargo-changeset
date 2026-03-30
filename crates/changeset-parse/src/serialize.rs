use indexmap::IndexMap;
use serde::Serialize;

use crate::error::{FormatError, ValidationError};
use crate::parse::FRONT_MATTER_DELIMITER;
use changeset_core::{BumpType, ChangeCategory, Changeset};

#[derive(Serialize)]
struct FrontMatterOutput<'a> {
    #[serde(skip_serializing_if = "is_default_category")]
    category: ChangeCategory,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "consumedForPrerelease"
    )]
    consumed_for_prerelease: Option<&'a str>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    graduate: bool,
    #[serde(flatten)]
    releases: IndexMap<&'a str, BumpType>,
}

#[must_use = "serialization result should be handled"]
pub fn serialize_changeset(changeset: &Changeset) -> Result<String, FormatError> {
    if changeset.releases().is_empty() {
        return Err(ValidationError::NoReleases.into());
    }

    let releases_map: IndexMap<&str, BumpType> = changeset
        .releases()
        .iter()
        .map(|r| (r.name().as_str(), r.bump_type()))
        .collect();

    let front_matter = FrontMatterOutput {
        category: changeset.category(),
        consumed_for_prerelease: changeset.consumed_for_prerelease().map(String::as_str),
        graduate: changeset.graduate(),
        releases: releases_map,
    };

    let yaml = serde_yml::to_string(&front_matter)?;

    let mut output = String::new();
    output.push_str(FRONT_MATTER_DELIMITER);
    output.push('\n');
    output.push_str(&yaml);
    output.push_str(FRONT_MATTER_DELIMITER);
    output.push('\n');

    if !changeset.summary().is_empty() {
        output.push_str(changeset.summary());
        output.push('\n');
    }

    Ok(output)
}

fn is_default_category(category: &ChangeCategory) -> bool {
    *category == ChangeCategory::default()
}

#[cfg(test)]
mod tests {
    use changeset_core::PackageRelease;

    use super::*;
    use crate::parse::parse_changeset;

    #[test]
    fn roundtrip() {
        let original = Changeset::new(
            "Test summary".to_string(),
            vec![
                PackageRelease::new("crate-a".to_string(), BumpType::Minor),
                PackageRelease::new("crate-b".to_string(), BumpType::Patch),
            ],
            ChangeCategory::default(),
        );

        let serialized = serialize_changeset(&original).expect("should serialize");
        let parsed = parse_changeset(&serialized).expect("should parse");

        assert_eq!(parsed.summary(), original.summary());
        assert_eq!(parsed.releases().len(), original.releases().len());
        assert_eq!(parsed.category(), original.category());
        assert_eq!(
            parsed.consumed_for_prerelease(),
            original.consumed_for_prerelease()
        );

        for (original_release, parsed_release) in
            original.releases().iter().zip(parsed.releases().iter())
        {
            assert_eq!(parsed_release.name(), original_release.name());
            assert_eq!(parsed_release.bump_type(), original_release.bump_type());
        }
    }

    #[test]
    fn preserves_order() {
        let original = Changeset::new(
            "Test".to_string(),
            vec![
                PackageRelease::new("zebra".to_string(), BumpType::Major),
                PackageRelease::new("apple".to_string(), BumpType::Minor),
                PackageRelease::new("banana".to_string(), BumpType::Patch),
            ],
            ChangeCategory::default(),
        );

        let serialized = serialize_changeset(&original).expect("should serialize");
        let parsed = parse_changeset(&serialized).expect("should parse");

        assert_eq!(parsed.releases()[0].name(), "zebra");
        assert_eq!(parsed.releases()[1].name(), "apple");
        assert_eq!(parsed.releases()[2].name(), "banana");
    }

    #[test]
    fn error_empty_releases() {
        let changeset = Changeset::new(
            "Some summary".to_string(),
            vec![],
            ChangeCategory::default(),
        );

        let err = serialize_changeset(&changeset).expect_err("should fail");
        assert!(err.to_string().contains("at least one release"));
    }

    #[test]
    fn roundtrip_with_category() {
        let original = Changeset::new(
            "Fixed a bug".to_string(),
            vec![PackageRelease::new("my-crate".to_string(), BumpType::Patch)],
            ChangeCategory::Fixed,
        );

        let serialized = serialize_changeset(&original).expect("should serialize");
        let parsed = parse_changeset(&serialized).expect("should parse");

        assert_eq!(parsed.category(), ChangeCategory::Fixed);
        assert_eq!(parsed.summary(), original.summary());
    }

    #[test]
    fn default_category_not_serialized() {
        let changeset = Changeset::new(
            "Some change".to_string(),
            vec![PackageRelease::new("my-crate".to_string(), BumpType::Minor)],
            ChangeCategory::Changed,
        );

        let serialized = serialize_changeset(&changeset).expect("should serialize");
        assert!(
            !serialized.contains("category:"),
            "Default category should not be serialized"
        );
    }

    #[test]
    fn non_default_category_serialized() {
        let changeset = Changeset::new(
            "Security fix".to_string(),
            vec![PackageRelease::new("my-crate".to_string(), BumpType::Patch)],
            ChangeCategory::Security,
        );

        let serialized = serialize_changeset(&changeset).expect("should serialize");
        assert!(
            serialized.contains("category: security"),
            "Non-default category should be serialized"
        );
    }

    #[test]
    fn roundtrip_with_consumed_for_prerelease() {
        let original = Changeset::new(
            "Pre-release fix".to_string(),
            vec![PackageRelease::new("my-crate".to_string(), BumpType::Patch)],
            ChangeCategory::Fixed,
        )
        .with_consumed_for_prerelease(Some("1.0.1-alpha.1".to_string()));

        let serialized = serialize_changeset(&original).expect("should serialize");
        let parsed = parse_changeset(&serialized).expect("should parse");

        assert_eq!(
            parsed.consumed_for_prerelease(),
            Some(&"1.0.1-alpha.1".to_string())
        );
        assert_eq!(parsed.category(), ChangeCategory::Fixed);
        assert_eq!(parsed.summary(), original.summary());
    }

    #[test]
    fn consumed_for_prerelease_serialized_with_camel_case() {
        let changeset = Changeset::new(
            "Some change".to_string(),
            vec![PackageRelease::new("my-crate".to_string(), BumpType::Minor)],
            ChangeCategory::Changed,
        )
        .with_consumed_for_prerelease(Some("2.0.0-beta.3".to_string()));

        let serialized = serialize_changeset(&changeset).expect("should serialize");
        assert!(
            serialized.contains("consumedForPrerelease:"),
            "consumedForPrerelease should be serialized with camelCase, got: {serialized}"
        );
        assert!(
            serialized.contains("2.0.0-beta.3"),
            "version value should be present in serialized output, got: {serialized}"
        );
    }

    #[test]
    fn consumed_for_prerelease_none_not_serialized() {
        let changeset = Changeset::new(
            "Some change".to_string(),
            vec![PackageRelease::new("my-crate".to_string(), BumpType::Minor)],
            ChangeCategory::Changed,
        );

        let serialized = serialize_changeset(&changeset).expect("should serialize");
        assert!(
            !serialized.contains("consumedForPrerelease"),
            "None consumed_for_prerelease should not be serialized"
        );
    }

    #[test]
    fn graduate_false_not_serialized() {
        let changeset = Changeset::new(
            "Some change".to_string(),
            vec![PackageRelease::new("my-crate".to_string(), BumpType::Minor)],
            ChangeCategory::Changed,
        );

        let serialized = serialize_changeset(&changeset).expect("should serialize");
        assert!(
            !serialized.contains("graduate"),
            "graduate: false should not be serialized, got: {serialized}"
        );
    }

    #[test]
    fn graduate_true_serialized() {
        let changeset = Changeset::new(
            "Graduating to 1.0".to_string(),
            vec![PackageRelease::new("my-crate".to_string(), BumpType::Major)],
            ChangeCategory::Added,
        )
        .with_graduate(true);

        let serialized = serialize_changeset(&changeset).expect("should serialize");
        assert!(
            serialized.contains("graduate: true"),
            "graduate: true should be serialized, got: {serialized}"
        );
    }

    #[test]
    fn roundtrip_with_none_bump_type() {
        let original = Changeset::new(
            "Internal refactoring".to_string(),
            vec![PackageRelease::new("my-crate".to_string(), BumpType::None)],
            ChangeCategory::default(),
        );

        let serialized = serialize_changeset(&original).expect("should serialize");
        let parsed = parse_changeset(&serialized).expect("should parse");

        assert_eq!(parsed.releases()[0].bump_type(), BumpType::None);
        assert_eq!(parsed.summary(), original.summary());
    }

    #[test]
    fn roundtrip_with_graduate() {
        let original = Changeset::new(
            "Graduating to 1.0".to_string(),
            vec![PackageRelease::new("my-crate".to_string(), BumpType::Major)],
            ChangeCategory::Added,
        )
        .with_graduate(true);

        let serialized = serialize_changeset(&original).expect("should serialize");
        let parsed = parse_changeset(&serialized).expect("should parse");

        assert!(parsed.graduate());
        assert_eq!(parsed.category(), ChangeCategory::Added);
        assert_eq!(parsed.summary(), original.summary());
    }
}
