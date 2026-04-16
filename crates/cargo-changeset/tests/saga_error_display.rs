use std::fs;

use changeset_test_helpers::changesets::{write_changeset, write_multi_changeset};
use changeset_test_helpers::git::{create_tag, git_add_and_commit};
use changeset_test_helpers::workspaces::WorkspaceBuilder;
use predicates::str::contains;
use tempfile::TempDir;

fn create_single_package_with_git() -> TempDir {
    WorkspaceBuilder::single_package("my-crate", "1.0.0")
        .with_changeset_dir()
        .with_git()
        .build()
}

fn create_workspace_with_two_crates() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member("crate-a", "1.0.0")
        .crate_member("crate-b", "2.0.0")
        .with_changeset_dir()
        .with_git()
        .build()
}

#[test]
fn release_saga_failure_shows_failed_step_and_rollback_message() {
    let workspace = create_single_package_with_git();
    write_changeset(&workspace, "fix.md", "my-crate", "patch", "Fix a bug");
    git_add_and_commit(&workspace, "Add changeset");

    create_tag(&workspace, "v1.0.1", "Pre-existing conflicting tag");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("Error: Release failed at step"))
        .stderr(contains("create_tags"))
        .stderr(contains("Rollback completed successfully"))
        .stderr(contains("restored to its original state"));
}

#[test]
fn release_saga_failure_message_includes_step_name() {
    let workspace = create_single_package_with_git();
    write_changeset(&workspace, "fix.md", "my-crate", "patch", "Fix a bug");
    git_add_and_commit(&workspace, "Add changeset");

    create_tag(&workspace, "v1.0.1", "Pre-existing conflicting tag");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("'create_tags'"));
}

#[test]
fn release_saga_failure_with_rollback_restores_version_in_manifest() {
    let workspace = create_single_package_with_git();
    write_changeset(&workspace, "fix.md", "my-crate", "patch", "Fix a bug");
    git_add_and_commit(&workspace, "Add changeset");

    create_tag(&workspace, "v1.0.1", "Pre-existing conflicting tag");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .failure();

    let manifest_content =
        fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
    assert!(
        manifest_content.contains("version = \"1.0.0\""),
        "version should be restored to original after rollback"
    );
}

#[test]
fn release_saga_failure_multi_package_shows_proper_error_format() {
    let workspace = create_workspace_with_two_crates();
    write_multi_changeset(
        &workspace,
        "multi.md",
        &[("crate-a", "patch"), ("crate-b", "patch")],
        "Fix bugs in both crates",
    );
    git_add_and_commit(&workspace, "Add changeset");

    create_tag(
        &workspace,
        "crate-b@v2.0.1",
        "Pre-existing conflicting tag for crate-b",
    );

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("Error: Release failed at step"))
        .stderr(contains("Rollback completed successfully"));
}

#[test]
fn release_saga_failure_error_includes_cause_chain() {
    let workspace = create_single_package_with_git();
    write_changeset(&workspace, "fix.md", "my-crate", "patch", "Fix a bug");
    git_add_and_commit(&workspace, "Add changeset");

    create_tag(&workspace, "v1.0.1", "Pre-existing conflicting tag");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("->"));
}
