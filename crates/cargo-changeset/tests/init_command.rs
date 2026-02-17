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

fn setup_single_package() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    fs::create_dir_all(dir.path().join("src")).expect("create src dir");
    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[package]
name = "test-crate"
version = "1.0.0"
edition = "2021"
"#,
    )
    .expect("write Cargo.toml");
    fs::write(dir.path().join("src/lib.rs"), "").expect("write lib.rs");
    dir
}

fn setup_workspace() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    fs::create_dir_all(dir.path().join("crates/foo/src")).expect("create foo dir");
    fs::create_dir_all(dir.path().join("crates/bar/src")).expect("create bar dir");
    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"
"#,
    )
    .expect("write workspace Cargo.toml");
    fs::write(
        dir.path().join("crates/foo/Cargo.toml"),
        r#"[package]
name = "foo"
version = "1.0.0"
edition = "2021"
"#,
    )
    .expect("write foo Cargo.toml");
    fs::write(dir.path().join("crates/foo/src/lib.rs"), "").expect("write foo lib.rs");
    fs::write(
        dir.path().join("crates/bar/Cargo.toml"),
        r#"[package]
name = "bar"
version = "2.0.0"
edition = "2021"
"#,
    )
    .expect("write bar Cargo.toml");
    fs::write(dir.path().join("crates/bar/src/lib.rs"), "").expect("write bar lib.rs");
    dir
}

fn setup_virtual_workspace() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    fs::create_dir_all(dir.path().join("crates/alpha/src")).expect("create alpha dir");
    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"
"#,
    )
    .expect("write workspace Cargo.toml");
    fs::write(
        dir.path().join("crates/alpha/Cargo.toml"),
        r#"[package]
name = "alpha"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("write alpha Cargo.toml");
    fs::write(dir.path().join("crates/alpha/src/lib.rs"), "").expect("write alpha lib.rs");
    dir
}

fn setup_workspace_with_root_package() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    fs::create_dir_all(dir.path().join("src")).expect("create src dir");
    fs::create_dir_all(dir.path().join("crates/sub/src")).expect("create sub dir");
    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[package]
name = "root-pkg"
version = "1.0.0"
edition = "2021"

[workspace]
members = ["crates/*"]
"#,
    )
    .expect("write workspace Cargo.toml");
    fs::write(dir.path().join("src/lib.rs"), "").expect("write root lib.rs");
    fs::write(
        dir.path().join("crates/sub/Cargo.toml"),
        r#"[package]
name = "sub-pkg"
version = "0.5.0"
edition = "2021"
"#,
    )
    .expect("write sub Cargo.toml");
    fs::write(dir.path().join("crates/sub/src/lib.rs"), "").expect("write sub lib.rs");
    dir
}

mod directory_creation {
    use super::*;

    #[test]
    fn creates_changeset_directory_and_gitkeep() {
        let dir = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init"])
            .current_dir(dir.path())
            .assert()
            .success()
            .stdout(contains("Created changeset directory"));

        assert!(
            dir.path().join(".changeset").exists(),
            ".changeset directory should exist"
        );
        assert!(
            dir.path().join(".changeset/.gitkeep").exists(),
            ".gitkeep should exist"
        );

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["add", "--bump", "patch", "-m", "Test"])
            .current_dir(dir.path())
            .assert()
            .success();

        assert!(
            dir.path().join(".changeset/changesets").exists(),
            "changesets subdirectory should exist after adding changeset"
        );
    }

    #[test]
    fn init_without_flags_creates_directory_only() {
        let dir = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init"])
            .current_dir(dir.path())
            .assert()
            .success();

        assert!(dir.path().join(".changeset").exists());

        let original_toml = r#"[package]
name = "test-crate"
version = "1.0.0"
edition = "2021"
"#;
        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert_eq!(cargo_toml, original_toml);
    }

    #[test]
    fn fails_outside_project() {
        let dir = TempDir::new().expect("create temp dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init"])
            .current_dir(dir.path())
            .assert()
            .failure()
            .stderr(contains("project error"));
    }
}

mod config_flags {
    use super::*;

