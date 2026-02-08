use indexmap::IndexMap;
use serde::Deserialize;
use serde_with::{MapPreventDuplicates, serde_as};

use changeset_core::{BumpType, Changeset, PackageRelease};

use crate::error::{FormatError, FrontMatterError, ValidationError};

pub(crate) const FRONT_MATTER_DELIMITER: &str = "---";

const MAX_INPUT_SIZE: usize = 100 * 1024 * 1024;

#[serde_as]
#[derive(Deserialize)]
struct ReleasesMap {
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

    let parsed: ReleasesMap = serde_yml::from_str(yaml_content)?;
    let releases_map = parsed.releases;

    if releases_map.is_empty() {
        return Err(ValidationError::NoReleases.into());
    }

    let releases = releases_map
        .into_iter()
        .map(|(name, bump_type)| PackageRelease { name, bump_type })
        .collect();

    Ok(Changeset {
        summary: body.trim().to_string(),
        releases,
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
}
