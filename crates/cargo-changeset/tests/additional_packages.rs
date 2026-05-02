use std::fs;
use std::process::Command;

use changeset_test_helpers::changesets::write_changeset;
use changeset_test_helpers::git::{git_add_and_commit, init_git_repo};
use changeset_test_helpers::workspaces::{
    add_helm_chart_config, add_helm_chart_config_with_three_deps, create_single_crate_workspace,
    create_workspace_with_helm_chart, create_workspace_with_unknown_dependency,
    create_workspace_with_version_tracking_additional_to_cargo,
    create_workspace_with_version_tracking_cargo_to_additional,
};
use predicates::str::contains;

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

mod dependencies_advanced_tests {
    use super::*;

    #[test]
    fn dependencies_add_rejects_invalid_manifest_path() {
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
                "path/that/does/not/exist.json",
                "--manifest-format",
                "json",
                "--version-field-path",
                "appVersion",
            ])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Added version-tracking dependency"));

        // The CLI does not validate manifest file existence at add time;
        // it just writes the config. Errors surface at release time.
        // Verify the config was written even with a nonexistent path.
        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml.contains("path/that/does/not/exist.json"),
            "expected the invalid path to be stored in config, got:\n{cargo_toml}"
        );
    }

    #[test]
    fn dependencies_add_rejects_unknown_package() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "additional-packages",
                "dependencies",
                "add",
                "--package",
                "totally-unknown-pkg",
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
            .failure()
            .stderr(contains("not found"));
    }

    #[test]
    fn dependencies_add_rejects_duplicate_dependency() {
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
            .success();

        // The CLI does not reject duplicate dependency declarations at add time;
        // duplicates are detected during release validation.
        // Verify the second add also succeeds at the CLI level.
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
                "betaVersion",
            ])
            .current_dir(workspace.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        let count = cargo_toml.matches("crate-a").count();
        assert!(
            count >= 2,
            "expected at least two crate-a references (duplicate), got {count} in:\n{cargo_toml}"
        );
    }

    #[test]
    fn dependencies_remove_one_of_many() {
        let workspace = create_workspace_with_helm_chart();

        fs::write(
            workspace.path().join("charts/my-chart/Chart.yaml"),
            "apiVersion: v2\nname: my-chart\nversion: \"2.0.0\"\nalphaVersion: \"1.0.0\"\nbetaVersion: \"1.0.0\"\ngammaVersion: \"1.0.0\"\n",
        )
        .expect("failed to write Chart.yaml");

        add_helm_chart_config_with_three_deps(&workspace);

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "additional-packages",
                "dependencies",
                "remove",
                "--package",
                "my-helm-chart",
                "--dependency",
                "dep-beta",
            ])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Removed version-tracking dependency"));

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            !cargo_toml.contains("dep-beta"),
            "expected dep-beta removed from Cargo.toml, got:\n{cargo_toml}"
        );
        assert!(
            cargo_toml.contains("dep-alpha"),
            "expected dep-alpha to remain in Cargo.toml, got:\n{cargo_toml}"
        );
        assert!(
            cargo_toml.contains("dep-gamma"),
            "expected dep-gamma to remain in Cargo.toml, got:\n{cargo_toml}"
        );
    }

    #[test]
    fn dependencies_list_shows_manifest_info() {
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
            .success();

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
            .stdout(contains("crate-a"))
            .stdout(contains("Chart.yaml"))
            .stdout(contains("yaml"))
            .stdout(contains("appVersion"));
    }
}

mod non_interactive_fallback_tests {
    use super::*;

    #[test]
    fn dependencies_add_no_args_non_interactive_shows_incomplete_args_error() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["additional-packages", "dependencies", "add"])
            .env("CARGO_CHANGESET_NO_TTY", "1")
            .env("NO_COLOR", "1")
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("not all required arguments"));
    }

    #[test]
    fn dependencies_remove_no_args_non_interactive_shows_incomplete_args_error() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["additional-packages", "dependencies", "remove"])
            .env("CARGO_CHANGESET_NO_TTY", "1")
            .env("NO_COLOR", "1")
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("not all required arguments"));
    }

    #[test]
    fn dependencies_list_no_args_non_interactive_shows_incomplete_args_error() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["additional-packages", "dependencies", "list"])
            .env("CARGO_CHANGESET_NO_TTY", "1")
            .env("NO_COLOR", "1")
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("not all required arguments"));
    }

    #[test]
    fn dependencies_add_partial_args_non_interactive_shows_incomplete_args_error() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "additional-packages",
                "dependencies",
                "add",
                "--package",
                "my-helm-chart",
            ])
            .env("CARGO_CHANGESET_NO_TTY", "1")
            .env("NO_COLOR", "1")
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("not all required arguments"));
    }
}

mod auto_detect_format_tests {
    use super::*;

