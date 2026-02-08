use indexmap::IndexMap;

use changeset_core::{BumpType, Changeset};

use crate::error::{FormatError, ValidationError};
use crate::parse::FRONT_MATTER_DELIMITER;

#[must_use = "serialization result should be handled"]
pub fn serialize_changeset(changeset: &Changeset) -> Result<String, FormatError> {
    if changeset.releases.is_empty() {
        return Err(ValidationError::NoReleases.into());
    }

    let releases_map: IndexMap<&str, BumpType> = changeset
        .releases
        .iter()
        .map(|r| (r.name.as_str(), r.bump_type))
        .collect();

    let yaml = serde_yml::to_string(&releases_map)?;

    let mut output = String::new();
    output.push_str(FRONT_MATTER_DELIMITER);
    output.push('\n');
    output.push_str(&yaml);
    output.push_str(FRONT_MATTER_DELIMITER);
    output.push('\n');

    if !changeset.summary.is_empty() {
        output.push_str(&changeset.summary);
        output.push('\n');
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use changeset_core::PackageRelease;

    use super::*;
    use crate::parse::parse_changeset;

    #[test]
    fn roundtrip() {
        let original = Changeset {
            summary: "Test summary".to_string(),
            releases: vec![
                PackageRelease {
                    name: "crate-a".to_string(),
                    bump_type: BumpType::Minor,
                },
                PackageRelease {
                    name: "crate-b".to_string(),
                    bump_type: BumpType::Patch,
                },
            ],
        };

        let serialized = serialize_changeset(&original).expect("should serialize");
        let parsed = parse_changeset(&serialized).expect("should parse");

        assert_eq!(parsed.summary, original.summary);
        assert_eq!(parsed.releases.len(), original.releases.len());

        for (original_release, parsed_release) in
            original.releases.iter().zip(parsed.releases.iter())
        {
            assert_eq!(parsed_release.name, original_release.name);
            assert_eq!(parsed_release.bump_type, original_release.bump_type);
        }
    }

    #[test]
    fn preserves_order() {
        let original = Changeset {
            summary: "Test".to_string(),
            releases: vec![
                PackageRelease {
                    name: "zebra".to_string(),
                    bump_type: BumpType::Major,
                },
                PackageRelease {
                    name: "apple".to_string(),
                    bump_type: BumpType::Minor,
                },
                PackageRelease {
                    name: "banana".to_string(),
                    bump_type: BumpType::Patch,
                },
            ],
        };

        let serialized = serialize_changeset(&original).expect("should serialize");
        let parsed = parse_changeset(&serialized).expect("should parse");

        assert_eq!(parsed.releases[0].name, "zebra");
        assert_eq!(parsed.releases[1].name, "apple");
        assert_eq!(parsed.releases[2].name, "banana");
    }

    #[test]
    fn error_empty_releases() {
        let changeset = Changeset {
            summary: "Some summary".to_string(),
            releases: vec![],
        };

        let err = serialize_changeset(&changeset).expect_err("should fail");
        assert!(err.to_string().contains("at least one release"));
    }
}
