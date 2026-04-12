mod common;

use std::fs;
use std::process::Command;

use common::changesets::{write_changeset, write_multi_changeset};
use common::git::{git_add_and_commit, init_git_repo};
use common::workspaces::{add_helm_chart_config, create_workspace_with_helm_chart};

#[test]
fn release_bumps_chart_yaml_version_and_preserves_inline_comments() {
    let workspace = create_workspace_with_helm_chart();
    init_git_repo(&workspace);
    add_helm_chart_config(&workspace);
    git_add_and_commit(&workspace, "Initial commit");
    write_changeset(
        &workspace,
        "helm-feat.md",
        "my-helm-chart",
        "minor",
        "Add feature",
    );
    git_add_and_commit(&workspace, "Add changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .success();

    let chart_yaml = fs::read_to_string(workspace.path().join("charts/my-chart/Chart.yaml"))
        .expect("failed to read Chart.yaml");

    assert!(
        chart_yaml.contains("version: 2.1.0"),
        "expected bumped version 2.1.0 in Chart.yaml, got:\n{chart_yaml}"
    );
    assert!(
        chart_yaml.contains("# This comment should survive release"),
        "expected inline comment to be preserved in Chart.yaml, got:\n{chart_yaml}"
    );

    assert!(
        !workspace
            .path()
            .join(".changeset/changesets/helm-feat.md")
            .exists(),
        "changeset file should have been consumed by release"
    );
}

#[test]
fn release_mixed_rust_and_helm_chart() {
    let workspace = create_workspace_with_helm_chart();
    init_git_repo(&workspace);
    add_helm_chart_config(&workspace);
    git_add_and_commit(&workspace, "Initial commit");

    let lockfile_output = Command::new("cargo")
        .args(["generate-lockfile"])
        .current_dir(workspace.path())
        .output()
        .expect("failed to run cargo generate-lockfile");
    assert!(
        lockfile_output.status.success(),
        "cargo generate-lockfile failed: {}",
        String::from_utf8_lossy(&lockfile_output.stderr)
    );

    git_add_and_commit(&workspace, "Add lockfile");

    write_multi_changeset(
        &workspace,
        "mixed.md",
        &[("crate-a", "patch"), ("my-helm-chart", "minor")],
        "Mixed release",
    );
    git_add_and_commit(&workspace, "Add changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .success();

    let crate_a_toml = fs::read_to_string(workspace.path().join("crates/crate-a/Cargo.toml"))
        .expect("failed to read crate-a Cargo.toml");
    assert!(
        crate_a_toml.contains("version = \"1.0.1\""),
        "expected crate-a version 1.0.1, got:\n{crate_a_toml}"
    );

    let chart_yaml = fs::read_to_string(workspace.path().join("charts/my-chart/Chart.yaml"))
        .expect("failed to read Chart.yaml");
    assert!(
        chart_yaml.contains("version: 2.1.0"),
        "expected Chart.yaml version 2.1.0, got:\n{chart_yaml}"
    );
}

#[test]
fn release_dry_run_does_not_modify_chart_yaml() {
    let workspace = create_workspace_with_helm_chart();
    init_git_repo(&workspace);
    add_helm_chart_config(&workspace);
    git_add_and_commit(&workspace, "Initial commit");
    write_changeset(
        &workspace,
        "helm-feat.md",
        "my-helm-chart",
        "minor",
        "Add feature",
    );
    git_add_and_commit(&workspace, "Add changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .args(["release", "--dry-run"])
        .current_dir(workspace.path())
        .assert()
        .success();

    let chart_yaml = fs::read_to_string(workspace.path().join("charts/my-chart/Chart.yaml"))
        .expect("failed to read Chart.yaml");
    assert!(
        chart_yaml.contains("version: \"2.0.0\""),
        "expected Chart.yaml version to remain 2.0.0 after dry-run, got:\n{chart_yaml}"
    );
}

#[test]
fn release_only_additional_package_no_rust_crates() {
    let workspace = create_workspace_with_helm_chart();
    init_git_repo(&workspace);
    add_helm_chart_config(&workspace);
    git_add_and_commit(&workspace, "Initial commit");
    write_changeset(
        &workspace,
        "helm-only.md",
        "my-helm-chart",
        "patch",
        "Fix chart configuration",
    );
    git_add_and_commit(&workspace, "Add changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .success();

    let chart_yaml = fs::read_to_string(workspace.path().join("charts/my-chart/Chart.yaml"))
        .expect("failed to read Chart.yaml");
    assert!(
        chart_yaml.contains("version: 2.0.1"),
        "expected patch-bumped version 2.0.1 in Chart.yaml, got:\n{chart_yaml}"
    );

    let crate_a_toml = fs::read_to_string(workspace.path().join("crates/crate-a/Cargo.toml"))
        .expect("failed to read crate-a Cargo.toml");
    assert!(
        crate_a_toml.contains("version = \"1.0.0\""),
        "expected crate-a version to remain 1.0.0, got:\n{crate_a_toml}"
    );
}

#[test]
fn release_major_bump_on_additional_package() {
    let workspace = create_workspace_with_helm_chart();
    init_git_repo(&workspace);
    add_helm_chart_config(&workspace);
    git_add_and_commit(&workspace, "Initial commit");
    write_changeset(
        &workspace,
        "helm-major.md",
        "my-helm-chart",
        "major",
        "Breaking chart API change",
    );
    git_add_and_commit(&workspace, "Add changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .success();

    let chart_yaml = fs::read_to_string(workspace.path().join("charts/my-chart/Chart.yaml"))
        .expect("failed to read Chart.yaml");
    assert!(
        chart_yaml.contains("version: 3.0.0"),
        "expected major-bumped version 3.0.0 in Chart.yaml, got:\n{chart_yaml}"
    );
}
