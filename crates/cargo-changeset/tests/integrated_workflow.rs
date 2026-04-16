mod common;

use std::fs;
use std::process::Command;

use predicates::str::contains;

use common::changesets::write_changeset;
use common::git::{create_branch, git_add_and_commit, init_git_repo};
use common::workspaces::{add_helm_chart_config, create_workspace_with_helm_chart};

mod additional_packages {
    use super::*;

    #[test]
    fn helm_chart_depending_on_crate_add_verify_changeset_status_release() {
        // Step 1: Set up workspace with git and lockfile
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);
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

        // Step 2: Create feature branch
        create_branch(&workspace, "feature");

        // Step 3: Assert crate-a starts at version 1.0.0
        let crate_a_toml = fs::read_to_string(workspace.path().join("crates/crate-a/Cargo.toml"))
            .expect("failed to read crate-a Cargo.toml");
        assert!(
            crate_a_toml.contains("version = \"1.0.0\""),
            "expected crate-a version 1.0.0, got:\n{crate_a_toml}"
        );

        // Step 4: Assert Chart.yaml starts at version 2.0.0 with appVersion 1.0.0
        let chart_yaml = fs::read_to_string(workspace.path().join("charts/my-chart/Chart.yaml"))
            .expect("failed to read Chart.yaml");
        assert!(
            chart_yaml.contains("version: \"2.0.0\""),
            "expected Chart.yaml version 2.0.0, got:\n{chart_yaml}"
        );
        assert!(
            chart_yaml.contains("appVersion: \"1.0.0\""),
            "expected Chart.yaml appVersion 1.0.0, got:\n{chart_yaml}"
        );

