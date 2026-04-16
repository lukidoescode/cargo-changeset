use changeset_test_helpers::changesets::write_changeset;
use changeset_test_helpers::workspaces::{
    WorkspaceBuilder, create_workspace_with_additional_package,
    create_workspace_with_version_tracking_additional_to_cargo,
};
use predicates::str::contains;
use tempfile::TempDir;

fn create_single_package_project() -> TempDir {
    WorkspaceBuilder::single_package("my-crate", "1.0.0")
        .with_changeset_dir()
        .build()
}

fn create_workspace_project() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member("crate-a", "1.0.0")
        .crate_member("crate-b", "2.0.0")
        .with_changeset_dir()
        .build()
}

fn create_workspace_with_inherited_versions() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .workspace_package("[workspace.package]\nversion = \"1.0.0\"\nedition = \"2021\"\n")
        .crate_member_with_inherited_version("crate-a", "crates/crate-a")
        .with_changeset_dir()
        .build()
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
