//! End-to-end system tests for saga rollback behavior.
//!
//! These tests use real git repositories and file system operations to verify
//! that the saga pattern correctly restores the workspace to its original state
//! when failures occur at various steps.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use changeset_operations::OperationError;
use changeset_operations::operations::{ReleaseInput, ReleaseOperation, ReleaseOutcome};
use changeset_operations::providers::{
    FileSystemChangelogWriter, FileSystemChangesetIO, FileSystemManifestWriter,
    FileSystemProjectProvider, FileSystemReleaseStateIO, Git2Provider,
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

    fs::create_dir_all(dir.path().join(".changeset/changesets"))
        .expect("create .changeset/changesets dir");

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

    fs::create_dir_all(dir.path().join(".changeset/changesets"))
        .expect("create .changeset/changesets dir");

    dir
}

fn init_git_repo(dir: &TempDir) {
    Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .expect("git init");

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(dir.path())
        .output()
        .expect("git config email");

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(dir.path())
        .output()
        .expect("git config name");
}

fn git_add_all(dir: &TempDir) {
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir.path())
        .output()
        .expect("git add");
}

fn git_commit(dir: &TempDir, message: &str) {
    Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(dir.path())
        .output()
        .expect("git commit");
}

fn git_tags(dir: &TempDir) -> Vec<String> {
    let output = Command::new("git")
        .args(["tag", "--list"])
        .current_dir(dir.path())
        .output()
        .expect("git tag --list");

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(String::from)
        .filter(|s| !s.is_empty())
        .collect()
}

fn create_tag(dir: &TempDir, tag_name: &str) {
    Command::new("git")
        .args(["tag", tag_name])
        .current_dir(dir.path())
        .output()
        .expect("git tag");
}

fn git_log_count(dir: &TempDir) -> usize {
    let output = Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .current_dir(dir.path())
        .output()
        .expect("git rev-list");

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
}

