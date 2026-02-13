use std::fs;
use std::path::Path;

use changeset_operations::providers::FileSystemChangesetIO;
use changeset_operations::traits::{ChangesetReader, ChangesetWriter};
use changeset_parse::parse_changeset;
use semver::Version;
use tempfile::TempDir;

fn create_changeset_dir() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    fs::create_dir_all(dir.path().join(".changeset")).expect("create .changeset dir");
    dir
}

fn write_changeset_file(dir: &TempDir, filename: &str, package: &str, bump: &str, summary: &str) {
    let content = format!(
        r#"---
"{package}": {bump}
---

{summary}
"#
    );
    fs::write(dir.path().join(".changeset").join(filename), content).expect("write changeset file");
}

fn write_consumed_changeset_file(
    dir: &TempDir,
    filename: &str,
    package: &str,
    bump: &str,
    summary: &str,
    consumed_version: &str,
) {
    let content = format!(
        r#"---
consumedForPrerelease: "{consumed_version}"
"{package}": {bump}
---

{summary}
"#
    );
    fs::write(dir.path().join(".changeset").join(filename), content)
        .expect("write consumed changeset file");
}

fn read_changeset_file(dir: &TempDir, filename: &str) -> String {
    fs::read_to_string(dir.path().join(".changeset").join(filename)).expect("read changeset file")
}

#[test]
fn mark_consumed_for_prerelease_updates_frontmatter() {
    let dir = create_changeset_dir();
    write_changeset_file(&dir, "feature.md", "my-crate", "minor", "Add a feature");

    let changeset_io = FileSystemChangesetIO::new(dir.path());
    let changeset_dir = Path::new(".changeset");
    let version = Version::parse("1.0.1-alpha.1").expect("parse version");

    let path = Path::new("feature.md");
    changeset_io
        .mark_consumed_for_prerelease(changeset_dir, &[path], &version)
        .expect("mark consumed should succeed");

    let content = read_changeset_file(&dir, "feature.md");
    let parsed = parse_changeset(&content).expect("parse changeset");

    assert_eq!(
        parsed.consumed_for_prerelease,
        Some("1.0.1-alpha.1".to_string()),
        "consumed_for_prerelease should be set to the version"
    );
    assert_eq!(
        parsed.releases.len(),
        1,
        "changeset should still have one release"
    );
    assert!(
        content.contains("Add a feature"),
        "summary should be preserved"
    );
}

#[test]
fn list_changesets_excludes_consumed() {
    let dir = create_changeset_dir();
    write_changeset_file(&dir, "unconsumed.md", "crate-a", "patch", "Fix bug");
    write_consumed_changeset_file(
        &dir,
        "consumed.md",
        "crate-b",
        "minor",
        "Add feature",
        "2.0.0-beta.1",
    );

    let changeset_io = FileSystemChangesetIO::new(dir.path());
    let changeset_dir = Path::new(".changeset");

    let changesets = changeset_io
        .list_changesets(changeset_dir)
        .expect("list changesets should succeed");

    assert_eq!(
        changesets.len(),
        1,
        "should return only one changeset (the unconsumed one)"
    );
    assert!(
        changesets[0].to_string_lossy().contains("unconsumed.md"),
        "should return the unconsumed changeset"
    );
}

#[test]
fn list_consumed_changesets_returns_only_consumed() {
    let dir = create_changeset_dir();
    write_changeset_file(&dir, "unconsumed.md", "crate-a", "patch", "Fix bug");
    write_consumed_changeset_file(
        &dir,
        "consumed.md",
        "crate-b",
        "minor",
        "Add feature",
        "2.0.0-beta.1",
    );

    let changeset_io = FileSystemChangesetIO::new(dir.path());
    let changeset_dir = Path::new(".changeset");

    let changesets = changeset_io
        .list_consumed_changesets(changeset_dir)
        .expect("list consumed changesets should succeed");

    assert_eq!(
        changesets.len(),
        1,
        "should return only one changeset (the consumed one)"
    );
    assert!(
        changesets[0].to_string_lossy().contains("consumed.md"),
        "should return the consumed changeset"
    );
}

#[test]
fn clear_consumed_for_prerelease_removes_flag() {
    let dir = create_changeset_dir();
    write_consumed_changeset_file(
        &dir,
        "consumed.md",
        "my-crate",
        "patch",
        "Fix a bug",
        "1.0.1-alpha.1",
    );

    let content_before = read_changeset_file(&dir, "consumed.md");
    let parsed_before = parse_changeset(&content_before).expect("parse before");
    assert!(
        parsed_before.consumed_for_prerelease.is_some(),
        "precondition: changeset should be consumed"
    );

    let changeset_io = FileSystemChangesetIO::new(dir.path());
    let changeset_dir = Path::new(".changeset");

    let path = Path::new("consumed.md");
    changeset_io
        .clear_consumed_for_prerelease(changeset_dir, &[path])
        .expect("clear consumed should succeed");

    let content_after = read_changeset_file(&dir, "consumed.md");
    let parsed_after = parse_changeset(&content_after).expect("parse after");

    assert!(
        parsed_after.consumed_for_prerelease.is_none(),
        "consumed_for_prerelease should be None after clearing"
    );
    assert_eq!(
        parsed_after.releases.len(),
        1,
        "changeset should still have one release"
    );
    assert!(
        content_after.contains("Fix a bug"),
        "summary should be preserved"
    );
}

