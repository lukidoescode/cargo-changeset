use indexmap::IndexMap;
use serde::Deserialize;
use serde_with::{MapPreventDuplicates, serde_as};

use changeset_core::{BumpType, ChangeCategory, Changeset, PackageRelease};

use crate::error::{FormatError, FrontMatterError, ValidationError};

pub(crate) const FRONT_MATTER_DELIMITER: &str = "---";

const MAX_INPUT_SIZE: usize = 100 * 1024 * 1024;

#[serde_as]
#[derive(Deserialize)]
struct FrontMatter {
    #[serde(default)]
    category: ChangeCategory,
    #[serde(default, rename = "consumedForPrerelease")]
    consumed_for_prerelease: Option<String>,
    #[serde(default)]
    graduate: bool,
    #[serde(flatten)]
    #[serde_as(as = "MapPreventDuplicates<_, _>")]
    releases: IndexMap<String, BumpType>,
}

fn strip_line_ending(s: &str) -> &str {
    s.strip_prefix("\r\n")
        .or_else(|| s.strip_prefix('\n'))
        .unwrap_or(s)
}

fn find_closing_delimiter(content: &str) -> Option<usize> {
    if content.starts_with(FRONT_MATTER_DELIMITER) {
        return Some(0);
    }
    if let Some(pos) = content.find("\r\n---") {
        return Some(pos + 2);
    }
    if let Some(pos) = content.find("\n---") {
        return Some(pos + 1);
    }
    None
}

fn extract_front_matter(content: &str) -> Result<(&str, &str), FormatError> {
    let trimmed = content.trim_start();

    if !trimmed.starts_with(FRONT_MATTER_DELIMITER) {
        return Err(FrontMatterError::MissingOpeningDelimiter.into());
    }

    let after_opening = &trimmed[FRONT_MATTER_DELIMITER.len()..];
    let after_opening = strip_line_ending(after_opening);

    let Some(closing_pos) = find_closing_delimiter(after_opening) else {
        return Err(FrontMatterError::MissingClosingDelimiter.into());
    };

    let yaml_content = &after_opening[..closing_pos];
    let yaml_content = yaml_content.trim_end_matches('\r');
    if yaml_content.trim().is_empty() {
        return Err(FrontMatterError::EmptyFrontMatter.into());
    }

    let after_closing = &after_opening[closing_pos + FRONT_MATTER_DELIMITER.len()..];
    let body = strip_line_ending(after_closing);

    Ok((yaml_content, body))
}

