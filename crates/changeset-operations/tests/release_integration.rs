use std::fs;
use std::path::Path;

use changeset_operations::OperationError;
use changeset_operations::operations::{ReleaseInput, ReleaseOperation, ReleaseOutcome};
use changeset_operations::providers::{
    FileSystemChangesetIO, FileSystemManifestWriter, FileSystemProjectProvider,
};
use tempfile::TempDir;

fn create_single_package_project() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[package]
name = "my-crate"
version = "1.0.0"
edition = "2021"
"#,
    )
    .expect("write Cargo.toml");

    fs::create_dir_all(dir.path().join("src")).expect("create src dir");
    fs::write(dir.path().join("src/lib.rs"), "").expect("write lib.rs");

    fs::create_dir_all(dir.path().join(".changeset")).expect("create .changeset dir");

    dir
}

fn create_workspace_project() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"
"#,
    )
    .expect("write workspace Cargo.toml");

    fs::create_dir_all(dir.path().join("crates/crate-a/src")).expect("create crate-a dir");
    fs::write(
        dir.path().join("crates/crate-a/Cargo.toml"),
        r#"[package]
name = "crate-a"
version = "1.0.0"
edition = "2021"
"#,
    )
    .expect("write crate-a Cargo.toml");
    fs::write(dir.path().join("crates/crate-a/src/lib.rs"), "").expect("write lib.rs");

    fs::create_dir_all(dir.path().join("crates/crate-b/src")).expect("create crate-b dir");
    fs::write(
        dir.path().join("crates/crate-b/Cargo.toml"),
        r#"[package]
name = "crate-b"
version = "2.0.0"
edition = "2021"
"#,
    )
    .expect("write crate-b Cargo.toml");
    fs::write(dir.path().join("crates/crate-b/src/lib.rs"), "").expect("write lib.rs");

    fs::create_dir_all(dir.path().join(".changeset")).expect("create .changeset dir");

    dir
}

fn create_workspace_with_inherited_versions() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.package]
version = "1.0.0"
edition = "2021"
"#,
    )
    .expect("write workspace Cargo.toml");

    fs::create_dir_all(dir.path().join("crates/crate-a/src")).expect("create crate-a dir");
    fs::write(
        dir.path().join("crates/crate-a/Cargo.toml"),
        r#"[package]
name = "crate-a"
version.workspace = true
edition.workspace = true
"#,
    )
    .expect("write crate-a Cargo.toml");
    fs::write(dir.path().join("crates/crate-a/src/lib.rs"), "").expect("write lib.rs");

    fs::create_dir_all(dir.path().join("crates/crate-b/src")).expect("create crate-b dir");
    fs::write(
        dir.path().join("crates/crate-b/Cargo.toml"),
        r#"[package]
name = "crate-b"
version.workspace = true
edition.workspace = true
"#,
    )
    .expect("write crate-b Cargo.toml");
    fs::write(dir.path().join("crates/crate-b/src/lib.rs"), "").expect("write lib.rs");

    fs::create_dir_all(dir.path().join(".changeset")).expect("create .changeset dir");

    dir
}

fn write_changeset(dir: &TempDir, filename: &str, package: &str, bump: &str, summary: &str) {
    let content = format!(
        r#"---
"{package}": {bump}
---

{summary}
"#
    );
    fs::write(dir.path().join(".changeset").join(filename), content).expect("write changeset");
}

fn read_version(path: &Path) -> String {
    let content = fs::read_to_string(path).expect("read file");
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("version") && line.contains('=') && !line.contains("workspace") {
            let version = line
                .split('=')
                .nth(1)
                .expect("version value")
                .trim()
                .trim_matches('"');
            return version.to_string();
        }
    }
    panic!("version not found in {}", path.display());
}

fn run_release(
    dir: &TempDir,
    dry_run: bool,
    convert_inherited: bool,
) -> Result<ReleaseOutcome, OperationError> {
    let project_provider = FileSystemProjectProvider::new();
    let changeset_reader = FileSystemChangesetIO::new(dir.path());
    let manifest_writer = FileSystemManifestWriter::new();

    let operation = ReleaseOperation::new(project_provider, changeset_reader, manifest_writer);
    let input = ReleaseInput {
        dry_run,
        convert_inherited,
    };

    operation.execute(dir.path(), &input)
}

#[test]
fn single_package_version_update() {
    let dir = create_single_package_project();
    write_changeset(&dir, "fix.md", "my-crate", "patch", "Fix a bug");

    let result = run_release(&dir, false, false).expect("release should succeed");

    let ReleaseOutcome::Executed(output) = result else {
        panic!("expected Executed outcome");
    };

    assert_eq!(output.planned_releases.len(), 1);
    assert_eq!(output.planned_releases[0].name, "my-crate");
    assert_eq!(output.planned_releases[0].new_version.to_string(), "1.0.1");

    let version = read_version(&dir.path().join("Cargo.toml"));
    assert_eq!(version, "1.0.1");
}

