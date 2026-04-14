mod common;

use std::fs;
use std::process::Command;

use predicates::str::is_match;

use common::changesets::{write_changeset, write_multi_changeset};
use common::git::{git_add_and_commit, init_git_repo};
use common::workspaces::{
    add_helm_chart_config, create_workspace_with_cascade_chain,
    create_workspace_with_circular_version_tracking,
    create_workspace_with_deeply_nested_json_field, create_workspace_with_duplicate_dependency,
    create_workspace_with_helm_chart, create_workspace_with_invalid_version_field_path,
    create_workspace_with_json_extra_fields, create_workspace_with_multiple_deps_on_same_crate,
    create_workspace_with_prerelease_dependency,
    create_workspace_with_version_tracking_additional_to_cargo,
};

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

#[test]
fn release_dry_run_does_not_update_version_tracking_manifests() {
    let workspace = create_workspace_with_version_tracking_additional_to_cargo();
    init_git_repo(&workspace);

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

    git_add_and_commit(&workspace, "Initial commit");

    write_changeset(
        &workspace,
        "bump-rust.md",
        "my-rust-crate",
        "patch",
        "Fix a bug",
    );
    git_add_and_commit(&workspace, "Add changeset");

    let chart_before = fs::read_to_string(workspace.path().join("charts/Chart.yaml"))
        .expect("failed to read Chart.yaml before dry-run");
    let rust_toml_before =
        fs::read_to_string(workspace.path().join("crates/my-rust-crate/Cargo.toml"))
            .expect("failed to read Cargo.toml before dry-run");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .args(["release", "--dry-run"])
        .current_dir(workspace.path())
        .assert()
        .success();

    let chart_after = fs::read_to_string(workspace.path().join("charts/Chart.yaml"))
        .expect("failed to read Chart.yaml after dry-run");
    let rust_toml_after =
        fs::read_to_string(workspace.path().join("crates/my-rust-crate/Cargo.toml"))
            .expect("failed to read Cargo.toml after dry-run");

    assert_eq!(
        chart_before, chart_after,
        "Chart.yaml should not be modified by dry-run"
    );
    assert_eq!(
        rust_toml_before, rust_toml_after,
        "Cargo.toml should not be modified by dry-run"
    );
}

