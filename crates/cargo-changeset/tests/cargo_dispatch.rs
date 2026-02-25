use std::fs;
use std::process::Command;

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

fn create_single_package_project() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    init_git_repo(&dir);

    fs::create_dir_all(dir.path().join("src")).expect("failed to create src dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"
[package]
name = "my-crate"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("failed to write Cargo.toml");

    fs::write(dir.path().join("src/lib.rs"), "").expect("failed to write lib.rs");

    git_add_and_commit(&dir, "Initial commit");

    dir
}

#[test]
fn cargo_dispatch_verify_succeeds_with_changeset_prefix() {
    let workspace = create_single_package_project();

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn cargo_dispatch_help_succeeds_with_changeset_prefix() {
    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("changeset")
        .arg("--help")
        .assert()
        .success();
}

#[test]
fn version_flag_succeeds() {
    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn cargo_dispatch_version_succeeds_with_changeset_prefix() {
    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("changeset")
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::contains(env!("CARGO_PKG_VERSION")));
}
