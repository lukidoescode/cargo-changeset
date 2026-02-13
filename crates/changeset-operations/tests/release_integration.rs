use std::fs;
use std::path::Path;
use std::process::Command;

use changeset_operations::OperationError;
use changeset_operations::operations::{
    ReleaseInput, ReleaseOperation, ReleaseOutcome, StatusOperation,
};
use changeset_operations::providers::{
    FileSystemChangelogWriter, FileSystemChangesetIO, FileSystemManifestWriter,
    FileSystemProjectProvider, Git2Provider,
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
    let changeset_io = FileSystemChangesetIO::new(dir.path());
    let manifest_writer = FileSystemManifestWriter::new();
    let changelog_writer = FileSystemChangelogWriter::new();
    let git_provider = Git2Provider::new();

    let operation = ReleaseOperation::new(
        project_provider,
        changeset_io,
        manifest_writer,
        changelog_writer,
        git_provider,
    );
    let input = ReleaseInput {
        dry_run,
        convert_inherited,
        no_commit: true,
        no_tags: true,
        keep_changesets: true,
        prerelease: None,
        force: false,
        graduate: false,
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

#[test]
fn creates_changelog_on_release() {
    let dir = create_single_package_project();
    write_changeset(&dir, "fix.md", "my-crate", "patch", "Fix a bug");

    let result = run_release(&dir, false, false).expect("release should succeed");

    let ReleaseOutcome::Executed(output) = result else {
        panic!("expected Executed outcome");
    };

    assert_eq!(output.changelog_updates.len(), 1);
    assert!(output.changelog_updates[0].created);

    let changelog_path = dir.path().join("CHANGELOG.md");
    assert!(changelog_path.exists(), "CHANGELOG.md should be created");

    let content = fs::read_to_string(&changelog_path).expect("read CHANGELOG.md");
    assert!(content.contains("# Changelog"));
    assert!(content.contains("## [1.0.1]"));
    assert!(content.contains("Fix a bug"));
}

#[test]
fn dry_run_skips_changelog_creation() {
    let dir = create_single_package_project();
    write_changeset(&dir, "fix.md", "my-crate", "patch", "Fix a bug");

    let result = run_release(&dir, true, false).expect("release should succeed");

    let ReleaseOutcome::DryRun(output) = result else {
        panic!("expected DryRun outcome");
    };

    assert!(
        output.changelog_updates.is_empty(),
        "dry run should not create changelog updates"
    );

    let changelog_path = dir.path().join("CHANGELOG.md");
    assert!(
        !changelog_path.exists(),
        "CHANGELOG.md should not be created in dry run"
    );
}

#[test]
fn changelog_aggregates_multiple_changesets() {
    let dir = create_single_package_project();
    write_changeset(&dir, "fix.md", "my-crate", "patch", "Fix a bug");
    write_changeset(&dir, "feature.md", "my-crate", "minor", "Add new feature");

    let result = run_release(&dir, false, false).expect("release should succeed");

    let ReleaseOutcome::Executed(_) = result else {
        panic!("expected Executed outcome");
    };

    let changelog_path = dir.path().join("CHANGELOG.md");
    let content = fs::read_to_string(&changelog_path).expect("read CHANGELOG.md");

    assert!(content.contains("Fix a bug"), "Should contain first change");
    assert!(
        content.contains("Add new feature"),
        "Should contain second change"
    );
    assert!(content.contains("## [1.1.0]"), "Version should be 1.1.0");
}

#[test]
fn status_and_release_calculate_identical_versions() {
    let dir = create_workspace_project();
    write_changeset(&dir, "fix-a.md", "crate-a", "patch", "Fix bug in A");
    write_changeset(&dir, "feature-a.md", "crate-a", "minor", "Add feature to A");
    write_changeset(
        &dir,
        "breaking-b.md",
        "crate-b",
        "major",
        "Breaking change in B",
    );

    let project_provider = FileSystemProjectProvider::new();
    let changeset_reader = FileSystemChangesetIO::new(dir.path());
    let inherited_checker = FileSystemManifestWriter::new();

    let status_operation =
        StatusOperation::new(project_provider, changeset_reader, inherited_checker);
    let status_output = status_operation
        .execute(dir.path())
        .expect("status should succeed");

    let release_result = run_release(&dir, true, false).expect("release should succeed");
    let ReleaseOutcome::DryRun(release_output) = release_result else {
        panic!("expected DryRun outcome");
    };

    assert_eq!(
        status_output.projected_releases.len(),
        release_output.planned_releases.len(),
        "status and release should have same number of releases"
    );

    for status_release in &status_output.projected_releases {
        let matching_release = release_output
            .planned_releases
            .iter()
            .find(|r| r.name == status_release.name)
            .expect("release should have matching package");

        assert_eq!(
            status_release.current_version, matching_release.current_version,
            "current versions should match for {}",
            status_release.name
        );
        assert_eq!(
            status_release.new_version, matching_release.new_version,
            "new versions should match for {}",
            status_release.name
        );
        assert_eq!(
            status_release.bump_type, matching_release.bump_type,
            "bump types should match for {}",
            status_release.name
        );
    }
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

fn git_log_oneline(dir: &TempDir, count: usize) -> Vec<String> {
    let output = Command::new("git")
        .args(["log", "--oneline", "-n", &count.to_string()])
        .current_dir(dir.path())
        .output()
        .expect("git log");

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(String::from)
        .filter(|s| !s.is_empty())
        .collect()
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

    let operation = ReleaseOperation::new(
        project_provider,
        changeset_reader,
        manifest_writer,
        changelog_writer,
        git_provider,
    );
    let input = ReleaseInput {
        dry_run: false,
        convert_inherited: false,
        no_commit,
        no_tags,
        keep_changesets,
        prerelease: None,
        force: false,
        graduate: false,
    };

    operation.execute(dir.path(), &input)
}

#[test]
fn release_creates_commit_and_tags() {
    let dir = create_single_package_project();
    init_git_repo(&dir);
    git_add_all(&dir);
    git_commit(&dir, "Initial commit");

    write_changeset(&dir, "fix.md", "my-crate", "patch", "Fix a bug");
    git_add_all(&dir);
    git_commit(&dir, "Add changeset");

    let result = run_release_with_git(&dir, false, false, false).expect("release should succeed");

    let ReleaseOutcome::Executed(output) = result else {
        panic!("expected Executed outcome");
    };

    assert!(
        output.git_result.is_some(),
        "git operations should have been performed"
    );
    let git_result = output.git_result.as_ref().expect("git_result");

    assert!(
        git_result.commit.is_some(),
        "a commit should have been created"
    );

    assert_eq!(
        git_result.tags_created.len(),
        1,
        "one tag should have been created"
    );
    assert_eq!(git_result.tags_created[0].name, "v1.0.1");

    let tags = git_tags(&dir);
    assert!(
        tags.contains(&"v1.0.1".to_string()),
        "tag should exist in git"
    );

    let changeset_path = dir.path().join(".changeset/fix.md");
    assert!(
        !changeset_path.exists(),
        "changeset file should have been deleted"
    );

    assert_eq!(
        git_result.changesets_deleted.len(),
        1,
        "one changeset should have been deleted"
    );
}

#[test]
fn release_workspace_creates_prefixed_tags() {
    let dir = create_workspace_project();
    init_git_repo(&dir);
    git_add_all(&dir);
    git_commit(&dir, "Initial commit");

    write_changeset(&dir, "feature.md", "crate-a", "minor", "Add feature");
    write_changeset(&dir, "fix.md", "crate-b", "patch", "Fix bug");
    git_add_all(&dir);
    git_commit(&dir, "Add changesets");

    let result = run_release_with_git(&dir, false, false, false).expect("release should succeed");

    let ReleaseOutcome::Executed(output) = result else {
        panic!("expected Executed outcome");
    };

    let git_result = output.git_result.as_ref().expect("git result");
    assert_eq!(git_result.tags_created.len(), 2);

    let tag_names: Vec<_> = git_result.tags_created.iter().map(|t| &t.name).collect();
    assert!(
        tag_names.contains(&&"crate-a-v1.1.0".to_string()),
        "should have crate-a tag"
    );
    assert!(
        tag_names.contains(&&"crate-b-v2.0.1".to_string()),
        "should have crate-b tag"
    );

    let tags = git_tags(&dir);
    assert!(tags.contains(&"crate-a-v1.1.0".to_string()));
    assert!(tags.contains(&"crate-b-v2.0.1".to_string()));
}

#[test]
fn release_no_commit_skips_git_operations() {
    let dir = create_single_package_project();
    init_git_repo(&dir);
    git_add_all(&dir);
    git_commit(&dir, "Initial commit");

    write_changeset(&dir, "fix.md", "my-crate", "patch", "Fix a bug");

    let result = run_release_with_git(&dir, true, false, true).expect("release should succeed");

    let ReleaseOutcome::Executed(output) = result else {
        panic!("expected Executed outcome");
    };

    let git_result = output
        .git_result
        .as_ref()
        .expect("git_result is always present");
    assert!(
        git_result.commit.is_none(),
        "no commit should be created when --no-commit"
    );
    assert!(
        git_result.tags_created.is_empty(),
        "no tags should be created when --no-commit"
    );
    assert!(
        git_result.changesets_deleted.is_empty(),
        "no changesets should be deleted when --keep-changesets"
    );

    let version = read_version(&dir.path().join("Cargo.toml"));
    assert_eq!(version, "1.0.1", "version should still be updated");

    let tags = git_tags(&dir);
    assert!(tags.is_empty(), "no tags should have been created");

    let changeset_path = dir.path().join(".changeset/fix.md");
    assert!(
        changeset_path.exists(),
        "changeset should be kept with --keep-changesets"
    );
}

#[test]
fn release_no_tags_creates_commit_without_tags() {
    let dir = create_single_package_project();
    init_git_repo(&dir);
    git_add_all(&dir);
    git_commit(&dir, "Initial commit");

    write_changeset(&dir, "fix.md", "my-crate", "patch", "Fix a bug");
    git_add_all(&dir);
    git_commit(&dir, "Add changeset");

    let result = run_release_with_git(&dir, false, true, false).expect("release should succeed");

    let ReleaseOutcome::Executed(output) = result else {
        panic!("expected Executed outcome");
    };

    let git_result = output.git_result.as_ref().expect("git result");
    assert!(git_result.commit.is_some(), "commit should be created");
    assert!(git_result.tags_created.is_empty(), "no tags when --no-tags");

    let tags = git_tags(&dir);
    assert!(tags.is_empty(), "no tags should exist in git");

    let logs = git_log_oneline(&dir, 1);
    assert!(
        logs[0].contains("v1.0.1"),
        "commit message should contain version"
    );
}

#[test]
fn release_keep_changesets_preserves_files() {
    let dir = create_single_package_project();
    init_git_repo(&dir);
    git_add_all(&dir);
    git_commit(&dir, "Initial commit");

    write_changeset(&dir, "fix.md", "my-crate", "patch", "Fix a bug");
    git_add_all(&dir);
    git_commit(&dir, "Add changeset");

    let result = run_release_with_git(&dir, false, false, true).expect("release should succeed");

    let ReleaseOutcome::Executed(output) = result else {
        panic!("expected Executed outcome");
    };

    let git_result = output.git_result.as_ref().expect("git result");
    assert!(
        git_result.changesets_deleted.is_empty(),
        "no changesets should be deleted"
    );

    let changeset_path = dir.path().join(".changeset/fix.md");
    assert!(
        changeset_path.exists(),
        "changeset should still exist with --keep-changesets"
    );
}

#[test]
fn release_errors_on_dirty_working_tree() {
    let dir = create_single_package_project();
    init_git_repo(&dir);
    git_add_all(&dir);
    git_commit(&dir, "Initial commit");

    write_changeset(&dir, "fix.md", "my-crate", "patch", "Fix a bug");

    let result = run_release_with_git(&dir, false, false, false);

    assert!(
        matches!(result, Err(OperationError::DirtyWorkingTree)),
        "should error on dirty working tree: {result:?}"
    );
}

fn create_mixed_prerelease_workspace() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"
"#,
    )
    .expect("write workspace Cargo.toml");

    fs::create_dir_all(dir.path().join("crates/prerelease-crate/src"))
        .expect("create prerelease-crate dir");
    fs::write(
        dir.path().join("crates/prerelease-crate/Cargo.toml"),
        r#"[package]
name = "prerelease-crate"
version = "1.0.0-alpha.1"
edition = "2021"
"#,
    )
    .expect("write prerelease-crate Cargo.toml");
    fs::write(dir.path().join("crates/prerelease-crate/src/lib.rs"), "").expect("write lib.rs");

    fs::create_dir_all(dir.path().join("crates/stable-crate/src"))
        .expect("create stable-crate dir");
    fs::write(
        dir.path().join("crates/stable-crate/Cargo.toml"),
        r#"[package]
name = "stable-crate"
version = "2.0.0"
edition = "2021"
"#,
    )
    .expect("write stable-crate Cargo.toml");
    fs::write(dir.path().join("crates/stable-crate/src/lib.rs"), "").expect("write lib.rs");

    fs::create_dir_all(dir.path().join(".changeset")).expect("create .changeset dir");

    dir
}

fn run_release_with_prerelease(
    dir: &TempDir,
    prerelease: Option<changeset_core::PrereleaseSpec>,
) -> Result<ReleaseOutcome, OperationError> {
    let project_provider = FileSystemProjectProvider::new();
    let changeset_io = FileSystemChangesetIO::new(dir.path());
    let manifest_writer = FileSystemManifestWriter::new();
    let changelog_writer = FileSystemChangelogWriter::new();
    let git_provider = Git2Provider::new();

    let operation = ReleaseOperation::new(
        project_provider,
        changeset_io,
        manifest_writer,
        changelog_writer,
        git_provider,
    );
    let input = ReleaseInput {
        dry_run: false,
        convert_inherited: false,
        no_commit: true,
        no_tags: true,
        keep_changesets: true,
        prerelease,
        force: false,
        graduate: false,
    };

    operation.execute(dir.path(), &input)
}

fn write_consumed_changeset(
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
    fs::write(dir.path().join(".changeset").join(filename), content).expect("write changeset");
}

#[test]
fn workspace_with_mixed_prerelease_and_stable_packages_graduates_prereleases_only() {
    let dir = create_mixed_prerelease_workspace();
    write_changeset(
        &dir,
        "fix-pre.md",
        "prerelease-crate",
        "patch",
        "Fix bug in prerelease",
    );
    write_changeset(
        &dir,
        "fix-stable.md",
        "stable-crate",
        "patch",
        "Fix bug in stable",
    );

    let result = run_release_with_prerelease(&dir, None).expect("release should succeed");

    let ReleaseOutcome::Executed(output) = result else {
        panic!("expected Executed outcome");
    };

    assert_eq!(
        output.planned_releases.len(),
        1,
        "only prerelease packages should graduate when any prerelease exists"
    );

    let prerelease_pkg = output
        .planned_releases
        .iter()
        .find(|r| r.name == "prerelease-crate")
        .expect("prerelease-crate should be in releases");
    assert_eq!(
        prerelease_pkg.new_version.to_string(),
        "1.0.0",
        "prerelease should graduate to stable when no --prerelease flag"
    );

    assert!(
        output
            .unchanged_packages
            .contains(&"stable-crate".to_string()),
        "stable packages are skipped during graduation; run a second release for them"
    );
}

#[test]
fn prerelease_with_multiple_prerelease_packages() {
    let dir = TempDir::new().expect("create temp dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"
"#,
    )
    .expect("write workspace Cargo.toml");

    fs::create_dir_all(dir.path().join("crates/crate-alpha/src")).expect("create crate-alpha dir");
    fs::write(
        dir.path().join("crates/crate-alpha/Cargo.toml"),
        r#"[package]
name = "crate-alpha"
version = "1.0.0-alpha.1"
edition = "2021"
"#,
    )
    .expect("write crate-alpha Cargo.toml");
    fs::write(dir.path().join("crates/crate-alpha/src/lib.rs"), "").expect("write lib.rs");

    fs::create_dir_all(dir.path().join("crates/crate-beta/src")).expect("create crate-beta dir");
    fs::write(
        dir.path().join("crates/crate-beta/Cargo.toml"),
        r#"[package]
name = "crate-beta"
version = "2.0.0-beta.1"
edition = "2021"
"#,
    )
    .expect("write crate-beta Cargo.toml");
    fs::write(dir.path().join("crates/crate-beta/src/lib.rs"), "").expect("write lib.rs");

    fs::create_dir_all(dir.path().join(".changeset")).expect("create .changeset dir");

    write_changeset(&dir, "fix-alpha.md", "crate-alpha", "patch", "Fix in alpha");
    write_changeset(
        &dir,
        "feature-beta.md",
        "crate-beta",
        "minor",
        "Feature for beta",
    );

    let result = run_release_with_prerelease(&dir, Some(changeset_core::PrereleaseSpec::Beta))
        .expect("release should succeed");

    let ReleaseOutcome::Executed(output) = result else {
        panic!("expected Executed outcome");
    };

    assert_eq!(output.planned_releases.len(), 2);

    let alpha_pkg = output
        .planned_releases
        .iter()
        .find(|r| r.name == "crate-alpha")
        .expect("crate-alpha should be in releases");
    assert_eq!(
        alpha_pkg.new_version.to_string(),
        "1.0.0-beta.1",
        "alpha package should switch to beta.1 (tag change resets number)"
    );

    let beta_pkg = output
        .planned_releases
        .iter()
        .find(|r| r.name == "crate-beta")
        .expect("crate-beta should be in releases");
    assert_eq!(
        beta_pkg.new_version.to_string(),
        "2.0.0-beta.2",
        "beta package already at beta.1 should increment to beta.2"
    );
}

#[test]
fn alpha_to_beta_tag_transition_resets_number() {
    let dir = create_single_package_project();

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[package]
name = "my-crate"
version = "1.0.0-alpha.5"
edition = "2021"
"#,
    )
    .expect("update version to alpha.5");

    write_changeset(&dir, "feature.md", "my-crate", "minor", "Add feature");

    let result = run_release_with_prerelease(&dir, Some(changeset_core::PrereleaseSpec::Beta))
        .expect("release should succeed");

    let ReleaseOutcome::Executed(output) = result else {
        panic!("expected Executed outcome");
    };

    assert_eq!(output.planned_releases.len(), 1);
    assert_eq!(
        output.planned_releases[0].new_version.to_string(),
        "1.0.0-beta.1",
        "switching from alpha to beta should reset prerelease number to 1"
    );

    let version = read_version(&dir.path().join("Cargo.toml"));
    assert_eq!(
        version, "1.0.0-beta.1",
        "Cargo.toml should be updated to beta.1"
    );
}