#[test]
fn release_detects_circular_dependency_fails_with_clear_error() {
    let workspace = create_workspace_with_circular_version_tracking();
    init_git_repo(&workspace);

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

    git_add_and_commit(&workspace, "Initial commit");

    write_changeset(&workspace, "bump-a.md", "crate-a", "patch", "Some fix");
    git_add_and_commit(&workspace, "Add changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(is_match(r"(?i)circular").expect("valid regex"));
}

#[test]
fn release_detects_duplicate_dependency_declarations() {
    let workspace = create_workspace_with_duplicate_dependency();
    init_git_repo(&workspace);

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

    git_add_and_commit(&workspace, "Initial commit");

    write_changeset(&workspace, "bump-a.md", "crate-a", "patch", "Some fix");
    git_add_and_commit(&workspace, "Add changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(is_match(r"(?i)duplicate").expect("valid regex"));
}

#[test]
fn release_dep_chain_cascading_auto_patch() {
    let workspace = create_workspace_with_cascade_chain();
    init_git_repo(&workspace);

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

    git_add_and_commit(&workspace, "Initial commit");

    write_changeset(
        &workspace,
        "bump-a.md",
        "crate-a",
        "patch",
        "Fix in crate-a",
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
        "expected crate-a bumped to 1.0.1, got:\n{crate_a_toml}"
    );

    let pkg_b_manifest = fs::read_to_string(workspace.path().join("packages/pkg-b/manifest.json"))
        .expect("failed to read pkg-b manifest.json");
    assert!(
        pkg_b_manifest.contains("\"version\": \"2.0.1\"")
            || pkg_b_manifest.contains("\"version\":\"2.0.1\""),
        "expected pkg-b auto-bumped to 2.0.1, got:\n{pkg_b_manifest}"
    );
    assert!(
        pkg_b_manifest.contains("\"upstreamVersion\": \"1.0.1\"")
            || pkg_b_manifest.contains("\"upstreamVersion\":\"1.0.1\""),
        "expected pkg-b upstreamVersion updated to 1.0.1, got:\n{pkg_b_manifest}"
    );

    let pkg_c_manifest = fs::read_to_string(workspace.path().join("packages/pkg-c/manifest.json"))
        .expect("failed to read pkg-c manifest.json");
    assert!(
        pkg_c_manifest.contains("\"version\": \"3.0.1\"")
            || pkg_c_manifest.contains("\"version\":\"3.0.1\""),
        "expected pkg-c auto-bumped to 3.0.1, got:\n{pkg_c_manifest}"
    );
    assert!(
        pkg_c_manifest.contains("\"upstreamVersion\": \"2.0.1\"")
            || pkg_c_manifest.contains("\"upstreamVersion\":\"2.0.1\""),
        "expected pkg-c upstreamVersion updated to 2.0.1, got:\n{pkg_c_manifest}"
    );
}

#[test]
fn release_no_auto_patch_when_dependency_not_released() {
    let workspace = create_workspace_with_version_tracking_additional_to_cargo();
    init_git_repo(&workspace);

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

    git_add_and_commit(&workspace, "Initial commit");

    write_changeset(
        &workspace,
        "bump-chart.md",
        "my-helm-chart",
        "minor",
        "Add chart feature",
    );
    git_add_and_commit(&workspace, "Add changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .success();

    let chart_yaml = fs::read_to_string(workspace.path().join("charts/Chart.yaml"))
        .expect("failed to read Chart.yaml");
    assert!(
        chart_yaml.contains("version: 0.1.1"),
        "expected my-helm-chart patched to 0.1.1, got:\n{chart_yaml}"
    );
    assert!(
        chart_yaml.contains("appVersion: 1.0.0") || chart_yaml.contains("appVersion: \"1.0.0\""),
        "expected appVersion to remain 1.0.0 since my-rust-crate was not released, got:\n{chart_yaml}"
    );

    let rust_toml = fs::read_to_string(workspace.path().join("crates/my-rust-crate/Cargo.toml"))
        .expect("failed to read my-rust-crate Cargo.toml");
    assert!(
        rust_toml.contains("version = \"1.0.0\""),
        "expected my-rust-crate to remain at 1.0.0, got:\n{rust_toml}"
    );
}

#[test]
fn release_preserves_structure_in_json_version_tracking_manifest() {
    let workspace = create_workspace_with_json_extra_fields();
    init_git_repo(&workspace);

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

    git_add_and_commit(&workspace, "Initial commit");

    write_changeset(
        &workspace,
        "bump-a.md",
        "crate-a",
        "patch",
        "Fix in crate-a",
    );
    git_add_and_commit(&workspace, "Add changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .success();

    let app_json_content = fs::read_to_string(workspace.path().join("packages/my-app/app.json"))
        .expect("failed to read app.json");

    let parsed: serde_json::Value =
        serde_json::from_str(&app_json_content).expect("app.json should be valid JSON");

    assert_eq!(
        parsed.get("appVersion").and_then(|v| v.as_str()),
        Some("1.0.1"),
        "expected appVersion updated to 1.0.1, got:\n{app_json_content}"
    );

    assert_eq!(
        parsed.get("description").and_then(|v| v.as_str()),
        Some("my app"),
        "expected description field preserved, got:\n{app_json_content}"
    );

    assert_eq!(
        parsed.get("extraField").and_then(|v| v.as_i64()),
        Some(42),
        "expected extraField preserved, got:\n{app_json_content}"
    );

    let nested = parsed
        .get("nested")
        .expect("expected nested field to be present");
    assert_eq!(
        nested.get("key").and_then(|v| v.as_str()),
        Some("value"),
        "expected nested.key to be 'value', got:\n{app_json_content}"
    );

    assert_eq!(
        parsed.get("version").and_then(|v| v.as_str()),
        Some("0.1.1"),
        "expected my-app version auto-patched to 0.1.1, got:\n{app_json_content}"
    );
}

#[test]
fn release_version_tracking_with_multiple_deps_on_same_package() {
    let workspace = create_workspace_with_multiple_deps_on_same_crate();
    init_git_repo(&workspace);

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

    git_add_and_commit(&workspace, "Initial commit");

    write_changeset(
        &workspace,
        "bump-a.md",
        "crate-a",
        "patch",
        "Fix in crate-a",
    );
    git_add_and_commit(&workspace, "Add changeset");

    // The workspace declares duplicate dependency-name = "crate-a", which
    // triggers the duplicate dependency validation at release time.
    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(is_match(r"(?i)duplicate").expect("valid regex"));
}

#[test]
fn release_invalid_version_field_path_fails_gracefully() {
    let workspace = create_workspace_with_invalid_version_field_path();
    init_git_repo(&workspace);

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

    git_add_and_commit(&workspace, "Initial commit");

    write_changeset(
        &workspace,
        "bump-a.md",
        "crate-a",
        "patch",
        "Fix in crate-a",
    );
    git_add_and_commit(&workspace, "Add changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(is_match(r"(?i)(not found|path|field)").expect("valid regex"));
}

#[test]
fn release_version_tracking_with_prerelease_versions() {
    let workspace = create_workspace_with_prerelease_dependency();
    init_git_repo(&workspace);

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

    git_add_and_commit(&workspace, "Initial commit");

    write_changeset(
        &workspace,
        "bump-a.md",
        "crate-a",
        "patch",
        "Fix in crate-a",
    );
    git_add_and_commit(&workspace, "Add changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .success();

    let rust_toml = fs::read_to_string(workspace.path().join("crates/crate-a/Cargo.toml"))
        .expect("failed to read crate-a Cargo.toml");
    // A patch bump on a prerelease version strips the prerelease tag:
    // 1.0.0-alpha.1 + patch -> 1.0.0
    assert!(
        rust_toml.contains("version = \"1.0.0\""),
        "expected crate-a version 1.0.0 (prerelease stripped), got:\n{rust_toml}"
    );

    let manifest_json = fs::read_to_string(workspace.path().join("packages/pkg-b/manifest.json"))
        .expect("failed to read manifest.json");
    assert!(
        manifest_json.contains("\"1.0.0\""),
        "expected tracking manifest upstreamVersion updated to 1.0.0, got:\n{manifest_json}"
    );
}

#[test]
fn release_version_tracking_deeply_nested_field_paths() {
    let workspace = create_workspace_with_deeply_nested_json_field();
    init_git_repo(&workspace);

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

    git_add_and_commit(&workspace, "Initial commit");

    write_changeset(
        &workspace,
        "bump-a.md",
        "crate-a",
        "patch",
        "Fix in crate-a",
    );
    git_add_and_commit(&workspace, "Add changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .success();

    let manifest_json = fs::read_to_string(workspace.path().join("packages/pkg-b/manifest.json"))
        .expect("failed to read manifest.json");
    let parsed: serde_json::Value =
        serde_json::from_str(&manifest_json).expect("manifest.json should be valid JSON");

    let upstream_version = parsed
        .get("metadata")
        .and_then(|m| m.get("versions"))
        .and_then(|v| v.get("upstream_crate"))
        .and_then(|v| v.as_str());
    assert_eq!(
        upstream_version,
        Some("1.0.1"),
        "expected deeply nested upstream_crate version updated to 1.0.1, got:\n{manifest_json}"
    );
}