#[must_use = "parsing result should be handled"]
pub fn parse_changeset(content: &str) -> Result<Changeset, FormatError> {
    if content.len() > MAX_INPUT_SIZE {
        return Err(ValidationError::InputTooLarge {
            max_bytes: MAX_INPUT_SIZE,
        }
        .into());
    }

    let (yaml_content, body) = extract_front_matter(content)?;

    let parsed: FrontMatter = serde_yml::from_str(yaml_content)?;

    if parsed.releases.is_empty() {
        return Err(ValidationError::NoReleases.into());
    }

    let releases = parsed
        .releases
        .into_iter()
        .map(|(name, bump_type)| PackageRelease { name, bump_type })
        .collect();

    Ok(Changeset {
        summary: body.trim().to_string(),
        releases,
        category: parsed.category,
        consumed_for_prerelease: parsed.consumed_for_prerelease,
        graduate: parsed.graduate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_crate_with_summary() {
        let content = r#"---
"my-package": patch
---
Fix critical bug in authentication flow.
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert_eq!(changeset.releases.len(), 1);
        assert_eq!(changeset.releases[0].name, "my-package");
        assert_eq!(changeset.releases[0].bump_type, BumpType::Patch);
        assert_eq!(
            changeset.summary,
            "Fix critical bug in authentication flow."
        );
    }

    #[test]
    fn multiple_crates_preserves_order() {
        let content = r#"---
"crate-one": major
"crate-two": minor
"crate-three": patch
---
Breaking change to API.
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert_eq!(changeset.releases.len(), 3);

        assert_eq!(changeset.releases[0].name, "crate-one");
        assert_eq!(changeset.releases[0].bump_type, BumpType::Major);
        assert_eq!(changeset.releases[1].name, "crate-two");
        assert_eq!(changeset.releases[1].bump_type, BumpType::Minor);
        assert_eq!(changeset.releases[2].name, "crate-three");
        assert_eq!(changeset.releases[2].bump_type, BumpType::Patch);
    }

    #[test]
    fn multiline_summary() {
        let content = r#"---
"my-crate": minor
---
This is a multiline summary.

It contains multiple paragraphs and describes the change in detail.

- Feature one
- Feature two
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert!(changeset.summary.contains("multiline summary"));
        assert!(changeset.summary.contains("Feature one"));
        assert!(changeset.summary.contains("Feature two"));
    }

    #[test]
    fn empty_body() {
        let content = r#"---
"my-crate": patch
---
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert!(changeset.summary.is_empty());
    }

    #[test]
    fn delimiter_inside_summary() {
        let content = r#"---
"my-crate": patch
---
Summary with --- inside text should not break parsing.
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert!(changeset.summary.contains("---"));
    }

    #[test]
    fn windows_line_endings() {
        let content = "---\r\n\"my-crate\": patch\r\n---\r\nWindows style summary.\r\n";

        let changeset = parse_changeset(content).expect("should parse");
        assert_eq!(changeset.releases.len(), 1);
        assert_eq!(changeset.releases[0].name, "my-crate");
        assert!(changeset.summary.contains("Windows style summary"));
    }

    #[test]
    fn mixed_line_endings() {
        let content = "---\r\n\"crate-a\": major\n\"crate-b\": minor\r\n---\nMixed endings.\r\n";

        let changeset = parse_changeset(content).expect("should parse");
        assert_eq!(changeset.releases.len(), 2);
        assert_eq!(changeset.releases[0].name, "crate-a");
        assert_eq!(changeset.releases[1].name, "crate-b");
    }

    #[test]
    fn no_trailing_newline() {
        let content = "---\n\"my-crate\": patch\n---\nSummary without trailing newline";

        let changeset = parse_changeset(content).expect("should parse");
        assert_eq!(changeset.summary, "Summary without trailing newline");
    }

    #[test]
    fn unicode_crate_name_and_summary() {
        let content = r#"---
"über-crate": minor
---
Добавлена поддержка Unicode 🎉
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert_eq!(changeset.releases[0].name, "über-crate");
        assert!(changeset.summary.contains("Добавлена"));
        assert!(changeset.summary.contains("🎉"));
    }

    #[test]
    fn very_long_summary() {
        let long_summary = "A".repeat(10000);
        let content = format!("---\n\"my-crate\": patch\n---\n{long_summary}\n");

        let changeset = parse_changeset(&content).expect("should parse");
        assert_eq!(changeset.summary.len(), 10000);
    }

    #[test]
    fn whitespace_only_summary() {
        let content = "---\n\"my-crate\": patch\n---\n   \n\t\n   \n";

        let changeset = parse_changeset(content).expect("should parse");
        assert!(changeset.summary.is_empty());
    }

    #[test]
    fn error_missing_opening_delimiter() {
        let content = r#"
"my-crate": patch
---
Some summary.
"#;

        let err = parse_changeset(content).expect_err("should fail");
        assert!(err.to_string().contains("opening delimiter"));
    }

    #[test]
    fn error_missing_closing_delimiter() {
        let content = r#"---
"my-crate": patch
Some summary without closing delimiter.
"#;

        let err = parse_changeset(content).expect_err("should fail");
        assert!(err.to_string().contains("closing delimiter"));
    }

    #[test]
    fn error_empty_front_matter() {
        let content = r#"---
---
Some summary.
"#;

        let err = parse_changeset(content).expect_err("should fail");
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn error_invalid_bump_type() {
        let content = r#"---
"my-crate": invalid
---
Some summary.
"#;

        let err = parse_changeset(content).expect_err("should fail");
        assert!(err.to_string().contains("YAML"));
    }

    #[test]
    fn error_empty_releases() {
        let content = r#"---
{}
---
Some summary.
"#;

        let err = parse_changeset(content).expect_err("should fail");
        assert!(err.to_string().contains("at least one release"));
    }

    #[test]
    fn error_input_too_large() {
        let huge_content = "a".repeat(MAX_INPUT_SIZE + 1);

        let err = parse_changeset(&huge_content).expect_err("should fail");
        assert!(err.to_string().contains("maximum size"));
    }

    #[test]
    fn error_duplicate_package() {
        let content = r#"---
"my-crate": major
"my-crate": patch
---
Some summary.
"#;

        let err = parse_changeset(content).expect_err("should fail");
        let err_str = err.to_string();
        assert!(
            err_str.contains("duplicate"),
            "Expected 'duplicate' in error message, got: {err_str}"
        );
    }

    #[test]
    fn category_defaults_to_changed() {
        let content = r#"---
"my-crate": patch
---
Some summary.
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert_eq!(changeset.category, ChangeCategory::Changed);
    }

    #[test]
    fn parses_category_fixed() {
        let content = r#"---
category: fixed
"my-crate": patch
---
Fixed a bug.
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert_eq!(changeset.category, ChangeCategory::Fixed);
        assert_eq!(changeset.releases[0].name, "my-crate");
    }

    #[test]
    fn parses_category_added() {
        let content = r#"---
category: added
"my-feature": minor
---
Added new feature.
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert_eq!(changeset.category, ChangeCategory::Added);
    }

    #[test]
    fn parses_category_deprecated() {
        let content = r#"---
category: deprecated
"old-api": minor
---
Deprecated old API.
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert_eq!(changeset.category, ChangeCategory::Deprecated);
    }

    #[test]
    fn parses_category_removed() {
        let content = r#"---
category: removed
"old-feature": major
---
Removed old feature.
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert_eq!(changeset.category, ChangeCategory::Removed);
    }

    #[test]
    fn parses_category_security() {
        let content = r#"---
category: security
"auth-module": patch
---
Fixed security vulnerability.
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert_eq!(changeset.category, ChangeCategory::Security);
    }

    #[test]
    fn parses_category_changed() {
        let content = r#"---
category: changed
"my-crate": minor
---
Changed behavior.
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert_eq!(changeset.category, ChangeCategory::Changed);
    }

    #[test]
    fn error_invalid_category() {
        let content = r#"---
category: unknown
"my-crate": patch
---
Some summary.
"#;

        let err = parse_changeset(content).expect_err("should fail");
        assert!(err.to_string().contains("YAML"));
    }

    #[test]
    fn consumed_for_prerelease_defaults_to_none() {
        let content = r#"---
"my-crate": patch
---
Some summary.
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert_eq!(changeset.consumed_for_prerelease, None);
    }

    #[test]
    fn parses_consumed_for_prerelease() {
        let content = r#"---
consumedForPrerelease: 1.0.1-alpha.1
"my-crate": patch
---
Some summary.
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert_eq!(
            changeset.consumed_for_prerelease,
            Some("1.0.1-alpha.1".to_string())
        );
    }

    #[test]
    fn parses_consumed_for_prerelease_with_category() {
        let content = r#"---
category: fixed
consumedForPrerelease: 2.0.0-beta.3
"my-crate": patch
---
Fixed a bug.
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert_eq!(changeset.category, ChangeCategory::Fixed);
        assert_eq!(
            changeset.consumed_for_prerelease,
            Some("2.0.0-beta.3".to_string())
        );
    }

    #[test]
    fn parses_consumed_for_prerelease_with_quoted_value() {
        let content = r#"---
consumedForPrerelease: "1.2.3-rc.1"
"my-crate": minor
---
Release candidate.
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert_eq!(
            changeset.consumed_for_prerelease,
            Some("1.2.3-rc.1".to_string())
        );
    }

    #[test]
    fn graduate_defaults_to_false() {
        let content = r#"---
"my-crate": patch
---
Some summary.
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert!(!changeset.graduate);
    }

    #[test]
    fn parses_graduate_true() {
        let content = r#"---
graduate: true
"my-crate": major
---
Graduate to 1.0.0.
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert!(changeset.graduate);
    }

    #[test]
    fn parses_graduate_false() {
        let content = r#"---
graduate: false
"my-crate": major
---
Major bump.
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert!(!changeset.graduate);
    }

    #[test]
    fn parses_graduate_with_category() {
        let content = r#"---
category: added
graduate: true
"my-crate": major
---
New major release.
"#;

        let changeset = parse_changeset(content).expect("should parse");
        assert!(changeset.graduate);
        assert_eq!(changeset.category, ChangeCategory::Added);
    }
}