    #[test]
    fn defaults_flag_writes_config() {
        let dir = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--defaults"])
            .current_dir(dir.path())
            .assert()
            .success()
            .stdout(contains("Wrote configuration"))
            .stdout(contains("[package.metadata.changeset]"));

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains("[package.metadata.changeset]"));
    }

    #[test]
    fn workspace_uses_workspace_metadata() {
        let dir = setup_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--defaults"])
            .current_dir(dir.path())
            .assert()
            .success()
            .stdout(contains("[workspace.metadata.changeset]"));

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains("[workspace.metadata.changeset]"));
    }

    #[test]
    fn commit_flag_writes_config() {
        let dir = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--commit", "false"])
            .current_dir(dir.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains("commit = false"));
    }

    #[test]
    fn tags_flag_writes_config() {
        let dir = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--tags", "true"])
            .current_dir(dir.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains("tags = true"));
    }

    #[test]
    fn keep_changesets_flag_writes_config() {
        let dir = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--keep-changesets", "true"])
            .current_dir(dir.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains("keep_changesets = true"));
    }

    #[test]
    fn tag_format_version_only_writes_config() {
        let dir = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--tag-format", "version-only"])
            .current_dir(dir.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains(r#"tag_format = "version-only""#));
    }

    #[test]
    fn tag_format_crate_prefixed_writes_config() {
        let dir = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--tag-format", "crate-prefixed"])
            .current_dir(dir.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains(r#"tag_format = "crate-prefixed""#));
    }

    #[test]
    fn changelog_root_writes_config() {
        let dir = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--changelog", "root"])
            .current_dir(dir.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains(r#"changelog = "root""#));
    }

    #[test]
    fn changelog_per_package_writes_config() {
        let dir = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--changelog", "per-package"])
            .current_dir(dir.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains(r#"changelog = "per-package""#));
    }

    #[test]
    fn comparison_links_auto_writes_config() {
        let dir = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--comparison-links", "auto"])
            .current_dir(dir.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains(r#"comparison_links = "auto""#));
    }

    #[test]
    fn comparison_links_enabled_writes_config() {
        let dir = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--comparison-links", "enabled"])
            .current_dir(dir.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains(r#"comparison_links = "enabled""#));
    }

    #[test]
    fn comparison_links_disabled_writes_config() {
        let dir = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--comparison-links", "disabled"])
            .current_dir(dir.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains(r#"comparison_links = "disabled""#));
    }

    #[test]
    fn zero_version_behavior_effective_minor_writes_config() {
        let dir = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--zero-version-behavior", "effective-minor"])
            .current_dir(dir.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains(r#"zero_version_behavior = "effective-minor""#));
    }

    #[test]
    fn zero_version_behavior_auto_promote_on_major_writes_config() {
        let dir = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--zero-version-behavior", "auto-promote-on-major"])
            .current_dir(dir.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains(r#"zero_version_behavior = "auto-promote-on-major""#));
    }

    #[test]
    fn multiple_git_flags_write_config() {
        let dir = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "init",
                "--commit",
                "false",
                "--tags",
                "true",
                "--keep-changesets",
                "true",
            ])
            .current_dir(dir.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains("commit = false"));
        assert!(cargo_toml.contains("tags = true"));
        assert!(cargo_toml.contains("keep_changesets = true"));
    }

    #[test]
    fn all_options_combined() {
        let workspace = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "init",
                "--commit",
                "true",
                "--tags",
                "true",
                "--keep-changesets",
                "false",
                "--tag-format",
                "crate-prefixed",
                "--changelog",
                "per-package",
                "--comparison-links",
                "enabled",
                "--zero-version-behavior",
                "auto-promote-on-major",
            ])
            .current_dir(workspace.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains("commit = true"));
        assert!(cargo_toml.contains("tags = true"));
        assert!(cargo_toml.contains("keep_changesets = false"));
        assert!(cargo_toml.contains(r#"tag_format = "crate-prefixed""#));
        assert!(cargo_toml.contains(r#"changelog = "per-package""#));
        assert!(cargo_toml.contains(r#"comparison_links = "enabled""#));
        assert!(cargo_toml.contains(r#"zero_version_behavior = "auto-promote-on-major""#));

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["status"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("No pending changesets"));
    }

    #[test]
    fn incremental_config_additions() {
        let workspace = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init"])
            .current_dir(workspace.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            !cargo_toml.contains("[package.metadata.changeset]"),
            "no config should be written without flags"
        );

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--commit", "true"])
            .current_dir(workspace.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains("commit = true"));

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--tags", "false"])
            .current_dir(workspace.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml.contains("commit = true"),
            "should preserve commit"
        );
        assert!(cargo_toml.contains("tags = false"), "should add tags");
    }
}

mod invalid_flags {
    use super::*;

    #[test]
    fn invalid_tag_format_fails() {
        let dir = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--tag-format", "invalid"])
            .current_dir(dir.path())
            .assert()
            .failure();
    }

    #[test]
    fn invalid_changelog_location_fails() {
        let dir = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--changelog", "invalid"])
            .current_dir(dir.path())
            .assert()
            .failure();
    }

    #[test]
    fn invalid_comparison_links_fails() {
        let dir = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--comparison-links", "invalid"])
            .current_dir(dir.path())
            .assert()
            .failure();
    }

    #[test]
    fn invalid_zero_version_behavior_fails() {
        let dir = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--zero-version-behavior", "invalid"])
            .current_dir(dir.path())
            .assert()
            .failure();
    }
}

mod reinit_scenarios {
    use super::*;

    #[test]
    fn reinit_after_adding_changesets_preserves_them() {
        let workspace = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init"])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["add", "--bump", "patch", "-m", "First fix"])
            .current_dir(workspace.path())
            .assert()
            .success();

        let changeset_dir = workspace.path().join(".changeset/changesets");
        let files_before: Vec<_> = fs::read_dir(&changeset_dir)
            .expect("read changeset dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
            .collect();
        assert_eq!(files_before.len(), 1);

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--defaults"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("already exists"));

        let files_after: Vec<_> = fs::read_dir(&changeset_dir)
            .expect("read changeset dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
            .collect();
        assert_eq!(
            files_after.len(),
            1,
            "changeset should still exist after reinit"
        );
    }

    #[test]
    fn reinit_updates_config_preserves_changesets() {
        let workspace = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--defaults"])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["add", "--bump", "minor", "-m", "New feature"])
            .current_dir(workspace.path())
            .assert()
            .success();

        let cargo_toml_before =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml_before.contains("[package.metadata.changeset]"));

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--commit", "false", "--tags", "false"])
            .current_dir(workspace.path())
            .assert()
            .success();

        let cargo_toml_after =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml_after.contains("commit = false"),
            "config should be updated"
        );
        assert!(
            cargo_toml_after.contains("tags = false"),
            "config should be updated"
        );

        let changeset_dir = workspace.path().join(".changeset/changesets");
        let files: Vec<_> = fs::read_dir(&changeset_dir)
            .expect("read changeset dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
            .collect();
        assert_eq!(
            files.len(),
            1,
            "changeset should be preserved after config update"
        );
    }

    #[test]
    fn reinit_with_different_config_options() {
        let workspace = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--tag-format", "version-only"])
            .current_dir(workspace.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains(r#"tag_format = "version-only""#));

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--tag-format", "crate-prefixed"])
            .current_dir(workspace.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml.contains(r#"tag_format = "crate-prefixed""#),
            "config should be updated with new value"
        );
    }
}

mod workflow_tests {
    use super::*;

    #[test]
    fn init_then_add_changeset_succeeds() {
        let workspace = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init"])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["add", "--bump", "patch", "-m", "Fix a bug"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Created changeset"));

        let changeset_dir = workspace.path().join(".changeset/changesets");
        assert!(changeset_dir.exists(), "changesets directory should exist");

        let files: Vec<_> = fs::read_dir(&changeset_dir)
            .expect("read changeset dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
            .collect();

        assert_eq!(files.len(), 1, "should have one changeset file");
    }

    #[test]
    fn init_then_status_shows_no_changesets() {
        let workspace = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init"])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["status"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("No pending changesets"));
    }

    #[test]
    fn init_then_add_then_status_shows_changeset() {
        let workspace = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init"])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["add", "--bump", "minor", "-m", "Add new feature"])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["status"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Pending changesets: 1"))
            .stdout(contains("test-crate: 1.0.0 -> 1.1.0"));
    }

    #[test]
    fn init_then_verify_succeeds_without_changes() {
        let workspace = setup_single_package();
        init_git_repo(&workspace);
        git_add_and_commit(&workspace, "Initial commit");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init"])
            .current_dir(workspace.path())
            .assert()
            .success();

        git_add_and_commit(&workspace, "Add changeset directory");

        create_branch(&workspace, "feature");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["verify", "--base", "main"])
            .current_dir(workspace.path())
            .assert()
            .success();
    }

    #[test]
    fn init_add_verify_workflow() {
        let workspace = setup_single_package();
        init_git_repo(&workspace);
        git_add_and_commit(&workspace, "Initial commit");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init"])
            .current_dir(workspace.path())
            .assert()
            .success();

        git_add_and_commit(&workspace, "Add changeset directory");
        create_branch(&workspace, "feature");

        fs::write(
            workspace.path().join("src/lib.rs"),
            "pub fn new_function() {}",
        )
        .expect("modify lib.rs");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["add", "--bump", "minor", "-m", "Add new function"])
            .current_dir(workspace.path())
            .assert()
            .success();

        git_add_and_commit(&workspace, "Add feature with changeset");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["verify", "--base", "main"])
            .current_dir(workspace.path())
            .assert()
            .success();
    }

    #[test]
    fn init_verify_add_status_cycle() {
        let workspace = setup_workspace();
        init_git_repo(&workspace);
        git_add_and_commit(&workspace, "Initial commit");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--defaults"])
            .current_dir(workspace.path())
            .assert()
            .success();

        git_add_and_commit(&workspace, "Initialize changeset");
        create_branch(&workspace, "feature-1");

        fs::write(
            workspace.path().join("crates/foo/src/lib.rs"),
            "pub fn feature_one() {}",
        )
        .expect("modify foo");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "add",
                "--package",
                "foo",
                "--bump",
                "minor",
                "-m",
                "Feature one",
            ])
            .current_dir(workspace.path())
            .assert()
            .success();

        git_add_and_commit(&workspace, "Add feature one");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["verify", "--base", "main"])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["status"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("foo: 1.0.0 -> 1.1.0"));
    }
}

mod project_type_scenarios {
    use super::*;

    #[test]
    fn virtual_workspace_full_workflow() {
        let workspace = setup_virtual_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--defaults"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("[workspace.metadata.changeset]"));

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains("[workspace.metadata.changeset]"));

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "add",
                "--package",
                "alpha",
                "--bump",
                "minor",
                "-m",
                "Add feature to alpha",
            ])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["status"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Pending changesets: 1"))
            .stdout(contains("alpha"));
    }

    #[test]
    fn single_package_full_workflow() {
        let workspace = setup_single_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--defaults"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("[package.metadata.changeset]"));

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains("[package.metadata.changeset]"));

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["add", "--bump", "patch", "-m", "Bug fix"])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["status"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Pending changesets: 1"))
            .stdout(contains("test-crate: 1.0.0 -> 1.0.1"));
    }

    #[test]
    fn workspace_with_root_package_full_workflow() {
        let workspace = setup_workspace_with_root_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--defaults"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("[workspace.metadata.changeset]"));

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(cargo_toml.contains("[workspace.metadata.changeset]"));

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "add",
                "--package",
                "root-pkg",
                "--bump",
                "major",
                "-m",
                "Breaking change in root",
            ])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "add",
                "--package",
                "sub-pkg",
                "--bump",
                "minor",
                "-m",
                "Feature in sub",
            ])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["status"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Pending changesets: 2"))
            .stdout(contains("root-pkg: 1.0.0 -> 2.0.0"))
            .stdout(contains("sub-pkg: 0.5.0 -> 0.5.1"));
    }

    #[test]
    fn multi_package_workspace_full_workflow() {
        let workspace = setup_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--defaults"])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "add",
                "--package-bump",
                "foo:minor",
                "--package-bump",
                "bar:patch",
                "-m",
                "Multi-package change",
            ])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["status"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Pending changesets: 1"))
            .stdout(contains("foo: 1.0.0 -> 1.1.0"))
            .stdout(contains("bar: 2.0.0 -> 2.0.1"));
    }
}
