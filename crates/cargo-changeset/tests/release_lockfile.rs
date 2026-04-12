mod common;

use std::fs;
use std::process::Command;

use tempfile::TempDir;

use common::changesets::write_changeset;
use common::git::{git_add_and_commit, init_git_repo};

fn lockfile_hash(dir: &TempDir) -> Vec<u8> {
    let output = Command::new("git")
        .args(["hash-object", "Cargo.lock"])
        .current_dir(dir.path())
        .output()
        .expect("failed to hash Cargo.lock");

    output.stdout
}

fn setup_workspace_with_dependency() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");

    init_git_repo(&dir);

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.dependencies]
crate-a = { path = "crates/crate-a", version = "1.0.0" }
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
    fs::write(dir.path().join("crates/crate-a/src/lib.rs"), "").expect("write crate-a lib.rs");

    fs::create_dir_all(dir.path().join("crates/crate-b/src")).expect("create crate-b dir");
    fs::write(
        dir.path().join("crates/crate-b/Cargo.toml"),
        r#"[package]
name = "crate-b"
version = "2.0.0"
edition = "2021"

[dependencies]
crate-a = { workspace = true }
"#,
    )
    .expect("write crate-b Cargo.toml");
    fs::write(dir.path().join("crates/crate-b/src/lib.rs"), "").expect("write crate-b lib.rs");

    fs::create_dir_all(dir.path().join(".changeset/changesets"))
        .expect("create .changeset/changesets dir");

    let output = Command::new("cargo")
        .args(["generate-lockfile"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run cargo generate-lockfile");
    assert!(
        output.status.success(),
        "cargo generate-lockfile failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    git_add_and_commit(&dir, "Initial commit");

    dir
}

#[test]
fn release_lockfile_not_dirtied_by_cargo_check() {
    let workspace = setup_workspace_with_dependency();
    write_changeset(&workspace, "fix.md", "crate-a", "patch", "Fix a bug");
    git_add_and_commit(&workspace, "Add changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .success();

    let hash_before = lockfile_hash(&workspace);

    let check_output = Command::new("cargo")
        .args(["check"])
        .current_dir(workspace.path())
        .output()
        .expect("failed to run cargo check");
    assert!(
        check_output.status.success(),
        "cargo check failed: {}",
        String::from_utf8_lossy(&check_output.stderr)
    );

    let hash_after = lockfile_hash(&workspace);

    assert_eq!(
        hash_before, hash_after,
        "cargo check should not change Cargo.lock after release — \
         the release commit must include an up-to-date lockfile"
    );
}

#[test]
fn release_lockfile_correct_after_minor_bump_with_workspace_dep() {
    let workspace = setup_workspace_with_dependency();
    write_changeset(&workspace, "feat.md", "crate-a", "minor", "Add a feature");
    git_add_and_commit(&workspace, "Add changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .success();

    let hash_before = lockfile_hash(&workspace);

    let check_output = Command::new("cargo")
        .args(["check"])
        .current_dir(workspace.path())
        .output()
        .expect("failed to run cargo check");
    assert!(
        check_output.status.success(),
        "cargo check failed: {}",
        String::from_utf8_lossy(&check_output.stderr)
    );

    let hash_after = lockfile_hash(&workspace);

    assert_eq!(
        hash_before, hash_after,
        "cargo check should not change Cargo.lock after minor bump release"
    );
}