#[test]
fn beta_to_rc_tag_transition_resets_number() {
    let dir = create_single_package_project();

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[package]
name = "my-crate"
version = "2.0.0-beta.3"
edition = "2021"
"#,
    )
    .expect("update version to beta.3");

    write_changeset(&dir, "fix.md", "my-crate", "patch", "Fix bug");

    let result = run_release_with_prerelease(&dir, Some(changeset_core::PrereleaseSpec::Rc))
        .expect("release should succeed");

    let ReleaseOutcome::Executed(output) = result else {
        panic!("expected Executed outcome");
    };

    assert_eq!(output.planned_releases.len(), 1);
    assert_eq!(
        output.planned_releases[0].new_version.to_string(),
        "2.0.0-rc.1",
        "switching from beta to rc should reset prerelease number to 1"
    );
}

#[test]
fn custom_prerelease_tag_works() {
    let dir = create_single_package_project();
    write_changeset(&dir, "feature.md", "my-crate", "minor", "Add feature");

    let result = run_release_with_prerelease(
        &dir,
        Some(changeset_core::PrereleaseSpec::Custom(
            "nightly".to_string(),
        )),
    )
    .expect("release should succeed");

    let ReleaseOutcome::Executed(output) = result else {
        panic!("expected Executed outcome");
    };

    assert_eq!(output.planned_releases.len(), 1);
    assert_eq!(
        output.planned_releases[0].new_version.to_string(),
        "1.1.0-nightly.1",
        "custom prerelease tag should be applied"
    );
}