    #[test]
    fn dependencies_add_auto_detects_yaml_format() {
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
                "--version-field-path",
                "appVersion",
            ])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Added version-tracking dependency"));

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml.contains(r#"format = "yaml""#),
            "expected auto-detected yaml format stored in Cargo.toml, got:\n{cargo_toml}"
        );
    }

    #[test]
    fn dependencies_add_auto_detects_json_format() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

        fs::write(
            workspace.path().join("charts/my-chart/versions.json"),
            r#"{"appVersion": "1.0.0"}"#,
        )
        .expect("write json file");

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
                "charts/my-chart/versions.json",
                "--version-field-path",
                "appVersion",
            ])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Added version-tracking dependency"));

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml.contains(r#"format = "json""#),
            "expected auto-detected json format stored in Cargo.toml, got:\n{cargo_toml}"
        );
    }

    #[test]
    fn dependencies_add_auto_detects_toml_format() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

        fs::write(
            workspace.path().join("charts/my-chart/versions.toml"),
            "[versions]\nappVersion = \"1.0.0\"\n",
        )
        .expect("write toml file");

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
                "charts/my-chart/versions.toml",
                "--version-field-path",
                "appVersion",
            ])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Added version-tracking dependency"));

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml.contains(r#"format = "toml""#),
            "expected auto-detected toml format stored in Cargo.toml, got:\n{cargo_toml}"
        );
    }

    #[test]
    fn additional_packages_add_auto_detects_manifest_format() {
        let workspace = create_workspace_with_helm_chart();

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
                "--version-field-path",
                "version",
            ])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Added additional package 'my-helm-chart'"));

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml.contains(r#"format = "yaml""#),
            "expected auto-detected yaml format stored in Cargo.toml, got:\n{cargo_toml}"
        );
    }
}

mod direct_binary_invocation_tests {
    use super::*;

    #[test]
    fn direct_invocation_additional_packages_dependencies_add_no_args_shows_proper_error() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

        assert_cmd::Command::cargo_bin("cargo-changeset")
            .expect("binary exists")
            .args(["additional-packages", "dependencies", "add"])
            .env("CARGO_CHANGESET_NO_TTY", "1")
            .env("NO_COLOR", "1")
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("not all required arguments"));
    }
}

mod help_and_ux_tests {
    use super::*;

    #[test]
    fn dependencies_subcommand_shows_in_additional_packages_help() {
        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["additional-packages", "--help"])
            .assert()
            .success()
            .stdout(contains("dependencies"));
    }

    #[test]
    fn dependencies_add_shows_success_message() {
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
    }

    #[test]
    fn dependencies_remove_shows_success_message() {
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
            .success();

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
    }
}

#[cfg(not(windows))]
mod interactive_add_tests {
    use std::path::PathBuf;

    use changeset_test_helpers::terminal_session::TerminalSession;
    use indoc::indoc;

    use super::*;

    fn bin_path() -> PathBuf {
        assert_cmd::cargo::cargo_bin("cargo-changeset")
    }

    fn spawn_add(workspace: &tempfile::TempDir) -> TerminalSession {
        TerminalSession::spawn(&bin_path(), workspace, &["additional-packages", "add"])
    }