        // Step 5: Add version-tracking dependency from my-helm-chart to crate-a
        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "additional-packages",
                "dependencies",
                "add",
                "--package",
                "my-helm-chart",
                "--dependency",
                "crate-a",
                "--manifest-file",
                "charts/my-chart/Chart.yaml",
                "--manifest-format",
                "yaml",
                "--version-field-path",
                "appVersion",
            ])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Added version-tracking dependency"));

        // Step 6: Assert workspace Cargo.toml now contains the dependency config
        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml.contains("dependency-name = \"crate-a\""),
            "expected dependency-name = \"crate-a\" in Cargo.toml, got:\n{cargo_toml}"
        );
        assert!(
            cargo_toml.contains("version-field-path = \"appVersion\""),
            "expected version-field-path = \"appVersion\" in Cargo.toml, got:\n{cargo_toml}"
        );

        // Step 7: Assert crate-a Cargo.toml unchanged
        let crate_a_toml = fs::read_to_string(workspace.path().join("crates/crate-a/Cargo.toml"))
            .expect("failed to read crate-a Cargo.toml");
        assert!(
            crate_a_toml.contains("version = \"1.0.0\""),
            "expected crate-a still at 1.0.0, got:\n{crate_a_toml}"
        );

        // Step 8: Assert Chart.yaml unchanged
        let chart_yaml = fs::read_to_string(workspace.path().join("charts/my-chart/Chart.yaml"))
            .expect("failed to read Chart.yaml");
        assert!(
            chart_yaml.contains("version: \"2.0.0\""),
            "expected Chart.yaml still at 2.0.0, got:\n{chart_yaml}"
        );
        assert!(
            chart_yaml.contains("appVersion: \"1.0.0\""),
            "expected Chart.yaml appVersion still 1.0.0, got:\n{chart_yaml}"
        );

        // Step 9: Modify crate-a source
        fs::write(
            workspace.path().join("crates/crate-a/src/lib.rs"),
            "pub fn new_feature() {}\n",
        )
        .expect("failed to write lib.rs");

        // Step 10: Commit the change
        git_add_and_commit(&workspace, "Add feature");

        // Step 11: Verify fails (no changeset coverage)
        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["verify", "--base", "main"])
            .current_dir(workspace.path())
            .assert()
            .failure();

        // Step 12: Assert all manifests unchanged after verify
        let crate_a_toml = fs::read_to_string(workspace.path().join("crates/crate-a/Cargo.toml"))
            .expect("failed to read crate-a Cargo.toml");
        assert!(
            crate_a_toml.contains("version = \"1.0.0\""),
            "verify should not change crate-a version, got:\n{crate_a_toml}"
        );
        let chart_yaml = fs::read_to_string(workspace.path().join("charts/my-chart/Chart.yaml"))
            .expect("failed to read Chart.yaml");
        assert!(
            chart_yaml.contains("version: \"2.0.0\""),
            "verify should not change Chart.yaml version, got:\n{chart_yaml}"
        );

        // Step 13: Add a changeset for crate-a
        write_changeset(
            &workspace,
            "bump-crate-a.md",
            "crate-a",
            "patch",
            "Add new feature",
        );

        // Step 14: Commit the changeset
        git_add_and_commit(&workspace, "Add changeset");

        // Step 15: Verify now passes
        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["verify", "--base", "main"])
            .current_dir(workspace.path())
            .assert()
            .success();

        // Step 16: Assert changeset file exists
        assert!(
            workspace
                .path()
                .join(".changeset/changesets/bump-crate-a.md")
                .exists(),
            "changeset file should exist before release"
        );

        // Step 17: Assert all manifests still unchanged
        let crate_a_toml = fs::read_to_string(workspace.path().join("crates/crate-a/Cargo.toml"))
            .expect("failed to read crate-a Cargo.toml");
        assert!(
            crate_a_toml.contains("version = \"1.0.0\""),
            "manifests should not change before release, got:\n{crate_a_toml}"
        );
        let chart_yaml = fs::read_to_string(workspace.path().join("charts/my-chart/Chart.yaml"))
            .expect("failed to read Chart.yaml");
        assert!(
            chart_yaml.contains("version: \"2.0.0\""),
            "Chart.yaml should not change before release, got:\n{chart_yaml}"
        );

        // Step 18: Status shows projected versions
        let status_output = assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("status")
            .current_dir(workspace.path())
            .assert()
            .success();

        let status_stdout = String::from_utf8_lossy(&status_output.get_output().stdout);
        assert!(
            status_stdout.contains("crate-a: 1.0.0 -> 1.0.1"),
            "expected crate-a projected release in status output, got:\n{status_stdout}"
        );
        assert!(
            status_stdout.contains("my-helm-chart"),
            "expected my-helm-chart in status output, got:\n{status_stdout}"
        );

        // Step 19: Release
        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("release")
            .current_dir(workspace.path())
            .assert()
            .success();

        // Step 20: Assert crate-a bumped to 1.0.1
        let crate_a_toml = fs::read_to_string(workspace.path().join("crates/crate-a/Cargo.toml"))
            .expect("failed to read crate-a Cargo.toml after release");
        assert!(
            crate_a_toml.contains("version = \"1.0.1\""),
            "expected crate-a version 1.0.1 after release, got:\n{crate_a_toml}"
        );

        // Step 21: Assert Chart.yaml auto-patched and appVersion tracked
        let chart_yaml = fs::read_to_string(workspace.path().join("charts/my-chart/Chart.yaml"))
            .expect("failed to read Chart.yaml after release");
        assert!(
            chart_yaml.contains(r"version: 2.0.1"),
            "expected Chart.yaml auto-patched to 2.0.1, got:\n{chart_yaml}"
        );
        assert!(
            chart_yaml.contains(r"appVersion: 1.0.1"),
            "expected appVersion tracked to 1.0.1, got:\n{chart_yaml}"
        );

        // Step 22: Assert changeset file consumed
        assert!(
            !workspace
                .path()
                .join(".changeset/changesets/bump-crate-a.md")
                .exists(),
            "changeset file should be consumed after release"
        );
    }
}