#[test]
fn consumed_changesets_aggregated_in_changelog_on_graduation() {
    let dir = create_single_package_project();

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[package]
name = "my-crate"
version = "1.0.1-alpha.2"
edition = "2021"
"#,
    )
    .expect("update version to alpha.2");

    write_consumed_changeset(
        &dir,
        "fix1.md",
        "my-crate",
        "patch",
        "Fix bug one from alpha.1",
        "1.0.1-alpha.1",
    );
    write_consumed_changeset(
        &dir,
        "fix2.md",
        "my-crate",
        "patch",
        "Fix bug two from alpha.2",
        "1.0.1-alpha.2",
    );

    let result = run_release_with_prerelease(&dir, None).expect("graduation should succeed");

    let ReleaseOutcome::Executed(output) = result else {
        panic!("expected Executed outcome");
    };

    assert_eq!(output.planned_releases.len(), 1);
    assert_eq!(
        output.planned_releases[0].new_version.to_string(),
        "1.0.1",
        "graduation should produce stable version"
    );

    let changelog_path = dir.path().join("CHANGELOG.md");
    assert!(changelog_path.exists(), "CHANGELOG.md should be created");

    let changelog_content = fs::read_to_string(&changelog_path).expect("read CHANGELOG.md");

    assert!(
        changelog_content.contains("Fix bug one from alpha.1"),
        "changelog should contain first consumed changeset: {changelog_content}"
    );
    assert!(
        changelog_content.contains("Fix bug two from alpha.2"),
        "changelog should contain second consumed changeset: {changelog_content}"
    );
    assert!(
        changelog_content.contains("## [1.0.1]"),
        "changelog should have stable version header: {changelog_content}"
    );
}