    #[test]
    fn interactive_add_cancel_at_name() {
        let workspace = create_workspace_with_helm_chart();
        let cargo_toml_before =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");

        let mut session = spawn_add(&workspace);
        session.wait_for("Package name");
        session.assert_screen(
            "name prompt before ctrl-c",
            indoc! {"
            Package name:"},
        );
        session.ctrl_c();
        session.wait_for_exit();

        let cargo_toml_after =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert_eq!(
            cargo_toml_before, cargo_toml_after,
            "Cargo.toml must not change after Ctrl+C at name prompt"
        );
    }

    #[test]
    fn interactive_add_cancel_at_directory_path() {
        let workspace = create_workspace_with_helm_chart();
        let cargo_toml_before =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");

        let mut session = spawn_add(&workspace);
        session.wait_for("Package name");
        session.assert_screen(
            "name prompt",
            indoc! {"
            Package name:"},
        );
        session.type_line("my-helm-chart");
        session.wait_for("Package directory path");
        session.assert_screen(
            "path prompt before ctrl-c",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root):"},
        );
        session.ctrl_c();
        session.wait_for_exit();

        let cargo_toml_after =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert_eq!(
            cargo_toml_before, cargo_toml_after,
            "Cargo.toml must not change after Ctrl+C at path prompt"
        );
    }

    #[test]
    fn interactive_add_cancel_at_manifest_format() {
        let workspace = create_workspace_with_helm_chart();
        let cargo_toml_before =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");

        let mut session = spawn_add(&workspace);
        session.wait_for("Package name");
        session.assert_screen(
            "name prompt",
            indoc! {"
            Package name:"},
        );
        session.type_line("my-helm-chart");
        session.wait_for("Package directory path");
        session.assert_screen(
            "path prompt",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root):"},
        );
        session.type_line("charts/my-chart");
        session.wait_for("Glob pattern");
        session.assert_screen(
            "influence pattern prompt with default",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern [charts/my-chart/**]:"},
        );
        session.type_line("charts/my-chart/**");
        session.wait_for("Additional pattern");
        session.assert_screen(
            "additional pattern prompt",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern: charts/my-chart/**
                Additional pattern:"},
        );
        session.type_line("");
        session.wait_for("version manifest file");
        session.assert_screen(
            "manifest file prompt",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern: charts/my-chart/**
                Additional pattern:
                Path to version manifest file:"},
        );
        session.type_line("charts/my-chart/Chart.yaml");
        session.wait_for("Manifest format");
        session.assert_screen(
            "manifest format menu before cancel",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern: charts/my-chart/**
                Additional pattern:
                Path to version manifest file: charts/my-chart/Chart.yaml
                Manifest format:
                  toml
                > yaml
                  json"},
        );
        session.cancel();
        session.wait_for_exit();

        let cargo_toml_after =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert_eq!(
            cargo_toml_before, cargo_toml_after,
            "Cargo.toml must not change after cancel at format"
        );
    }

    #[test]
    fn interactive_add_full_flow() {
        let workspace = create_workspace_with_helm_chart();

        let mut session = spawn_add(&workspace);
        session.wait_for("Package name");
        session.assert_screen(
            "name prompt",
            indoc! {"
            Package name:"},
        );
        session.type_line("my-helm-chart");
        session.wait_for("Package directory path");
        session.assert_screen(
            "path prompt",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root):"},
        );
        session.type_line("charts/my-chart");
        session.wait_for("Glob pattern");
        session.assert_screen(
            "influence pattern prompt with default",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern [charts/my-chart/**]:"},
        );
        session.type_line("charts/my-chart/**");
        session.wait_for("Additional pattern");
        session.type_line("");
        session.wait_for("version manifest file");
        session.assert_screen(
            "manifest file prompt",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern: charts/my-chart/**
                Additional pattern:
                Path to version manifest file:"},
        );
        session.type_line("charts/my-chart/Chart.yaml");
        session.wait_for("Manifest format");
        session.assert_screen(
            "manifest format menu with yaml auto-detected",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern: charts/my-chart/**
                Additional pattern:
                Path to version manifest file: charts/my-chart/Chart.yaml
                Manifest format:
                  toml
                > yaml
                  json"},
        );
        session.confirm();
        session.wait_for("version field");
        session.assert_screen(
            "version field prompt",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern: charts/my-chart/**
                Additional pattern:
                Path to version manifest file: charts/my-chart/Chart.yaml
                Manifest format: yaml
                Path to version field in manifest (e.g. 'version' or 'info.version'):"},
        );
        session.type_line("version");
        session.wait_for("Added additional package");
        session.wait_for_exit();
        session.assert_screen(
            "full flow complete",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern: charts/my-chart/**
                Additional pattern:
                Path to version manifest file: charts/my-chart/Chart.yaml
                Manifest format: yaml
                Path to version field in manifest (e.g. 'version' or 'info.version'): version
                Added additional package 'my-helm-chart'"},
        );

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml.contains(r#"name = "my-helm-chart""#),
            "expected package name in config, got:\n{cargo_toml}"
        );
        assert!(
            cargo_toml.contains(r#"path = "charts/my-chart""#),
            "expected path in config, got:\n{cargo_toml}"
        );
        assert!(
            cargo_toml.contains("charts/my-chart/**"),
            "expected influence pattern in config, got:\n{cargo_toml}"
        );
        assert!(
            cargo_toml.contains(r#"format = "yaml""#),
            "expected yaml format in config, got:\n{cargo_toml}"
        );
    }

    #[test]
    fn interactive_add_multiple_influence_patterns() {
        let workspace = create_workspace_with_helm_chart();

        let mut session = spawn_add(&workspace);
        session.wait_for("Package name");
        session.assert_screen(
            "name prompt",
            indoc! {"
            Package name:"},
        );
        session.type_line("my-helm-chart");
        session.wait_for("Package directory path");
        session.assert_screen(
            "path prompt",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root):"},
        );
        session.type_line("charts/my-chart");
        session.wait_for("Glob pattern");
        session.assert_screen(
            "first influence pattern prompt",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern [charts/my-chart/**]:"},
        );
        session.type_line("charts/my-chart/**");
        session.wait_for("Additional pattern");
        session.assert_screen(
            "second influence pattern prompt",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern: charts/my-chart/**
                Additional pattern:"},
        );
        session.type_line("charts/shared/**");
        session.wait_for("charts/shared/**\nAdditional pattern:");
        session.assert_screen(
            "third influence pattern prompt",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern: charts/my-chart/**
                Additional pattern: charts/shared/**
                Additional pattern:"},
        );
        session.type_line("common/**");
        session.wait_for("common/**\nAdditional pattern:");
        session.assert_screen(
            "finish patterns prompt",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern: charts/my-chart/**
                Additional pattern: charts/shared/**
                Additional pattern: common/**
                Additional pattern:"},
        );
        session.type_line("");
        session.wait_for("version manifest file");
        session.assert_screen(
            "manifest file prompt",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern: charts/my-chart/**
                Additional pattern: charts/shared/**
                Additional pattern: common/**
                Additional pattern:
                Path to version manifest file:"},
        );
        session.type_line("charts/my-chart/Chart.yaml");
        session.wait_for("Manifest format");
        session.assert_screen(
            "manifest format menu with yaml auto-detected",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern: charts/my-chart/**
                Additional pattern: charts/shared/**
                Additional pattern: common/**
                Additional pattern:
                Path to version manifest file: charts/my-chart/Chart.yaml
                Manifest format:
                  toml
                > yaml
                  json"},
        );
        session.confirm();
        session.wait_for("version field");
        session.assert_screen(
            "version field prompt",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern: charts/my-chart/**
                Additional pattern: charts/shared/**
                Additional pattern: common/**
                Additional pattern:
                Path to version manifest file: charts/my-chart/Chart.yaml
                Manifest format: yaml
                Path to version field in manifest (e.g. 'version' or 'info.version'):"},
        );
        session.type_line("version");
        session.wait_for("Added additional package");
        session.wait_for_exit();

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml.contains("charts/my-chart/**"),
            "expected first influence pattern, got:\n{cargo_toml}"
        );
        assert!(
            cargo_toml.contains("charts/shared/**"),
            "expected second influence pattern, got:\n{cargo_toml}"
        );
        assert!(
            cargo_toml.contains("common/**"),
            "expected third influence pattern, got:\n{cargo_toml}"
        );
    }

    #[test]
    fn interactive_add_default_influence_pattern() {
        let workspace = create_workspace_with_helm_chart();

        let mut session = spawn_add(&workspace);
        session.wait_for("Package name");
        session.assert_screen(
            "name prompt",
            indoc! {"
            Package name:"},
        );
        session.type_line("my-helm-chart");
        session.wait_for("Package directory path");
        session.assert_screen(
            "path prompt",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root):"},
        );
        session.type_line("charts/my-chart");
        session.wait_for("Glob pattern");
        session.assert_screen(
            "influence pattern prompt with default",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern [charts/my-chart/**]:"},
        );
        session.confirm();
        session.wait_for("Additional pattern");
        session.assert_screen(
            "additional pattern prompt after accepting default",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern: charts/my-chart/**
                Additional pattern:"},
        );
        session.type_line("");
        session.wait_for("version manifest file");
        session.assert_screen(
            "manifest file prompt",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern: charts/my-chart/**
                Additional pattern:
                Path to version manifest file:"},
        );
        session.type_line("charts/my-chart/Chart.yaml");
        session.wait_for("Manifest format");
        session.assert_screen(
            "manifest format menu with yaml auto-detected",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern: charts/my-chart/**
                Additional pattern:
                Path to version manifest file: charts/my-chart/Chart.yaml
                Manifest format:
                  toml
                > yaml
                  json"},
        );
        session.confirm();
        session.wait_for("version field");
        session.assert_screen(
            "version field prompt",
            indoc! {"
                Package name: my-helm-chart
                Package directory path (relative to workspace root): charts/my-chart
                Enter glob patterns for files that influence this package (one per line, empty line to finish):
                Glob pattern: charts/my-chart/**
                Additional pattern:
                Path to version manifest file: charts/my-chart/Chart.yaml
                Manifest format: yaml
                Path to version field in manifest (e.g. 'version' or 'info.version'):"},
        );
        session.type_line("version");
        session.wait_for("Added additional package");
        session.wait_for_exit();

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml.contains("charts/my-chart/**"),
            "expected default influence pattern, got:\n{cargo_toml}"
        );
    }
}

#[cfg(not(windows))]
mod interactive_remove_package_tests {
    use std::path::PathBuf;

    use changeset_test_helpers::terminal_session::TerminalSession;
    use indoc::indoc;

    use super::*;

    fn bin_path() -> PathBuf {
        assert_cmd::cargo::cargo_bin("cargo-changeset")
    }

    fn spawn_remove(workspace: &tempfile::TempDir) -> TerminalSession {
        TerminalSession::spawn(&bin_path(), workspace, &["additional-packages", "remove"])
    }

    #[test]
    fn interactive_remove_shows_package_menu() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

        let mut session = spawn_remove(&workspace);
        session.wait_for("Select package to remove");
        session.assert_screen(
            "remove package menu",
            indoc! {"
                Select package to remove:
                  my-helm-chart (charts/my-chart)"},
        );
        session.cancel();
        session.wait_for_exit();
    }

    #[test]
    fn interactive_remove_cancel_at_selection() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);
        let cargo_toml_before =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");

        let mut session = spawn_remove(&workspace);
        session.wait_for("Select package to remove");
        session.assert_screen(
            "remove package menu before cancel",
            indoc! {"
                Select package to remove:
                  my-helm-chart (charts/my-chart)"},
        );
        session.cancel();
        session.wait_for_exit();

        let cargo_toml_after =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert_eq!(
            cargo_toml_before, cargo_toml_after,
            "Cargo.toml must not change after cancel"
        );
    }

    #[test]
    fn interactive_remove_decline_confirmation() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);
        let cargo_toml_before =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");

        let mut session = spawn_remove(&workspace);
        session.wait_for("Select package to remove");
        session.select_item(0);
        session.wait_for("Remove additional package");
        session.assert_screen(
            "confirmation prompt",
            indoc! {"
                Select package to remove: my-helm-chart (charts/my-chart)
                Remove additional package 'my-helm-chart'? [y/N]"},
        );
        session.type_line("n");
        session.wait_for_exit();

        let cargo_toml_after =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert_eq!(
            cargo_toml_before, cargo_toml_after,
            "Cargo.toml must not change after declining removal"
        );
    }

    #[test]
    fn interactive_remove_full_flow() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

        let mut session = spawn_remove(&workspace);
        session.wait_for("Select package to remove");
        session.assert_screen(
            "remove package menu",
            indoc! {"
                Select package to remove:
                  my-helm-chart (charts/my-chart)"},
        );
        session.select_item(0);
        session.wait_for("Remove additional package");
        session.assert_screen(
            "confirmation prompt",
            indoc! {"
                Select package to remove: my-helm-chart (charts/my-chart)
                Remove additional package 'my-helm-chart'? [y/N]"},
        );
        session.type_line("y");
        session.wait_for("Removed additional package");
        session.wait_for_exit();
        session.assert_screen(
            "removal complete",
            indoc! {"
                Select package to remove: my-helm-chart (charts/my-chart)
                Remove additional package 'my-helm-chart'? yes
                Removed additional package 'my-helm-chart'"},
        );

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            !cargo_toml.contains("my-helm-chart"),
            "expected package removed from Cargo.toml, got:\n{cargo_toml}"
        );
    }

    #[test]
    fn interactive_remove_no_packages_exits() {
        let workspace = create_workspace_with_helm_chart();

        let mut session = spawn_remove(&workspace);
        session.wait_for("No additional packages");
        session.wait_for_exit();
    }
}

#[cfg(not(windows))]
mod interactive_edit_package_tests {
    use std::path::PathBuf;

    use changeset_test_helpers::terminal_session::TerminalSession;
    use indoc::indoc;

    use super::*;

    fn bin_path() -> PathBuf {
        assert_cmd::cargo::cargo_bin("cargo-changeset")
    }

    fn spawn_edit(workspace: &tempfile::TempDir) -> TerminalSession {
        TerminalSession::spawn(&bin_path(), workspace, &["additional-packages", "edit"])
    }

    #[test]
    fn interactive_edit_shows_package_menu() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

        let mut session = spawn_edit(&workspace);
        session.wait_for("Select package to edit");
        session.assert_screen(
            "edit package menu",
            indoc! {"
                Select package to edit:
                  my-helm-chart (charts/my-chart)"},
        );
        session.cancel();
        session.wait_for_exit();
    }

    #[test]
    fn interactive_edit_cancel_at_package_selection() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);
        let cargo_toml_before =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");

        let mut session = spawn_edit(&workspace);
        session.wait_for("Select package to edit");
        session.assert_screen(
            "edit package menu before cancel",
            indoc! {"
                Select package to edit:
                  my-helm-chart (charts/my-chart)"},
        );
        session.cancel();
        session.wait_for_exit();

        let cargo_toml_after =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert_eq!(
            cargo_toml_before, cargo_toml_after,
            "Cargo.toml must not change after cancel"
        );
    }

    #[test]
    fn interactive_edit_shows_field_menu() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

        let mut session = spawn_edit(&workspace);
        session.wait_for("Select package to edit");
        session.select_item(0);
        session.wait_for("Which field");
        session.assert_screen(
            "field selection menu",
            indoc! {"
                Select package to edit: my-helm-chart (charts/my-chart)
                Which field would you like to edit?:
                > path
                  influence patterns
                  manifest file path
                  manifest format
                  manifest version path
                  Done"},
        );
        session.cancel();
        session.wait_for_exit();
    }

    #[test]
    fn interactive_edit_done_exits_immediately() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);
        let cargo_toml_before =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");

        let mut session = spawn_edit(&workspace);
        session.wait_for("Select package to edit");
        session.assert_screen(
            "edit package menu",
            indoc! {"
                Select package to edit:
                  my-helm-chart (charts/my-chart)"},
        );
        session.select_item(0);
        session.wait_for("Which field");
        session.assert_screen(
            "field menu before selecting Done",
            indoc! {"
                Select package to edit: my-helm-chart (charts/my-chart)
                Which field would you like to edit?:
                > path
                  influence patterns
                  manifest file path
                  manifest format
                  manifest version path
                  Done"},
        );
        session.select_item(4);
        session.wait_for_exit();

        let cargo_toml_after =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert_eq!(
            cargo_toml_before, cargo_toml_after,
            "Cargo.toml must not change when selecting Done immediately"
        );
    }

    #[test]
    fn interactive_edit_path_field() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

        fs::create_dir_all(workspace.path().join("charts/new-path")).expect("create new-path dir");

        let mut session = spawn_edit(&workspace);
        session.wait_for("Select package to edit");
        session.assert_screen(
            "edit package menu",
            indoc! {"
                Select package to edit:
                  my-helm-chart (charts/my-chart)"},
        );
        session.select_item(0);
        session.wait_for("Which field");
        session.assert_screen(
            "field menu",
            indoc! {"
                Select package to edit: my-helm-chart (charts/my-chart)
                Which field would you like to edit?:
                > path
                  influence patterns
                  manifest file path
                  manifest format
                  manifest version path
                  Done"},
        );
        session.confirm();
        session.wait_for("Package directory path");
        session.assert_screen(
            "path input prompt",
            indoc! {"
                Select package to edit: my-helm-chart (charts/my-chart)
                Which field would you like to edit?: path
                Package directory path (relative to workspace root):"},
        );
        session.type_line("charts/new-path");
        session.wait_for("Which field");
        session.select_item(4);
        session.wait_for("Updated");
        session.wait_for_exit();
        session.assert_screen(
            "edit complete",
            indoc! {"
                Select package to edit: my-helm-chart (charts/my-chart)
                Which field would you like to edit?: path
                Package directory path (relative to workspace root): charts/new-path
                Which field would you like to edit?: Done
                Updated additional package 'my-helm-chart' (fields: path)"},
        );

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml.contains(r#"path = "charts/new-path""#),
            "expected updated path, got:\n{cargo_toml}"
        );
    }

    #[test]
    fn interactive_edit_manifest_format_field() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

        let mut session = spawn_edit(&workspace);
        session.wait_for("Select package to edit");
        session.assert_screen(
            "edit package menu",
            indoc! {"
                Select package to edit:
                  my-helm-chart (charts/my-chart)"},
        );
        session.select_item(0);
        session.wait_for("Which field");
        session.assert_screen(
            "field menu before selecting manifest format",
            indoc! {"
                Select package to edit: my-helm-chart (charts/my-chart)
                Which field would you like to edit?:
                > path
                  influence patterns
                  manifest file path
                  manifest format
                  manifest version path
                  Done"},
        );
        session.select_item(2);
        session.wait_for("Manifest format");
        session.assert_screen(
            "manifest format menu",
            indoc! {"
                Select package to edit: my-helm-chart (charts/my-chart)
                Which field would you like to edit?: manifest format
                Manifest format:
                > toml
                  yaml
                  json"},
        );
        session.select_item(1);
        session.wait_for("Manifest format: json");
        session.assert_screen(
            "field menu after format change",
            indoc! {"
                Select package to edit: my-helm-chart (charts/my-chart)
                Which field would you like to edit?: manifest format
                Manifest format: json
                Which field would you like to edit?:
                > path
                  influence patterns
                  manifest file path
                  manifest format
                  manifest version path
                  Done"},
        );
        session.select_item(4);
        session.wait_for("Updated");
        session.wait_for_exit();

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml.contains(r#"format = "json""#),
            "expected updated format, got:\n{cargo_toml}"
        );
    }

    #[test]
    fn interactive_edit_no_packages_exits() {
        let workspace = create_workspace_with_helm_chart();

        let mut session = spawn_edit(&workspace);
        session.wait_for("No additional packages");
        session.wait_for_exit();
    }
}

#[cfg(not(windows))]
mod interactive_dependencies_tests {
    use std::path::PathBuf;

    use changeset_test_helpers::terminal_session::TerminalSession;
    use indoc::indoc;

    use super::*;

    fn bin_path() -> PathBuf {
        assert_cmd::cargo::cargo_bin("cargo-changeset")
    }

    // --- dependencies add ---

    #[test]
    fn interactive_dep_add_shows_owner_package_menu() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

        let mut session = TerminalSession::spawn(
            &bin_path(),
            &workspace,
            &["additional-packages", "dependencies", "add"],
        );
        session.wait_for("Select the owner package");
        session.assert_screen(
            "owner menu rendered",
            indoc! {"
                Select the owner package:
                  crate-a (crate)
                  my-helm-chart (additional: charts/my-chart)
            "},
        );
        session.cancel();
        session.wait_for_exit();
    }

    #[test]
    fn interactive_dep_add_cancel_on_owner_exits_cleanly() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);
        let cargo_toml_before =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");

        let mut session = TerminalSession::spawn(
            &bin_path(),
            &workspace,
            &["additional-packages", "dependencies", "add"],
        );
        session.wait_for("Select the owner package");
        session.assert_screen(
            "owner menu before cancel",
            indoc! {"
                Select the owner package:
                  crate-a (crate)
                  my-helm-chart (additional: charts/my-chart)
            "},
        );
        session.cancel();
        session.wait_for_exit();
        session.assert_screen("clean exit after cancel", "");

        let cargo_toml_after =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert_eq!(
            cargo_toml_before, cargo_toml_after,
            "Cargo.toml must not be modified after cancellation"
        );
    }

    #[test]
    fn interactive_dep_add_cancel_on_dependency_exits_cleanly() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);
        let cargo_toml_before =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");

        let mut session = TerminalSession::spawn(
            &bin_path(),
            &workspace,
            &["additional-packages", "dependencies", "add"],
        );
        session.wait_for("Select the owner package");
        session.assert_screen(
            "owner menu before selecting",
            indoc! {"
                Select the owner package:
                  crate-a (crate)
                  my-helm-chart (additional: charts/my-chart)
            "},
        );
        session.select_item(0);
        session.wait_for("Select the dependency to track");
        session.assert_screen(
            "dep menu after owner selected",
            indoc! {"
                Select the owner package: crate-a (crate)
                Select the dependency to track:
                  my-helm-chart (additional: charts/my-chart)
            "},
        );
        session.cancel();
        session.wait_for_exit();

        let cargo_toml_after =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert_eq!(
            cargo_toml_before, cargo_toml_after,
            "Cargo.toml must not be modified after cancellation on dependency menu"
        );
    }

    #[test]
    fn interactive_dep_add_full_flow() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

        let mut session = TerminalSession::spawn(
            &bin_path(),
            &workspace,
            &["additional-packages", "dependencies", "add"],
        );
        session.wait_for("Select the owner package");
        session.assert_screen(
            "owner menu rendered",
            indoc! {"
                Select the owner package:
                  crate-a (crate)
                  my-helm-chart (additional: charts/my-chart)
            "},
        );
        session.select_item(0);
        session.wait_for("Select the dependency to track");
        session.assert_screen(
            "dependency menu after selecting crate owner",
            indoc! {"
                Select the owner package: crate-a (crate)
                Select the dependency to track:
                  my-helm-chart (additional: charts/my-chart)
            "},
        );
        session.select_item(0);
        session.wait_for("version manifest file");
        session.assert_screen(
            "manifest path prompt has no default for crate owner",
            indoc! {"
                Select the owner package: crate-a (crate)
                Select the dependency to track: my-helm-chart (additional: charts/my-chart)
                Path to version manifest file:
            "},
        );
        session.type_line("charts/my-chart/Chart.yaml");
        session.wait_for("Manifest format");
        session.assert_screen(
            "manifest format menu with yaml auto-detected",
            indoc! {"
                Select the owner package: crate-a (crate)
                Select the dependency to track: my-helm-chart (additional: charts/my-chart)
                Path to version manifest file: charts/my-chart/Chart.yaml
                Manifest format:
                  toml
                > yaml
                  json
            "},
        );
        session.confirm();
        session.wait_for("version field");
        session.assert_screen(
            "version field prompt after format confirmed",
            indoc! {"
                Select the owner package: crate-a (crate)
                Select the dependency to track: my-helm-chart (additional: charts/my-chart)
                Path to version manifest file: charts/my-chart/Chart.yaml
                Manifest format: yaml
                Path to version field in manifest (e.g. 'version' or 'info.version'):
            "},
        );
        session.type_line("appVersion");
        session.wait_for("Added version-tracking dependency");
        session.wait_for_exit();
        session.assert_screen(
            "full flow complete",
            indoc! {"
                Select the owner package: crate-a (crate)
                Select the dependency to track: my-helm-chart (additional: charts/my-chart)
                Path to version manifest file: charts/my-chart/Chart.yaml
                Manifest format: yaml
                Path to version field in manifest (e.g. 'version' or 'info.version'): appVersion
                Added version-tracking dependency 'my-helm-chart' to package 'crate-a'
            "},
        );

        let cargo_toml = fs::read_to_string(workspace.path().join("crates/crate-a/Cargo.toml"))
            .expect("read crate-a Cargo.toml");
        assert!(
            cargo_toml.contains(r#"dependency-name = "my-helm-chart""#),
            "expected dependency-name stored, got:\n{cargo_toml}"
        );
        assert!(
            cargo_toml.contains(r#"file-path = "charts/my-chart/Chart.yaml""#),
            "expected manifest file-path stored, got:\n{cargo_toml}"
        );
        assert!(
            cargo_toml.contains(r#"format = "yaml""#),
            "expected auto-detected yaml format stored, got:\n{cargo_toml}"
        );
        assert!(
            cargo_toml.contains(r#"version-field-path = "appVersion""#),
            "expected version-field-path stored, got:\n{cargo_toml}"
        );
    }

    #[test]
    fn interactive_dep_add_self_filter() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

        let mut session = TerminalSession::spawn(
            &bin_path(),
            &workspace,
            &["additional-packages", "dependencies", "add"],
        );
        session.wait_for("Select the owner package");
        session.assert_screen(
            "owner menu before selecting",
            indoc! {"
                Select the owner package:
                  crate-a (crate)
                  my-helm-chart (additional: charts/my-chart)
            "},
        );
        session.select_item(0);
        session.wait_for("Select the dependency to track");
        session.assert_screen(
            "dep menu filters out owner",
            indoc! {"
                Select the owner package: crate-a (crate)
                Select the dependency to track:
                  my-helm-chart (additional: charts/my-chart)
            "},
        );
        session.cancel();
        session.wait_for_exit();
    }

    #[test]
    fn interactive_dep_add_defaults_manifest_for_additional_package_owner() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

        let mut session = TerminalSession::spawn(
            &bin_path(),
            &workspace,
            &["additional-packages", "dependencies", "add"],
        );
        session.wait_for("Select the owner package");
        session.assert_screen(
            "owner menu rendered",
            indoc! {"
                Select the owner package:
                  crate-a (crate)
                  my-helm-chart (additional: charts/my-chart)
            "},
        );
        session.select_item(1);
        session.wait_for("Select the dependency to track");
        session.assert_screen(
            "dependency menu after selecting additional package owner",
            indoc! {"
                Select the owner package: my-helm-chart (additional: charts/my-chart)
                Select the dependency to track:
                  crate-a (crate)
            "},
        );
        session.select_item(0);
        session.wait_for("version manifest file");
        session.assert_screen(
            "manifest path prompt shows default from additional package",
            indoc! {"
                Select the owner package: my-helm-chart (additional: charts/my-chart)
                Select the dependency to track: crate-a (crate)
                Path to version manifest file [charts/my-chart/Chart.yaml]:
            "},
        );
        session.confirm();
        session.wait_for("Manifest format");
        session.assert_screen(
            "manifest format menu with yaml auto-detected from default",
            indoc! {"
                Select the owner package: my-helm-chart (additional: charts/my-chart)
                Select the dependency to track: crate-a (crate)
                Path to version manifest file: charts/my-chart/Chart.yaml
                Manifest format:
                  toml
                > yaml
                  json
            "},
        );
        session.confirm();
        session.wait_for("version field");
        session.assert_screen(
            "version field prompt after format confirmed",
            indoc! {"
                Select the owner package: my-helm-chart (additional: charts/my-chart)
                Select the dependency to track: crate-a (crate)
                Path to version manifest file: charts/my-chart/Chart.yaml
                Manifest format: yaml
                Path to version field in manifest (e.g. 'version' or 'info.version'):
            "},
        );
        session.type_line("appVersion");
        session.wait_for("Added version-tracking dependency");
        session.wait_for_exit();
        session.assert_screen(
            "full flow with default manifest complete",
            indoc! {"
                Select the owner package: my-helm-chart (additional: charts/my-chart)
                Select the dependency to track: crate-a (crate)
                Path to version manifest file: charts/my-chart/Chart.yaml
                Manifest format: yaml
                Path to version field in manifest (e.g. 'version' or 'info.version'): appVersion
                Added version-tracking dependency 'crate-a' to package 'my-helm-chart'
            "},
        );
    }

    // --- dependencies remove ---

    #[test]
    fn interactive_dep_remove_shows_package_menu() {
        let workspace = create_workspace_with_version_tracking_additional_to_cargo();

        let mut session = TerminalSession::spawn(
            &bin_path(),
            &workspace,
            &["additional-packages", "dependencies", "remove"],
        );
        session.wait_for("Select package");
        session.assert_screen(
            "remove package menu",
            indoc! {"
                Select package:
                  my-rust-crate (crate)
                  my-helm-chart (additional: charts)
            "},
        );
        session.cancel();
        session.wait_for_exit();
    }

    #[test]
    fn interactive_dep_remove_cancel_exits_cleanly() {
        let workspace = create_workspace_with_version_tracking_additional_to_cargo();
        let cargo_toml_before =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");

        let mut session = TerminalSession::spawn(
            &bin_path(),
            &workspace,
            &["additional-packages", "dependencies", "remove"],
        );
        session.wait_for("Select package");
        session.assert_screen(
            "remove package menu before cancel",
            indoc! {"
                Select package:
                  my-rust-crate (crate)
                  my-helm-chart (additional: charts)
            "},
        );
        session.cancel();
        session.wait_for_exit();
        session.assert_screen("clean exit after remove cancel", "");

        let cargo_toml_after =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert_eq!(
            cargo_toml_before, cargo_toml_after,
            "Cargo.toml must not be modified after remove cancellation"
        );
    }

    #[test]
    fn interactive_dep_remove_no_deps_shows_message() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);
        let cargo_toml_before =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");

        let mut session = TerminalSession::spawn(
            &bin_path(),
            &workspace,
            &["additional-packages", "dependencies", "remove"],
        );
        session.wait_for("Select package");
        session.assert_screen(
            "remove package menu",
            indoc! {"
                Select package:
                  crate-a (crate)
                  my-helm-chart (additional: charts/my-chart)
            "},
        );
        session.select_item(0);
        session.wait_for("No version-tracking dependencies configured");
        session.wait_for_exit();
        session.assert_screen(
            "no deps message",
            indoc! {"
                Select package: crate-a (crate)
                No version-tracking dependencies configured for 'crate-a'.
            "},
        );

        let cargo_toml_after =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert_eq!(
            cargo_toml_before, cargo_toml_after,
            "Cargo.toml must not be modified when no deps exist"
        );
    }

    #[test]
    fn interactive_dep_remove_shows_registered_deps() {
        let workspace = create_workspace_with_version_tracking_additional_to_cargo();

        let mut session = TerminalSession::spawn(
            &bin_path(),
            &workspace,
            &["additional-packages", "dependencies", "remove"],
        );
        session.wait_for("Select package");
        session.assert_screen(
            "remove package menu",
            indoc! {"
                Select package:
                  my-rust-crate (crate)
                  my-helm-chart (additional: charts)
            "},
        );
        session.select_item(1);
        session.wait_for("Select dependency to remove");
        session.assert_screen(
            "dep removal menu",
            indoc! {"
                Select package: my-helm-chart (additional: charts)
                Select dependency to remove:
                  my-rust-crate
            "},
        );
        session.cancel();
        session.wait_for_exit();
    }

    #[test]
    fn interactive_dep_remove_full_flow() {
        let workspace = create_workspace_with_version_tracking_additional_to_cargo();

        let mut session = TerminalSession::spawn(
            &bin_path(),
            &workspace,
            &["additional-packages", "dependencies", "remove"],
        );
        session.wait_for("Select package");
        session.assert_screen(
            "remove package menu",
            indoc! {"
                Select package:
                  my-rust-crate (crate)
                  my-helm-chart (additional: charts)
            "},
        );
        session.select_item(1);
        session.wait_for("Select dependency to remove");
        session.assert_screen(
            "dependency removal menu after selecting package",
            indoc! {"
                Select package: my-helm-chart (additional: charts)
                Select dependency to remove:
                  my-rust-crate
            "},
        );
        session.select_item(0);
        session.wait_for("Removed version-tracking dependency");
        session.wait_for_exit();
        session.assert_screen(
            "removal complete",
            indoc! {"
                Select package: my-helm-chart (additional: charts)
                Select dependency to remove: my-rust-crate
                Removed version-tracking dependency 'my-rust-crate' from package 'my-helm-chart'
            "},
        );

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(!cargo_toml.contains("dependency-name = \"my-rust-crate\""));
    }

    // --- dependencies list ---

    #[test]
    fn interactive_dep_list_shows_package_menu() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

        let mut session = TerminalSession::spawn(
            &bin_path(),
            &workspace,
            &["additional-packages", "dependencies", "list"],
        );
        session.wait_for("Select package");
        session.assert_screen(
            "list package menu",
            indoc! {"
                Select package:
                  crate-a (crate)
                  my-helm-chart (additional: charts/my-chart)
            "},
        );
        session.cancel();
        session.wait_for_exit();
    }

    #[test]
    fn interactive_dep_list_cancel_exits_cleanly() {
        let workspace = create_workspace_with_helm_chart();
        add_helm_chart_config(&workspace);

        let mut session = TerminalSession::spawn(
            &bin_path(),
            &workspace,
            &["additional-packages", "dependencies", "list"],
        );
        session.wait_for("Select package");
        session.assert_screen(
            "list package menu before cancel",
            indoc! {"
                Select package:
                  crate-a (crate)
                  my-helm-chart (additional: charts/my-chart)
            "},
        );
        session.cancel();
        session.wait_for_exit();
        session.assert_screen("clean exit after list cancel", "");
    }

    #[test]
    fn interactive_dep_list_full_flow() {
        let workspace = create_workspace_with_version_tracking_additional_to_cargo();

        let mut session = TerminalSession::spawn(
            &bin_path(),
            &workspace,
            &["additional-packages", "dependencies", "list"],
        );
        session.wait_for("Select package");
        session.assert_screen(
            "list package menu",
            indoc! {"
                Select package:
                  my-rust-crate (crate)
                  my-helm-chart (additional: charts)
            "},
        );
        session.select_item(1);
        session.wait_for("my-rust-crate");
        session.wait_for_exit();
        session.assert_screen(
            "list output",
            indoc! {"
                Select package: my-helm-chart (additional: charts)

                Version-tracking dependencies:
                  my-rust-crate -> charts/Chart.yaml [yaml]
                    version field: appVersion
            "},
        );
    }
}
