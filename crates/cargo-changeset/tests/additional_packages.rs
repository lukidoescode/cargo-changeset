mod common;

use std::fs;
use std::process::Command;

use predicates::str::contains;

use common::changesets::write_changeset;
use common::git::{git_add_and_commit, init_git_repo};
use common::workspaces::{
    add_helm_chart_config, create_single_crate_workspace, create_workspace_with_helm_chart,
    create_workspace_with_unknown_dependency,
    create_workspace_with_version_tracking_additional_to_cargo,
    create_workspace_with_version_tracking_cargo_to_additional,
};

mod add_tests {
    use super::*;

    #[test]
    fn add_helm_chart_via_cli_flags() {
        let dir = create_workspace_with_helm_chart();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "additional-packages",
                "add",
                "--name",
                "my-helm-chart",
                "--path",
                "charts/my-chart",
                "--influence",
                "charts/my-chart/**",
                "--manifest-file",
                "charts/my-chart/Chart.yaml",
                "--manifest-format",
                "yaml",
                "--version-field-path",
                "version",
            ])
            .current_dir(dir.path())
            .assert()
            .success()
            .stdout(contains("Added additional package 'my-helm-chart'"));

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains(r#"name = "my-helm-chart""#));
        assert!(cargo_toml.contains(r#"format = "yaml""#));
        assert!(cargo_toml.contains(r#"version-field-path = "version""#));
    }

    #[test]
    fn add_rejects_duplicate_name() {
        let dir = create_workspace_with_helm_chart();
        add_helm_chart_config(&dir);

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "additional-packages",
                "add",
                "--name",
                "my-helm-chart",
                "--path",
                "charts/my-chart",
                "--influence",
                "charts/my-chart/**",
                "--manifest-file",
                "charts/my-chart/Chart.yaml",
                "--manifest-format",
                "yaml",
                "--version-field-path",
                "version",
            ])
            .current_dir(dir.path())
            .assert()
            .failure()
            .stderr(contains("already exists"));
    }

    #[test]
    fn add_rejects_name_collision_with_rust_crate() {
        let dir = create_workspace_with_helm_chart();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "additional-packages",
                "add",
                "--name",
                "crate-a",
                "--path",
                "charts/my-chart",
                "--influence",
                "charts/my-chart/**",
                "--manifest-file",
                "charts/my-chart/Chart.yaml",
                "--manifest-format",
                "yaml",
                "--version-field-path",
                "version",
            ])
            .current_dir(dir.path())
            .assert()
            .failure()
            .stderr(contains("already exists"));
    }

    #[test]
    fn add_rejects_nonexistent_manifest_file() {
        let dir = create_workspace_with_helm_chart();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "additional-packages",
                "add",
                "--name",
                "my-helm-chart",
                "--path",
                "charts/my-chart",
                "--influence",
                "charts/my-chart/**",
                "--manifest-file",
                "nonexistent/Chart.yaml",
                "--manifest-format",
                "yaml",
                "--version-field-path",
                "version",
            ])
            .current_dir(dir.path())
            .assert()
            .failure();
    }

    #[test]
    fn add_rejects_invalid_glob_pattern() {
        let dir = create_workspace_with_helm_chart();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "additional-packages",
                "add",
                "--name",
                "my-helm-chart",
                "--path",
                "charts/my-chart",
                "--influence",
                "[invalid",
                "--manifest-file",
                "charts/my-chart/Chart.yaml",
                "--manifest-format",
                "yaml",
                "--version-field-path",
                "version",
            ])
            .current_dir(dir.path())
            .assert()
            .failure();
    }
}

mod remove_tests {
    use super::*;

    #[test]
    fn remove_helm_chart() {
        let dir = create_workspace_with_helm_chart();
        add_helm_chart_config(&dir);

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["additional-packages", "remove", "--name", "my-helm-chart"])
            .current_dir(dir.path())
            .assert()
            .success()
            .stdout(contains("Removed additional package 'my-helm-chart'"));

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(!cargo_toml.contains("my-helm-chart"));
    }

    #[test]
    fn remove_nonexistent_package() {
        let dir = create_workspace_with_helm_chart();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["additional-packages", "remove", "--name", "nonexistent"])
            .current_dir(dir.path())
            .assert()
            .failure()
            .stderr(contains("not found"));
    }
}

mod edit_tests {
    use super::*;

    #[test]
    fn edit_updates_path() {
        let dir = create_workspace_with_helm_chart();
        add_helm_chart_config(&dir);

        fs::create_dir_all(dir.path().join("charts/new-path"))
            .expect("failed to create charts/new-path dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "additional-packages",
                "edit",
                "--name",
                "my-helm-chart",
                "--path",
                "charts/new-path",
            ])
            .current_dir(dir.path())
            .assert()
            .success()
            .stdout(contains("Updated"));

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains(r#"path = "charts/new-path""#));
    }

    #[test]
    fn edit_nonexistent_package() {
        let dir = create_workspace_with_helm_chart();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "additional-packages",
                "edit",
                "--name",
                "nonexistent",
                "--manifest-format",
                "yaml",
            ])
            .current_dir(dir.path())
            .assert()
            .failure()
            .stderr(contains("not found"));
    }
}

mod list_tests {
    use super::*;

