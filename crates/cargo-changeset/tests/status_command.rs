mod common;

use std::fs;

use predicates::str::contains;
use tempfile::TempDir;

use common::changesets::write_changeset;
use common::workspaces::{
    create_workspace_with_additional_package,
    create_workspace_with_version_tracking_additional_to_cargo,
};

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

    fs::create_dir_all(dir.path().join(".changeset/changesets"))
        .expect("create .changeset/changesets dir");

    dir
}

#[test]
fn status_with_no_changesets() {
    let workspace = create_single_package_project();

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("status")
        .current_dir(workspace.path())
        .assert()
        .success()
        .stdout(contains("No pending changesets."));
}

#[test]
fn status_shows_single_changeset() {
    let workspace = create_single_package_project();
    write_changeset(&workspace, "fix-bug.md", "my-crate", "patch", "Fix a bug");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("status")
        .current_dir(workspace.path())
        .assert()
        .success()
        .stdout(contains("Pending changesets: 1"))
        .stdout(contains("fix-bug.md"))
        .stdout(contains("Projected releases:"))
        .stdout(contains("my-crate: 1.0.0 -> 1.0.1 (patch)"))
        .stdout(contains("Summary: 1 changeset(s), 1 package(s) to release"));
}

#[test]
fn status_shows_multiple_changesets() {
    let workspace = create_single_package_project();
    write_changeset(&workspace, "fix.md", "my-crate", "patch", "Fix bug");
    write_changeset(&workspace, "feature.md", "my-crate", "minor", "Add feature");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("status")
        .current_dir(workspace.path())
        .assert()
        .success()
        .stdout(contains("Pending changesets: 2"))
        .stdout(contains("my-crate: 1.0.0 -> 1.1.0 (minor)"))
        .stdout(contains("(from: patch, minor)"));
}

#[test]
fn status_shows_workspace_packages() {
    let workspace = create_workspace_project();
    write_changeset(&workspace, "fix-a.md", "crate-a", "patch", "Fix A");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("status")
        .current_dir(workspace.path())
        .assert()
        .success()
        .stdout(contains("crate-a: 1.0.0 -> 1.0.1 (patch)"))
        .stdout(contains("Packages without changesets:"))
        .stdout(contains("crate-b (2.0.0)"));
}

#[test]
fn status_shows_inherited_version_warning() {
    let workspace = create_workspace_with_inherited_versions();

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("status")
        .current_dir(workspace.path())
        .assert()
        .success()
        .stdout(contains("No pending changesets."))
        .stdout(contains("Warning: Packages with inherited versions:"))
        .stdout(contains("crate-a"))
        .stdout(contains("--convert flag"));
}

#[test]
fn status_shows_inherited_version_warning_with_changesets() {
    let workspace = create_workspace_with_inherited_versions();
    write_changeset(&workspace, "fix.md", "crate-a", "patch", "Fix");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("status")
        .current_dir(workspace.path())
        .assert()
        .success()
        .stdout(contains("Pending changesets: 1"))
        .stdout(contains("Warning: Packages with inherited versions:"))
        .stdout(contains("--convert flag"));
}

#[test]
fn status_shows_unknown_package_warning() {
    let workspace = create_single_package_project();
    write_changeset(
        &workspace,
        "fix.md",
        "nonexistent-crate",
        "patch",
        "Fix typo",
    );

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("status")
        .current_dir(workspace.path())
        .assert()
        .success()
        .stdout(contains("Warning: Unknown packages in changesets:"))
        .stdout(contains("nonexistent-crate"));
}

#[test]
fn status_multiple_packages_multiple_bumps() {
    let workspace = create_workspace_project();
    write_changeset(&workspace, "fix-a.md", "crate-a", "patch", "Fix A");
    write_changeset(&workspace, "feature-a.md", "crate-a", "minor", "Feature A");
    write_changeset(
        &workspace,
        "breaking-b.md",
        "crate-b",
        "major",
        "Breaking B",
    );

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("status")
        .current_dir(workspace.path())
        .assert()
        .success()
        .stdout(contains("Pending changesets: 3"))
        .stdout(contains(
            "crate-a: 1.0.0 -> 1.1.0 (minor) (from: patch, minor)",
        ))
        .stdout(contains("crate-b: 2.0.0 -> 3.0.0 (major)"))
        .stdout(contains("Summary: 3 changeset(s), 2 package(s) to release"));
}

#[test]
fn status_shows_additional_package_with_changeset() {
    let workspace = create_workspace_with_additional_package();
    write_changeset(
        &workspace,
        "helm-feat.md",
        "my-helm-chart",
        "minor",
        "Add feature",
    );

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("status")
        .current_dir(workspace.path())
        .assert()
        .success()
        .stdout(contains("my-helm-chart"))
        .stdout(contains("2.0.0 -> 2.1.0 (minor)"));
}

#[test]
fn status_lists_additional_package_without_changeset() {
    let workspace = create_workspace_with_additional_package();
    write_changeset(
        &workspace,
        "crate-fix.md",
        "crate-a",
        "patch",
        "Fix crate-a",
    );

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("status")
        .current_dir(workspace.path())
        .assert()
        .success()
        .stdout(contains("Packages without changesets:"))
        .stdout(contains("my-helm-chart (2.0.0)"));
}

#[test]
fn status_shows_projected_auto_patch_for_version_tracking_deps() {
    let workspace = create_workspace_with_version_tracking_additional_to_cargo();
    write_changeset(
        &workspace,
        "bump-rust.md",
        "my-rust-crate",
        "patch",
        "Fix a bug",
    );

    let output = assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("status")
        .current_dir(workspace.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);

    assert!(
        stdout.contains("my-rust-crate: 1.0.0 -> 1.0.1"),
        "expected my-rust-crate projected release in status output, got:\n{stdout}"
    );
    assert!(
        stdout.contains("my-helm-chart"),
        "expected my-helm-chart (auto-patch dependent) in status output, got:\n{stdout}"
    );
}

#[test]
fn status_untracked_dep_not_releasing_does_not_auto_patch() {
    let workspace = create_workspace_with_version_tracking_additional_to_cargo();

    let output = assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("status")
        .current_dir(workspace.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);

    assert!(
        !stdout.contains("my-helm-chart: 0.1.0 ->"),
        "expected my-helm-chart NOT to have a projected release when its dependency is not releasing, got:\n{stdout}"
    );
}