#[test]
fn consumed_changesets_from_multiple_prereleases_aggregated() {
    let dir = create_single_package_project();

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[package]
name = "my-crate"
version = "2.0.0-rc.1"
edition = "2021"
"#,
    )
    .expect("update version to rc.1");

    write_consumed_changeset(
        &dir,
        "feature.md",
        "my-crate",
        "minor",
        "Add major feature",
        "2.0.0-alpha.1",
    );
    write_consumed_changeset(
        &dir,
        "fix.md",
        "my-crate",
        "patch",
        "Fix edge case",
        "2.0.0-beta.1",
    );
    write_consumed_changeset(
        &dir,
        "perf.md",
        "my-crate",
        "patch",
        "Improve performance",
        "2.0.0-rc.1",
    );

    let result = run_release_with_prerelease(&dir, None).expect("graduation should succeed");

    let ReleaseOutcome::Executed(output) = result else {
        panic!("expected Executed outcome");
    };

    assert_eq!(
        output.planned_releases[0].new_version.to_string(),
        "2.0.0",
        "graduation should produce stable version"
    );

    let changelog_path = dir.path().join("CHANGELOG.md");
    let changelog_content = fs::read_to_string(&changelog_path).expect("read CHANGELOG.md");

    assert!(
        changelog_content.contains("Add major feature"),
        "changelog should contain alpha changeset"
    );
    assert!(
        changelog_content.contains("Fix edge case"),
        "changelog should contain beta changeset"
    );
    assert!(
        changelog_content.contains("Improve performance"),
        "changelog should contain rc changeset"
    );
}