#[test]
fn mark_consumed_preserves_category() {
    let dir = create_changeset_dir();
    let content = r#"---
category: fixed
"my-crate": patch
---

Fix a security issue.
"#;
    fs::write(dir.path().join(".changeset/security-fix.md"), content)
        .expect("write changeset file");

    let changeset_io = FileSystemChangesetIO::new(dir.path());
    let changeset_dir = Path::new(".changeset");
    let version = Version::parse("1.0.1-rc.1").expect("parse version");

    let path = Path::new("security-fix.md");
    changeset_io
        .mark_consumed_for_prerelease(changeset_dir, &[path], &version)
        .expect("mark consumed should succeed");

    let content_after = read_changeset_file(&dir, "security-fix.md");
    let parsed = parse_changeset(&content_after).expect("parse changeset");

    assert_eq!(
        parsed.consumed_for_prerelease,
        Some("1.0.1-rc.1".to_string()),
        "consumed_for_prerelease should be set"
    );
    assert_eq!(
        parsed.category,
        changeset_core::ChangeCategory::Fixed,
        "category should be preserved as 'fixed'"
    );
    assert!(
        content_after.contains("Fix a security issue."),
        "summary should be preserved"
    );
}

#[test]
fn mark_multiple_changesets_consumed() {
    let dir = create_changeset_dir();
    write_changeset_file(&dir, "fix1.md", "crate-a", "patch", "Fix bug 1");
    write_changeset_file(&dir, "fix2.md", "crate-b", "patch", "Fix bug 2");
    write_changeset_file(&dir, "feature.md", "crate-a", "minor", "Add feature");

    let changeset_io = FileSystemChangesetIO::new(dir.path());
    let changeset_dir = Path::new(".changeset");
    let version = Version::parse("1.0.0-alpha.1").expect("parse version");

    let paths: Vec<&Path> = vec![
        Path::new("fix1.md"),
        Path::new("fix2.md"),
        Path::new("feature.md"),
    ];
    changeset_io
        .mark_consumed_for_prerelease(changeset_dir, &paths, &version)
        .expect("mark consumed should succeed");

    for filename in ["fix1.md", "fix2.md", "feature.md"] {
        let content = read_changeset_file(&dir, filename);
        let parsed = parse_changeset(&content).expect("parse changeset");
        assert_eq!(
            parsed.consumed_for_prerelease,
            Some("1.0.0-alpha.1".to_string()),
            "{filename} should be marked as consumed"
        );
    }
}

#[test]
fn list_changesets_with_mixed_consumed_status() {
    let dir = create_changeset_dir();
    write_changeset_file(&dir, "pending1.md", "crate-a", "patch", "Fix 1");
    write_changeset_file(&dir, "pending2.md", "crate-b", "minor", "Feature 1");
    write_consumed_changeset_file(
        &dir,
        "consumed1.md",
        "crate-a",
        "patch",
        "Fix 2",
        "1.0.0-alpha.1",
    );
    write_consumed_changeset_file(
        &dir,
        "consumed2.md",
        "crate-b",
        "major",
        "Breaking",
        "2.0.0-beta.1",
    );

    let changeset_io = FileSystemChangesetIO::new(dir.path());
    let changeset_dir = Path::new(".changeset");

    let pending = changeset_io
        .list_changesets(changeset_dir)
        .expect("list changesets");
    let consumed = changeset_io
        .list_consumed_changesets(changeset_dir)
        .expect("list consumed");

    assert_eq!(pending.len(), 2, "should have 2 pending changesets");
    assert_eq!(consumed.len(), 2, "should have 2 consumed changesets");

    let pending_names: Vec<_> = pending
        .iter()
        .map(|p| {
            p.file_name()
                .expect("path should have file name")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(pending_names.contains(&"pending1.md".to_string()));
    assert!(pending_names.contains(&"pending2.md".to_string()));

    let consumed_names: Vec<_> = consumed
        .iter()
        .map(|p| {
            p.file_name()
                .expect("path should have file name")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(consumed_names.contains(&"consumed1.md".to_string()));
    assert!(consumed_names.contains(&"consumed2.md".to_string()));
}
