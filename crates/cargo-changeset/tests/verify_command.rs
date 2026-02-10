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

fn create_branch(dir: &TempDir, name: &str) {
    Command::new("git")
        .args(["checkout", "-b", name])
        .current_dir(dir.path())
        .output()
        .expect("failed to create branch");
}

fn create_virtual_workspace_with_git() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    init_git_repo(&dir);

    fs::create_dir_all(dir.path().join("crates/crate-a/src"))
        .expect("failed to create crate a dir");
    fs::create_dir_all(dir.path().join("crates/crate-b/src"))
        .expect("failed to create crate b dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"
[workspace]
members = ["crates/*"]
resolver = "2"
"#,
    )
    .expect("failed to write workspace Cargo.toml");

    fs::write(
        dir.path().join("crates/crate-a/Cargo.toml"),
        r#"
[package]
name = "crate-a"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("failed to write crate-a Cargo.toml");

    fs::write(dir.path().join("crates/crate-a/src/lib.rs"), "")
        .expect("failed to write crate-a lib.rs");

    fs::write(
        dir.path().join("crates/crate-b/Cargo.toml"),
        r#"
[package]
name = "crate-b"
version = "0.2.0"
edition = "2021"
"#,
    )
    .expect("failed to write crate-b Cargo.toml");

    fs::write(dir.path().join("crates/crate-b/src/lib.rs"), "")
        .expect("failed to write crate-b lib.rs");

    git_add_and_commit(&dir, "Initial commit");

    dir
}

fn add_changeset(dir: &TempDir, package_name: &str) {
    fs::create_dir_all(dir.path().join(".changeset")).expect("failed to create .changeset dir");
    let filename = format!(".changeset/{package_name}-changeset.md");
    fs::write(
        dir.path().join(&filename),
        format!(
            r#"---
"{package_name}": patch
---

Test changeset for {package_name}.
"#
        ),
    )
    .expect("failed to write changeset");
}

#[test]
fn verify_exit_code_0_when_all_changes_covered() {
    let workspace = create_virtual_workspace_with_git();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// changed",
    )
    .expect("failed to modify lib.rs");

    add_changeset(&workspace, "crate-a");
    git_add_and_commit(&workspace, "Add changes with changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_exit_code_1_when_package_uncovered() {
    let workspace = create_virtual_workspace_with_git();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// changed",
    )
    .expect("failed to modify lib.rs");

    git_add_and_commit(&workspace, "Add changes without changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("crate-a"))
        .stderr(contains("without changeset coverage"));
}

