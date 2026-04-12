mod common;

use std::fs;

use predicates::str::contains;

use common::workspaces::{
    add_helm_chart_config, create_single_crate_workspace, create_workspace_with_helm_chart,
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
