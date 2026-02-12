use std::fs;

use predicates::str::contains;
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

macro_rules! cargo_changeset_status {
    () => {
        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
    };
}

#[test]
fn status_with_no_changesets() {
    let workspace = create_single_package_project();

    cargo_changeset_status!()
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

    cargo_changeset_status!()
        .arg("status")
        .current_dir(workspace.path())
        .assert()
        .success()
        .stdout(contains("Pending changesets: 1"))
        .stdout(contains("fix-bug.md"))
        .stdout(contains("Projected releases:"))
        .stdout(contains("my-crate: 1.0.0 -> 1.0.1 (Patch)"))
        .stdout(contains("Summary: 1 changeset(s), 1 package(s) affected"));
}

#[test]
fn status_shows_multiple_changesets() {
    let workspace = create_single_package_project();
    write_changeset(&workspace, "fix.md", "my-crate", "patch", "Fix bug");
    write_changeset(&workspace, "feature.md", "my-crate", "minor", "Add feature");

    cargo_changeset_status!()
        .arg("status")
        .current_dir(workspace.path())
        .assert()
        .success()
        .stdout(contains("Pending changesets: 2"))
        .stdout(contains("my-crate: 1.0.0 -> 1.1.0 (Minor)"))
        .stdout(contains("(from: Patch, Minor)"));
}

#[test]
fn status_shows_workspace_packages() {
    let workspace = create_workspace_project();
    write_changeset(&workspace, "fix-a.md", "crate-a", "patch", "Fix A");

    cargo_changeset_status!()
        .arg("status")
        .current_dir(workspace.path())
        .assert()
        .success()
        .stdout(contains("crate-a: 1.0.0 -> 1.0.1 (Patch)"))
        .stdout(contains("Packages without changesets:"))
        .stdout(contains("crate-b (2.0.0)"));
}

#[test]
fn status_shows_inherited_version_warning() {
    let workspace = create_workspace_with_inherited_versions();

    cargo_changeset_status!()
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

    cargo_changeset_status!()
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

    cargo_changeset_status!()
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

    cargo_changeset_status!()
        .arg("status")
        .current_dir(workspace.path())
        .assert()
        .success()
        .stdout(contains("Pending changesets: 3"))
        .stdout(contains(
            "crate-a: 1.0.0 -> 1.1.0 (Minor) (from: Patch, Minor)",
        ))
        .stdout(contains("crate-b: 2.0.0 -> 3.0.0 (Major)"))
        .stdout(contains("Summary: 3 changeset(s), 2 package(s) affected"));
}
