mod common;

use std::fs;

use predicates::str::contains;
use tempfile::TempDir;

use common::changesets::{write_changeset, write_multi_changeset};
use common::git::{create_tag, git_add_and_commit, init_git_repo};

fn create_single_package_with_git() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");

    init_git_repo(&dir);

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

    git_add_and_commit(&dir, "Initial commit");

    dir
}

fn create_workspace_with_two_crates() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");

    init_git_repo(&dir);

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

    git_add_and_commit(&dir, "Initial commit");

    dir
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