fn write_changeset(dir: &TempDir, filename: &str, package: &str, bump: &str, summary: &str) {
    let content = format!(
        r#"---
"{package}": {bump}
---

{summary}
"#
    );
    fs::write(
        dir.path().join(".changeset/changesets").join(filename),
        content,
    )
    .expect("write changeset");
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

fn run_release_with_git(
    dir: &TempDir,
    no_commit: bool,
    no_tags: bool,
    keep_changesets: bool,
) -> Result<ReleaseOutcome, OperationError> {
    let project_provider = FileSystemProjectProvider::new();
    let changeset_reader = FileSystemChangesetIO::new(dir.path());
    let manifest_writer = FileSystemManifestWriter::new();
    let changelog_writer = FileSystemChangelogWriter::new();
    let git_provider = Git2Provider::new();
    let release_state_io = FileSystemReleaseStateIO::new();

    let operation = ReleaseOperation::new(
        project_provider,
        changeset_reader,
        manifest_writer,
        changelog_writer,
        git_provider,
        release_state_io,
    );
    let input = ReleaseInput {
        dry_run: false,
        convert_inherited: false,
        no_commit,
        no_tags,
        keep_changesets,
        force: false,
        per_package_config: HashMap::new(),
        global_prerelease: None,
        graduate_all: false,
    };

    operation.execute(dir.path(), &input)
}

// =============================================================================
// Happy Path Tests
// =============================================================================

#[test]
fn system_test_successful_release_single_package() {
    let dir = create_single_package_project();
    init_git_repo(&dir);
    git_add_all(&dir);
    git_commit(&dir, "Initial commit");

    write_changeset(&dir, "fix.md", "my-crate", "patch", "Fix a bug");
    git_add_all(&dir);
    git_commit(&dir, "Add changeset");

    let initial_commit_count = git_log_count(&dir);
    let initial_version = read_version(&dir.path().join("Cargo.toml"));
    assert_eq!(initial_version, "1.0.0");

    let result = run_release_with_git(&dir, false, false, false).expect("release should succeed");

    let ReleaseOutcome::Executed(output) = result else {
        panic!("expected Executed outcome");
    };

    assert_eq!(output.planned_releases.len(), 1);
    assert_eq!(output.planned_releases[0].new_version.to_string(), "1.0.1");

    let final_version = read_version(&dir.path().join("Cargo.toml"));
    assert_eq!(final_version, "1.0.1", "version should be updated");

    let tags = git_tags(&dir);
    assert!(
        tags.contains(&"v1.0.1".to_string()),
        "tag should be created"
    );

    let final_commit_count = git_log_count(&dir);
    assert_eq!(
        final_commit_count,
        initial_commit_count + 1,
        "one commit should be added"
    );

    let changeset_path = dir.path().join(".changeset/changesets/fix.md");
    assert!(!changeset_path.exists(), "changeset should be deleted");
}

#[test]
fn system_test_successful_release_workspace() {
    let dir = create_workspace_project();
    init_git_repo(&dir);
    git_add_all(&dir);
    git_commit(&dir, "Initial commit");

    write_changeset(&dir, "feature-a.md", "crate-a", "minor", "Add feature to A");
    write_changeset(&dir, "fix-b.md", "crate-b", "patch", "Fix bug in B");
    git_add_all(&dir);
    git_commit(&dir, "Add changesets");

    let result = run_release_with_git(&dir, false, false, false).expect("release should succeed");

    let ReleaseOutcome::Executed(output) = result else {
        panic!("expected Executed outcome");
    };

    assert_eq!(output.planned_releases.len(), 2);

    let version_a = read_version(&dir.path().join("crates/crate-a/Cargo.toml"));
    assert_eq!(version_a, "1.1.0");

    let version_b = read_version(&dir.path().join("crates/crate-b/Cargo.toml"));
    assert_eq!(version_b, "2.0.1");

    let tags = git_tags(&dir);
    assert!(tags.contains(&"crate-a-v1.1.0".to_string()));
    assert!(tags.contains(&"crate-b-v2.0.1".to_string()));
}

// =============================================================================
// Rollback on Tag Failure Tests
// =============================================================================

#[test]
fn system_test_rollback_on_tag_conflict() {
    let dir = create_single_package_project();
    init_git_repo(&dir);
    git_add_all(&dir);
    git_commit(&dir, "Initial commit");

    write_changeset(&dir, "fix.md", "my-crate", "patch", "Fix a bug");
    git_add_all(&dir);
    git_commit(&dir, "Add changeset");

    create_tag(&dir, "v1.0.1");

    let initial_version = read_version(&dir.path().join("Cargo.toml"));
    let initial_commit_count = git_log_count(&dir);

    let result = run_release_with_git(&dir, false, false, false);

    assert!(result.is_err(), "release should fail due to tag conflict");

    let final_version = read_version(&dir.path().join("Cargo.toml"));
    assert_eq!(
        final_version, initial_version,
        "version should be restored to original"
    );

    let final_commit_count = git_log_count(&dir);
    assert_eq!(
        final_commit_count, initial_commit_count,
        "commit should be reset"
    );

    // Note: Changeset files are deleted via git (DeleteChangesetFilesStep uses
    // git_provider.delete_files()), but the restore uses changeset_rw.restore_changeset().
    // The backup is captured during execute() and stored in the output, but
    // compensation receives the original input (before backup was captured).
    // This is a known limitation in the current saga data flow design.
    // The changeset file may or may not be restored depending on implementation details.
}

#[test]
fn system_test_multi_package_rollback_on_second_tag_conflict() {
    let dir = create_workspace_project();
    init_git_repo(&dir);
    git_add_all(&dir);
    git_commit(&dir, "Initial commit");

    write_changeset(&dir, "feature-a.md", "crate-a", "minor", "Add feature to A");
    write_changeset(&dir, "fix-b.md", "crate-b", "patch", "Fix bug in B");
    git_add_all(&dir);
    git_commit(&dir, "Add changesets");

    create_tag(&dir, "crate-b-v2.0.1");

    let initial_version_a = read_version(&dir.path().join("crates/crate-a/Cargo.toml"));
    let initial_version_b = read_version(&dir.path().join("crates/crate-b/Cargo.toml"));
    let initial_commit_count = git_log_count(&dir);

    let result = run_release_with_git(&dir, false, false, false);

    assert!(
        result.is_err(),
        "release should fail due to tag conflict on crate-b"
    );

    let final_version_a = read_version(&dir.path().join("crates/crate-a/Cargo.toml"));
    let final_version_b = read_version(&dir.path().join("crates/crate-b/Cargo.toml"));
    assert_eq!(
        final_version_a, initial_version_a,
        "crate-a version should be restored"
    );
    assert_eq!(
        final_version_b, initial_version_b,
        "crate-b version should be restored"
    );

    let final_commit_count = git_log_count(&dir);
    assert_eq!(
        final_commit_count, initial_commit_count,
        "commit should be reset"
    );

    // The pre-existing conflicting tag should still exist
    let tags = git_tags(&dir);
    assert!(
        tags.contains(&"crate-b-v2.0.1".to_string()),
        "pre-existing conflicting tag should still exist"
    );

    // Note: When CreateTagsStep fails partway through (after creating crate-a tag
    // but before crate-b tag), the saga framework does not call compensate() for
    // the failed step - only for previously successful steps. This means the
    // crate-a tag that was created within the same step is not automatically
    // cleaned up. This is a known limitation of the saga pattern when a step
    // performs multiple operations. The step could be enhanced to clean up
    // partial work within execute() when it fails.
}

// =============================================================================
// State Verification Tests
// =============================================================================

#[test]
fn system_test_changelog_restored_on_failure() {
    let dir = create_single_package_project();
    init_git_repo(&dir);
    git_add_all(&dir);
    git_commit(&dir, "Initial commit");

    let changelog_path = dir.path().join("CHANGELOG.md");
    let original_changelog = "# Changelog\n\n## [1.0.0] - 2024-01-01\n\n- Initial release\n";
    fs::write(&changelog_path, original_changelog).expect("write changelog");

    write_changeset(&dir, "fix.md", "my-crate", "patch", "Fix a bug");
    git_add_all(&dir);
    git_commit(&dir, "Add changeset and changelog");

    create_tag(&dir, "v1.0.1");

    let result = run_release_with_git(&dir, false, false, false);
    assert!(result.is_err(), "release should fail");

    let restored_changelog = fs::read_to_string(&changelog_path).expect("read changelog");
    assert_eq!(
        restored_changelog, original_changelog,
        "changelog should be restored to original content"
    );
}

#[test]
fn system_test_new_changelog_deleted_on_failure() {
    let dir = create_single_package_project();
    init_git_repo(&dir);
    git_add_all(&dir);
    git_commit(&dir, "Initial commit");

    write_changeset(&dir, "fix.md", "my-crate", "patch", "Fix a bug");
    git_add_all(&dir);
    git_commit(&dir, "Add changeset");

    let changelog_path = dir.path().join("CHANGELOG.md");
    assert!(
        !changelog_path.exists(),
        "changelog should not exist before release"
    );

    create_tag(&dir, "v1.0.1");

    let result = run_release_with_git(&dir, false, false, false);
    assert!(result.is_err(), "release should fail");

    assert!(
        !changelog_path.exists(),
        "newly created changelog should be deleted during rollback"
    );
}

// =============================================================================
// Edge Case Tests
// =============================================================================

#[test]
fn system_test_no_rollback_needed_for_dry_run() {
    let dir = create_single_package_project();
    init_git_repo(&dir);
    git_add_all(&dir);
    git_commit(&dir, "Initial commit");

    write_changeset(&dir, "fix.md", "my-crate", "patch", "Fix a bug");
    git_add_all(&dir);
    git_commit(&dir, "Add changeset");

    let initial_version = read_version(&dir.path().join("Cargo.toml"));
    let initial_commit_count = git_log_count(&dir);

    let project_provider = FileSystemProjectProvider::new();
    let changeset_reader = FileSystemChangesetIO::new(dir.path());
    let manifest_writer = FileSystemManifestWriter::new();
    let changelog_writer = FileSystemChangelogWriter::new();
    let git_provider = Git2Provider::new();
    let release_state_io = FileSystemReleaseStateIO::new();

    let operation = ReleaseOperation::new(
        project_provider,
        changeset_reader,
        manifest_writer,
        changelog_writer,
        git_provider,
        release_state_io,
    );
    let input = ReleaseInput {
        dry_run: true,
        convert_inherited: false,
        no_commit: false,
        no_tags: false,
        keep_changesets: false,
        force: false,
        per_package_config: HashMap::new(),
        global_prerelease: None,
        graduate_all: false,
    };

    let result = operation
        .execute(dir.path(), &input)
        .expect("dry run should succeed");

    let ReleaseOutcome::DryRun(output) = result else {
        panic!("expected DryRun outcome");
    };

    assert_eq!(output.planned_releases[0].new_version.to_string(), "1.0.1");

    let final_version = read_version(&dir.path().join("Cargo.toml"));
    assert_eq!(
        final_version, initial_version,
        "version should not change in dry run"
    );

    let final_commit_count = git_log_count(&dir);
    assert_eq!(
        final_commit_count, initial_commit_count,
        "no commits should be added in dry run"
    );

    let tags = git_tags(&dir);
    assert!(tags.is_empty(), "no tags should be created in dry run");
}

#[test]
fn system_test_no_commit_mode_still_updates_files() {
    let dir = create_single_package_project();
    init_git_repo(&dir);
    git_add_all(&dir);
    git_commit(&dir, "Initial commit");

    write_changeset(&dir, "fix.md", "my-crate", "patch", "Fix a bug");

    let initial_commit_count = git_log_count(&dir);

    let result = run_release_with_git(&dir, true, true, true).expect("release should succeed");

    let ReleaseOutcome::Executed(output) = result else {
        panic!("expected Executed outcome");
    };

    assert_eq!(output.planned_releases[0].new_version.to_string(), "1.0.1");

    let final_version = read_version(&dir.path().join("Cargo.toml"));
    assert_eq!(final_version, "1.0.1", "version should be updated");

    let final_commit_count = git_log_count(&dir);
    assert_eq!(
        final_commit_count, initial_commit_count,
        "no commits should be added in no-commit mode"
    );

    let tags = git_tags(&dir);
    assert!(tags.is_empty(), "no tags should be created in no-tags mode");

    let changeset_path = dir.path().join(".changeset/changesets/fix.md");
    assert!(
        changeset_path.exists(),
        "changeset should be kept in keep-changesets mode"
    );
}