    #[test]
    fn list_shows_configured_packages() {
        let dir = create_workspace_with_helm_chart();
        add_helm_chart_config(&dir);

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["additional-packages", "list"])
            .current_dir(dir.path())
            .assert()
            .success()
            .stdout(contains("my-helm-chart"))
            .stdout(contains("charts/my-chart"))
            .stdout(contains("yaml"));
    }

    #[test]
    fn list_shows_empty_message_when_none_configured() {
        let dir = create_workspace_with_helm_chart();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["additional-packages", "list"])
            .current_dir(dir.path())
            .assert()
            .success()
            .stdout(contains("No additional packages configured"));
    }
}

mod workspace_rejection_tests {
    use super::*;

    #[test]
    fn add_rejects_single_package_project() {
        let dir = create_single_crate_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "additional-packages",
                "add",
                "--name",
                "x",
                "--path",
                ".",
                "--manifest-file",
                "Cargo.toml",
                "--manifest-format",
                "toml",
                "--version-field-path",
                "package.version",
                "--influence",
                "**",
            ])
            .current_dir(dir.path())
            .assert()
            .failure()
            .stderr(contains("workspace"));
    }

    #[test]
    fn remove_rejects_single_package_project() {
        let dir = create_single_crate_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["additional-packages", "remove", "--name", "x"])
            .current_dir(dir.path())
            .assert()
            .failure()
            .stderr(contains("workspace"));
    }

    #[test]
    fn list_rejects_single_package_project() {
        let dir = create_single_crate_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["additional-packages", "list"])
            .current_dir(dir.path())
            .assert()
            .failure()
            .stderr(contains("workspace"));
    }
}

mod version_tracking_tests {
    use super::*;

    #[test]
    fn release_with_version_tracking_dep_additional_to_cargo() {
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
            "Fix a bug in Rust crate",
        );
        git_add_and_commit(&workspace, "Add changeset");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("release")
            .current_dir(workspace.path())
            .assert()
            .success();

        let rust_toml =
            fs::read_to_string(workspace.path().join("crates/my-rust-crate/Cargo.toml"))
                .expect("failed to read my-rust-crate Cargo.toml");
        assert!(
            rust_toml.contains("version = \"1.0.1\""),
            "expected my-rust-crate version 1.0.1, got:\n{rust_toml}"
        );

        let chart_yaml = fs::read_to_string(workspace.path().join("charts/Chart.yaml"))
            .expect("failed to read Chart.yaml");
        assert!(
            chart_yaml.contains("version: 0.1.1"),
            "expected my-helm-chart auto-bumped to 0.1.1, got:\n{chart_yaml}"
        );
        assert!(
            chart_yaml.contains("appVersion: 1.0.1"),
            "expected appVersion updated to 1.0.1, got:\n{chart_yaml}"
        );
    }

    #[test]
    fn release_with_version_tracking_dep_cargo_to_additional() {
        let workspace = create_workspace_with_version_tracking_cargo_to_additional();
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
            "bump-lib.md",
            "my-lib",
            "patch",
            "Fix a bug in my-lib",
        );
        git_add_and_commit(&workspace, "Add changeset");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("release")
            .current_dir(workspace.path())
            .assert()
            .success();

        let package_json =
            fs::read_to_string(workspace.path().join("packages/my-lib/package.json"))
                .expect("failed to read package.json");
        assert!(
            package_json.contains("\"version\": \"2.0.1\"")
                || package_json.contains("\"version\":\"2.0.1\""),
            "expected my-lib version 2.0.1, got:\n{package_json}"
        );

        let rust_toml =
            fs::read_to_string(workspace.path().join("crates/my-rust-crate/Cargo.toml"))
                .expect("failed to read my-rust-crate Cargo.toml");
        assert!(
            rust_toml.contains("version = \"1.0.1\""),
            "expected my-rust-crate auto-bumped to 1.0.1, got:\n{rust_toml}"
        );

        let upstream_json = fs::read_to_string(
            workspace
                .path()
                .join("crates/my-rust-crate/src/upstream_version.json"),
        )
        .expect("failed to read upstream_version.json");
        assert!(
            upstream_json.contains("\"version\": \"2.0.1\"")
                || upstream_json.contains("\"version\":\"2.0.1\""),
            "expected upstream_version.json version field updated to 2.0.1, got:\n{upstream_json}"
        );
    }

    #[test]
    fn validate_unknown_dependency_errors() {
        let workspace = create_workspace_with_unknown_dependency();
        init_git_repo(&workspace);
        git_add_and_commit(&workspace, "Initial commit");

        write_changeset(
            &workspace,
            "some-change.md",
            "crate-a",
            "patch",
            "Some change",
        );
        git_add_and_commit(&workspace, "Add changeset");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("release")
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("unknown-pkg"));
    }
}

mod dependencies_cli_tests {
    use super::*;

    #[test]
    fn dependencies_cli_add_list_remove() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

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

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "additional-packages",
                "dependencies",
                "list",
                "--package",
                "my-helm-chart",
            ])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("crate-a"));

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "additional-packages",
                "dependencies",
                "remove",
                "--package",
                "my-helm-chart",
                "--dependency",
                "crate-a",
            ])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Removed version-tracking dependency"));

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            !cargo_toml.contains("crate-a"),
            "expected crate-a dependency to be removed from Cargo.toml, got:\n{cargo_toml}"
        );
    }
}