#[test]
fn verify_exit_code_0_when_only_changeset_directory_changes() {
    let workspace = create_virtual_workspace_with_git();
    create_branch(&workspace, "feature");

    add_changeset(&workspace, "crate-a");
    git_add_and_commit(&workspace, "Add changeset only");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_exit_code_0_for_workspace_ignored_files() {
    let dir = TempDir::new().expect("failed to create temp dir");

    init_git_repo(&dir);

    fs::create_dir_all(dir.path().join("crates/my-crate/src")).expect("failed to create crate dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.metadata.changeset]
ignored-files = ["*.md", "docs/**"]
"#,
    )
    .expect("failed to write workspace Cargo.toml");

    fs::write(
        dir.path().join("crates/my-crate/Cargo.toml"),
        r#"
[package]
name = "my-crate"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("failed to write crate Cargo.toml");

    fs::write(dir.path().join("crates/my-crate/src/lib.rs"), "").expect("failed to write lib.rs");

    git_add_and_commit(&dir, "Initial commit");
    create_branch(&dir, "feature");

    fs::write(dir.path().join("README.md"), "# README").expect("failed to write README.md");
    git_add_and_commit(&dir, "Add README");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn verify_exit_code_0_for_package_ignored_files() {
    let dir = TempDir::new().expect("failed to create temp dir");

    init_git_repo(&dir);

    fs::create_dir_all(dir.path().join("crates/my-crate/src")).expect("failed to create crate dir");
    fs::create_dir_all(dir.path().join("crates/my-crate/benches"))
        .expect("failed to create benches dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"
[workspace]
members = ["crates/*"]
resolver = "2"
"#,
    )
    .expect("failed to write workspace Cargo.toml");

    fs::write(
        dir.path().join("crates/my-crate/Cargo.toml"),
        r#"
[package]
name = "my-crate"
version = "0.1.0"
edition = "2021"

[package.metadata.changeset]
ignored-files = ["benches/**"]
"#,
    )
    .expect("failed to write crate Cargo.toml");

    fs::write(dir.path().join("crates/my-crate/src/lib.rs"), "").expect("failed to write lib.rs");

    fs::write(dir.path().join("crates/my-crate/benches/bench.rs"), "")
        .expect("failed to write bench.rs");

    git_add_and_commit(&dir, "Initial commit");
    create_branch(&dir, "feature");

    fs::write(
        dir.path().join("crates/my-crate/benches/bench.rs"),
        "// updated benchmark",
    )
    .expect("failed to update bench.rs");
    git_add_and_commit(&dir, "Update benchmark");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn verify_exit_code_1_for_nonexistent_base_branch() {
    let workspace = create_virtual_workspace_with_git();

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("nonexistent-branch")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("failed to resolve reference"));
}

#[test]
fn verify_verbose_shows_details() {
    let workspace = create_virtual_workspace_with_git();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// changed",
    )
    .expect("failed to modify lib.rs");

    add_changeset(&workspace, "crate-a");
    git_add_and_commit(&workspace, "Add changes with changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .arg("--verbose")
        .current_dir(workspace.path())
        .assert()
        .success()
        .stdout(contains("Changed packages"))
        .stdout(contains("crate-a"));
}

#[test]
fn verify_with_custom_base_branch() {
    let workspace = create_virtual_workspace_with_git();

    create_branch(&workspace, "develop");
    git_add_and_commit(&workspace, "Develop base");

    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-b/src/lib.rs"),
        "// changed",
    )
    .expect("failed to modify lib.rs");

    add_changeset(&workspace, "crate-b");
    git_add_and_commit(&workspace, "Add changes with changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("develop")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_no_changes_passes() {
    let workspace = create_virtual_workspace_with_git();
    create_branch(&workspace, "feature");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_multiple_packages_one_uncovered() {
    let workspace = create_virtual_workspace_with_git();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// changed a",
    )
    .expect("failed to modify crate-a lib.rs");

    fs::write(
        workspace.path().join("crates/crate-b/src/lib.rs"),
        "// changed b",
    )
    .expect("failed to modify crate-b lib.rs");

    add_changeset(&workspace, "crate-a");
    git_add_and_commit(&workspace, "Add changes with partial changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("crate-b"))
        .stderr(contains("without changeset coverage"));
}

#[test]
fn verify_project_level_changes_pass_without_changeset() {
    let workspace = create_virtual_workspace_with_git();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("Cargo.toml"),
        r#"
[workspace]
members = ["crates/*"]
resolver = "2"

# Added a comment
"#,
    )
    .expect("failed to modify workspace Cargo.toml");

    git_add_and_commit(&workspace, "Update workspace Cargo.toml");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_fails_on_malformed_changeset() {
    let workspace = create_virtual_workspace_with_git();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// changed",
    )
    .expect("failed to modify lib.rs");

    fs::create_dir_all(workspace.path().join(".changeset")).expect("failed to create .changeset");
    fs::write(
        workspace.path().join(".changeset/malformed.md"),
        r#"---
invalid yaml {{{ not closed
---

This changeset is malformed.
"#,
    )
    .expect("failed to write malformed changeset");

    git_add_and_commit(&workspace, "Add malformed changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("failed to parse changeset file"));
}
