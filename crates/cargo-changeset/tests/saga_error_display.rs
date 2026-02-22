use std::fs;
use std::process::Command;

use predicates::str::contains;
use tempfile::TempDir;

fn init_git_repo(dir: &TempDir) {
    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(dir.path())
        .output()
        .expect("failed to init git repo");

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(dir.path())
        .output()
        .expect("failed to configure git email");

    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir.path())
        .output()
        .expect("failed to configure git name");
}

fn git_add_and_commit(dir: &TempDir, message: &str) {
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir.path())
        .output()
        .expect("failed to git add");

    Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(dir.path())
        .output()
        .expect("failed to git commit");
}

fn create_tag(dir: &TempDir, tag_name: &str, message: &str) {
    Command::new("git")
        .args(["tag", "-a", tag_name, "-m", message])
        .current_dir(dir.path())
        .output()
        .expect("failed to create tag");
}

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

fn write_multi_package_changeset(
    dir: &TempDir,
    filename: &str,
    packages: &[(&str, &str)],
    summary: &str,
) {
    let package_entries: String = packages
        .iter()
        .map(|(pkg, bump)| format!("\"{pkg}\": {bump}"))
        .collect::<Vec<_>>()
        .join("\n");

    let content = format!(
        r#"---
{package_entries}
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

macro_rules! cargo_changeset {
    () => {
        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
    };
}

#[test]
fn release_saga_failure_shows_failed_step_and_rollback_message() {
    let workspace = create_single_package_with_git();
    write_changeset(&workspace, "fix.md", "my-crate", "patch", "Fix a bug");
    git_add_and_commit(&workspace, "Add changeset");

    create_tag(&workspace, "v1.0.1", "Pre-existing conflicting tag");

    cargo_changeset!()
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

    cargo_changeset!()
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

    cargo_changeset!()
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
    write_multi_package_changeset(
        &workspace,
        "multi.md",
        &[("crate-a", "patch"), ("crate-b", "patch")],
        "Fix bugs in both crates",
    );
    git_add_and_commit(&workspace, "Add changeset");

    create_tag(
        &workspace,
        "crate-b-v2.0.1",
        "Pre-existing conflicting tag for crate-b",
    );

    cargo_changeset!()
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

    cargo_changeset!()
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("->"));
}
