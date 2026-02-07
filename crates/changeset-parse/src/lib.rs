use changeset_core::{Changeset, ChangesetError, Result};

pub fn parse_changeset(content: &str) -> Result<Changeset> {
    serde_json::from_str(content)
        .map_err(|e| ChangesetError::Parse(format!("Failed to parse changeset: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use changeset_core::BumpType;

    #[test]
    fn test_parse_basic_changeset() {
        let json = r#"{
            "summary": "Fix critical bug",
            "releases": [
                {"name": "my-package", "bump_type": "patch"}
            ]
        }"#;

        let changeset = parse_changeset(json).unwrap();
        assert_eq!(changeset.summary, "Fix critical bug");
        assert_eq!(changeset.releases.len(), 1);
        assert_eq!(changeset.releases[0].name, "my-package");
        assert_eq!(changeset.releases[0].bump_type, BumpType::Patch);
    }
}