#[test]
fn workspace_with_multiple_packages() {
    let dir = create_workspace_project();
    write_changeset(&dir, "feature-a.md", "crate-a", "minor", "Add feature to A");
    write_changeset(
        &dir,
        "breaking-b.md",
        "crate-b",
        "major",
        "Breaking change in B",
    );

    let result = run_release(&dir, false, false).expect("release should succeed");

    let ReleaseOutcome::Executed(output) = result else {
        panic!("expected Executed outcome");
    };

    assert_eq!(output.planned_releases.len(), 2);

    let version_a = read_version(&dir.path().join("crates/crate-a/Cargo.toml"));
    assert_eq!(version_a, "1.1.0");

    let version_b = read_version(&dir.path().join("crates/crate-b/Cargo.toml"));
    assert_eq!(version_b, "3.0.0");
}

#[test]
fn inherited_version_requires_convert_flag() {
    let dir = create_workspace_with_inherited_versions();
    write_changeset(&dir, "fix.md", "crate-a", "patch", "Fix something");

    let result = run_release(&dir, false, false);

    assert!(matches!(
        result,
        Err(OperationError::InheritedVersionsRequireConvert { .. })
    ));
}

#[test]
fn inherited_version_conversion_with_convert_flag() {
    let dir = create_workspace_with_inherited_versions();
    write_changeset(&dir, "fix.md", "crate-a", "patch", "Fix something");

    let result = run_release(&dir, false, true).expect("release should succeed");

    let ReleaseOutcome::Executed(output) = result else {
        panic!("expected Executed outcome");
    };

    assert_eq!(output.planned_releases.len(), 1);
    assert_eq!(output.planned_releases[0].name, "crate-a");
    assert_eq!(output.planned_releases[0].new_version.to_string(), "1.0.1");

    let crate_a_content =
        fs::read_to_string(dir.path().join("crates/crate-a/Cargo.toml")).expect("read crate-a");
    assert!(
        crate_a_content.contains(r#"version = "1.0.1""#),
        "crate-a should have explicit version: {crate_a_content}"
    );
    assert!(
        !crate_a_content.contains("version.workspace"),
        "crate-a should not have inherited version"
    );

    let root_content =
        fs::read_to_string(dir.path().join("Cargo.toml")).expect("read root Cargo.toml");
    assert!(
        !root_content.contains("version ="),
        "workspace.package.version should be removed: {root_content}"
    );
}

#[test]
fn dry_run_skips_file_modifications() {
    let dir = create_single_package_project();
    write_changeset(&dir, "fix.md", "my-crate", "patch", "Fix a bug");

    let result = run_release(&dir, true, false).expect("release should succeed");

    let ReleaseOutcome::DryRun(output) = result else {
        panic!("expected DryRun outcome");
    };

    assert_eq!(output.planned_releases.len(), 1);
    assert_eq!(output.planned_releases[0].new_version.to_string(), "1.0.1");

    let version = read_version(&dir.path().join("Cargo.toml"));
    assert_eq!(
        version, "1.0.0",
        "version should not be modified in dry run"
    );
}

#[test]
fn format_preservation_comments_preserved() {
    let dir = TempDir::new().expect("create temp dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"# This is my crate configuration
[package]
name = "my-crate"
# The current version
version = "1.0.0"
# Edition matters
edition = "2021"
"#,
    )
    .expect("write Cargo.toml");

    fs::create_dir_all(dir.path().join("src")).expect("create src dir");
    fs::write(dir.path().join("src/lib.rs"), "").expect("write lib.rs");
    fs::create_dir_all(dir.path().join(".changeset")).expect("create .changeset dir");

    write_changeset(&dir, "fix.md", "my-crate", "minor", "Add feature");

    let _ = run_release(&dir, false, false).expect("release should succeed");

    let content = fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");

    assert!(
        content.contains("# This is my crate configuration"),
        "header comment should be preserved: {content}"
    );
    assert!(
        content.contains("# Edition matters"),
        "edition comment should be preserved: {content}"
    );
    assert!(
        content.contains(r#"version = "1.1.0""#),
        "version should be updated: {content}"
    );
}

#[test]
fn multiple_changesets_aggregate_correctly() {
    let dir = create_single_package_project();
    write_changeset(&dir, "fix1.md", "my-crate", "patch", "Fix bug 1");
    write_changeset(&dir, "fix2.md", "my-crate", "patch", "Fix bug 2");
    write_changeset(&dir, "feature.md", "my-crate", "minor", "Add feature");

    let result = run_release(&dir, false, false).expect("release should succeed");

    let ReleaseOutcome::Executed(output) = result else {
        panic!("expected Executed outcome");
    };

    assert_eq!(output.changesets_consumed.len(), 3);
    assert_eq!(
        output.planned_releases[0].new_version.to_string(),
        "1.1.0",
        "minor should win over patches"
    );
}
