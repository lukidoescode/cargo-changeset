use std::fs;

use changeset_test_helpers::git::{create_branch, git_add_and_commit, init_git_repo};
use changeset_test_helpers::workspaces::WorkspaceBuilder;
use predicates::str::contains;
use tempfile::TempDir;

fn setup_single_package() -> TempDir {
    WorkspaceBuilder::single_package("test-crate", "1.0.0").build()
}

fn setup_workspace() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member("foo", "1.0.0")
        .crate_member("bar", "2.0.0")
        .build()
}

fn setup_virtual_workspace() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member("alpha", "0.1.0")
        .build()
}

fn setup_workspace_with_root_package() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .root_package("root-pkg", "1.0.0")
        .crate_member_at("sub-pkg", "0.5.0", "crates/sub")
        .build()
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
        assert!(cargo_toml.contains("keep-changesets = true"));
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
        assert!(cargo_toml.contains(r#"tag-format = "version-only""#));
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
        assert!(cargo_toml.contains(r#"tag-format = "crate-prefixed""#));
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
        assert!(cargo_toml.contains(r#"comparison-links = "auto""#));
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
        assert!(cargo_toml.contains(r#"comparison-links = "enabled""#));
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
        assert!(cargo_toml.contains(r#"comparison-links = "disabled""#));
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
        assert!(cargo_toml.contains(r#"zero-version-behavior = "effective-minor""#));
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
        assert!(cargo_toml.contains(r#"zero-version-behavior = "auto-promote-on-major""#));
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
        assert!(cargo_toml.contains("keep-changesets = true"));
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
        assert!(cargo_toml.contains("keep-changesets = false"));
        assert!(cargo_toml.contains(r#"tag-format = "crate-prefixed""#));
        assert!(cargo_toml.contains(r#"changelog = "per-package""#));
        assert!(cargo_toml.contains(r#"comparison-links = "enabled""#));
        assert!(cargo_toml.contains(r#"zero-version-behavior = "auto-promote-on-major""#));

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
        assert!(cargo_toml.contains(r#"tag-format = "version-only""#));

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["init", "--tag-format", "crate-prefixed"])
            .current_dir(workspace.path())
            .assert()
            .success();

        let cargo_toml =
            fs::read_to_string(workspace.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml.contains(r#"tag-format = "crate-prefixed""#),
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

#[cfg(not(windows))]
mod interactive_init_tests {
    use std::path::PathBuf;

    use changeset_test_helpers::terminal_session::TerminalSession;
    use indoc::indoc;

    use super::*;

    fn bin_path() -> PathBuf {
        assert_cmd::cargo::cargo_bin("cargo-changeset")
    }

    fn skip_all_sections(session: &mut TerminalSession) {
        session.wait_for("Configure git settings?");
        session.assert_screen(
            "git settings prompt",
            indoc! {"
            Configure git settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure changelog settings?");
        session.assert_screen(
            "changelog settings prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure version settings?");
        session.assert_screen(
            "version settings prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure file filtering?");
        session.assert_screen(
            "file filtering prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? no
                Configure file filtering? [y/N]"},
        );
        session.send_raw("n");
    }

    #[test]
    fn interactive_init_skip_all_sections() {
        let dir = setup_single_package();

        let mut session = TerminalSession::spawn(&bin_path(), &dir, &["changeset", "init"]);
        skip_all_sections(&mut session);
        session.wait_for("Proceed with initialization?");
        session.wait_for("No configuration will be written (using defaults).");
        session.assert_screen_starts_with(
            "final confirmation with defaults (start)",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? no
                Configure file filtering? no"},
        );
        session.assert_screen_ends_with(
            "final confirmation with defaults (end)",
            indoc! {"
                No configuration will be written (using defaults).

                Proceed with initialization? [Y/n]"},
        );
        session.send_raw("y");
        session.wait_for_exit();

        assert!(
            dir.path().join(".changeset").exists(),
            ".changeset directory must be created"
        );
    }

    #[test]
    fn interactive_init_cancel_at_final_confirmation() {
        let dir = setup_single_package();

        let mut session = TerminalSession::spawn(&bin_path(), &dir, &["changeset", "init"]);
        skip_all_sections(&mut session);
        session.wait_for("Proceed with initialization?");
        session.wait_for("No configuration will be written (using defaults).");
        session.assert_screen_starts_with(
            "final confirmation with defaults before cancel (start)",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? no
                Configure file filtering? no"},
        );
        session.assert_screen_ends_with(
            "final confirmation with defaults before cancel (end)",
            indoc! {"
                No configuration will be written (using defaults).

                Proceed with initialization? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for_exit();

        assert!(
            !dir.path().join(".changeset").exists(),
            ".changeset directory must not be created after declining"
        );
    }

    #[test]
    fn interactive_init_cancel_at_git_settings() {
        let dir = setup_single_package();

        let mut session = TerminalSession::spawn(&bin_path(), &dir, &["changeset", "init"]);
        session.wait_for("Configure git settings?");
        session.assert_screen(
            "git settings prompt before cancel",
            indoc! {"
            Configure git settings? [Y/n]"},
        );
        session.cancel();
        session.wait_for_exit();

        assert!(
            !dir.path().join(".changeset").exists(),
            ".changeset directory must not be created after cancelling at git settings"
        );
    }

    #[test]
    fn interactive_init_cancel_at_changelog_settings() {
        let dir = setup_single_package();

        let mut session = TerminalSession::spawn(&bin_path(), &dir, &["changeset", "init"]);
        session.wait_for("Configure git settings?");
        session.assert_screen(
            "git settings prompt",
            indoc! {"
            Configure git settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure changelog settings?");
        session.assert_screen(
            "changelog settings prompt before cancel",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? [Y/n]"},
        );
        session.cancel();
        session.wait_for_exit();

        assert!(
            !dir.path().join(".changeset").exists(),
            ".changeset directory must not be created after cancelling at changelog settings"
        );
    }

    #[test]
    fn interactive_init_cancel_at_version_settings() {
        let dir = setup_single_package();

        let mut session = TerminalSession::spawn(&bin_path(), &dir, &["changeset", "init"]);
        session.wait_for("Configure git settings?");
        session.assert_screen(
            "git settings prompt",
            indoc! {"
            Configure git settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure changelog settings?");
        session.assert_screen(
            "changelog settings prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure version settings?");
        session.assert_screen(
            "version settings prompt before cancel",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? [Y/n]"},
        );
        session.cancel();
        session.wait_for_exit();

        assert!(
            !dir.path().join(".changeset").exists(),
            ".changeset directory must not be created after cancelling at version settings"
        );
    }

    #[test]
    fn interactive_init_cancel_at_filtering() {
        let dir = setup_single_package();

        let mut session = TerminalSession::spawn(&bin_path(), &dir, &["changeset", "init"]);
        session.wait_for("Configure git settings?");
        session.assert_screen(
            "git settings prompt",
            indoc! {"
            Configure git settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure changelog settings?");
        session.assert_screen(
            "changelog settings prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure version settings?");
        session.assert_screen(
            "version settings prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure file filtering?");
        session.assert_screen(
            "file filtering prompt before cancel",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? no
                Configure file filtering? [y/N]"},
        );
        session.cancel();
        session.wait_for_exit();

        assert!(
            !dir.path().join(".changeset").exists(),
            ".changeset directory must not be created after cancelling at filtering"
        );
    }

    #[test]
    fn interactive_init_cancel_mid_git_section() {
        let dir = setup_single_package();

        let mut session = TerminalSession::spawn(&bin_path(), &dir, &["changeset", "init"]);
        session.wait_for("Configure git settings?");
        session.assert_screen(
            "git settings prompt",
            indoc! {"
            Configure git settings? [Y/n]"},
        );
        session.send_raw("y");
        session.wait_for("Create git commits on release?");
        session.assert_screen(
            "commits prompt before cancel",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? [Y/n]"},
        );
        session.cancel();
        session.wait_for_exit();

        assert!(
            !dir.path().join(".changeset").exists(),
            ".changeset directory must not be created after cancelling mid git section"
        );
    }

    #[test]
    fn interactive_init_cancel_mid_changelog_section() {
        let dir = setup_workspace();

        let mut session = TerminalSession::spawn(&bin_path(), &dir, &["changeset", "init"]);
        session.wait_for("Configure git settings?");
        session.assert_screen(
            "git settings prompt",
            indoc! {"
            Configure git settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure changelog settings?");
        session.assert_screen(
            "changelog settings prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? [Y/n]"},
        );
        session.send_raw("y");
        session.wait_for("Select changelog location");
        session.assert_screen(
            "changelog location menu before cancel",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? yes
                Select changelog location:
                > root - Single CHANGELOG.md at project root (default)
                  per-package - CHANGELOG.md in each package directory"},
        );
        session.cancel();
        session.wait_for_exit();

        assert!(
            !dir.path().join(".changeset").exists(),
            ".changeset directory must not be created after cancelling mid changelog section"
        );
    }

    #[test]
    fn interactive_init_cancel_mid_version_section() {
        let dir = setup_single_package();

        let mut session = TerminalSession::spawn(&bin_path(), &dir, &["changeset", "init"]);
        session.wait_for("Configure git settings?");
        session.assert_screen(
            "git settings prompt",
            indoc! {"
            Configure git settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure changelog settings?");
        session.assert_screen(
            "changelog settings prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure version settings?");
        session.assert_screen(
            "version settings prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? [Y/n]"},
        );
        session.send_raw("y");
        session.wait_for("Select zero version");
        session.assert_screen(
            "zero version menu before cancel",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? yes
                Select zero version (0.x.y) behavior:
                > effective-minor - Major bump on 0.x increments minor (default)
                  auto-promote-on-major - Major bump on 0.x promotes to 1.0.0"},
        );
        session.cancel();
        session.wait_for_exit();

        assert!(
            !dir.path().join(".changeset").exists(),
            ".changeset directory must not be created after cancelling mid version section"
        );
    }

    #[test]
    fn interactive_init_git_settings_full_flow() {
        let dir = setup_single_package();

        let mut session = TerminalSession::spawn(&bin_path(), &dir, &["changeset", "init"]);
        session.wait_for("Configure git settings?");
        session.assert_screen(
            "configure git prompt",
            indoc! {"
            Configure git settings? [Y/n]"},
        );
        session.send_raw("y");

        session.wait_for("Create git commits on release?");
        session.assert_screen(
            "commits prompt",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? [Y/n]"},
        );
        session.send_raw("y");

        session.wait_for("Create git tags on release?");
        session.assert_screen(
            "tags prompt",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? [Y/n]"},
        );
        session.send_raw("n");

        session.wait_for("Keep changeset files after release?");
        session.assert_screen(
            "keep changesets prompt",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? no
                Keep changeset files after release? [y/N]"},
        );
        session.send_raw("y");

        session.wait_for("Select tag format");
        session.assert_screen(
            "tag format menu",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? no
                Keep changeset files after release? yes
                Select tag format:
                > version-only - Tags like v1.0.0
                  crate-prefixed - Tags like crate-name@1.0.0"},
        );
        session.confirm();

        session.wait_for("Default base branch");
        session.assert_screen(
            "base branch prompt",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? no
                Keep changeset files after release? yes
                Select tag format: version-only - Tags like v1.0.0
                Default base branch for git comparisons [main]:"},
        );
        session.confirm();

        session.wait_for("Commit title template");
        session.assert_screen(
            "commit title prompt",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? no
                Keep changeset files after release? yes
                Select tag format: version-only - Tags like v1.0.0
                Default base branch for git comparisons: main
                Commit title template (placeholder: {new-version}) [{new-version}]:"},
        );
        session.confirm();

        session.wait_for("Include version details in commit body?");
        session.assert_screen(
            "commit body prompt",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? no
                Keep changeset files after release? yes
                Select tag format: version-only - Tags like v1.0.0
                Default base branch for git comparisons: main
                Commit title template (placeholder: {new-version}): {new-version}
                Include version details in commit body? [Y/n]"},
        );
        session.send_raw("y");

        session.wait_for("Configure changelog settings?");
        session.assert_screen(
            "changelog section after git",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? no
                Keep changeset files after release? yes
                Select tag format: version-only - Tags like v1.0.0
                Default base branch for git comparisons: main
                Commit title template (placeholder: {new-version}): {new-version}
                Include version details in commit body? yes
                Configure changelog settings? [Y/n]"},
        );
        session.send_raw("n");

        session.wait_for("Configure version settings?");
        session.assert_screen(
            "version settings prompt after git",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? no
                Keep changeset files after release? yes
                Select tag format: version-only - Tags like v1.0.0
                Default base branch for git comparisons: main
                Commit title template (placeholder: {new-version}): {new-version}
                Include version details in commit body? yes
                Configure changelog settings? no
                Configure version settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure file filtering?");
        session.assert_screen(
            "file filtering prompt after git",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? no
                Keep changeset files after release? yes
                Select tag format: version-only - Tags like v1.0.0
                Default base branch for git comparisons: main
                Commit title template (placeholder: {new-version}): {new-version}
                Include version details in commit body? yes
                Configure changelog settings? no
                Configure version settings? no
                Configure file filtering? [y/N]"},
        );
        session.send_raw("n");
        session.wait_for("Proceed with initialization?");
        session.assert_screen_ends_with(
            "final confirmation after git settings (end)",
            "Proceed with initialization? [Y/n]",
        );
        session.send_raw("y");
        session.wait_for_exit();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml.contains("tags = false"),
            "expected tags = false, got:\n{cargo_toml}"
        );
        assert!(
            cargo_toml.contains("keep-changesets = true"),
            "expected keep-changesets = true, got:\n{cargo_toml}"
        );
    }

    #[test]
    fn interactive_init_git_no_commit_skips_title_and_body() {
        let dir = setup_single_package();

        let mut session = TerminalSession::spawn(&bin_path(), &dir, &["changeset", "init"]);
        session.wait_for("Configure git settings?");
        session.assert_screen(
            "configure git prompt",
            indoc! {"
            Configure git settings? [Y/n]"},
        );
        session.send_raw("y");

        session.wait_for("Create git commits on release?");
        session.assert_screen(
            "commits prompt",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? [Y/n]"},
        );
        session.send_raw("n");

        session.wait_for("Create git tags on release?");
        session.assert_screen(
            "tags prompt",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? no
                Create git tags on release? [Y/n]"},
        );
        session.send_raw("y");

        session.wait_for("Keep changeset files after release?");
        session.assert_screen(
            "keep changesets prompt",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? no
                Create git tags on release? yes
                Keep changeset files after release? [y/N]"},
        );
        session.send_raw("n");

        session.wait_for("Select tag format");
        session.assert_screen(
            "tag format menu",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? no
                Create git tags on release? yes
                Keep changeset files after release? no
                Select tag format:
                > version-only - Tags like v1.0.0
                  crate-prefixed - Tags like crate-name@1.0.0"},
        );
        session.confirm();

        session.wait_for("Default base branch");
        session.assert_screen(
            "base branch prompt",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? no
                Create git tags on release? yes
                Keep changeset files after release? no
                Select tag format: version-only - Tags like v1.0.0
                Default base branch for git comparisons [main]:"},
        );
        session.confirm();

        session.wait_for("Configure changelog settings?");
        session.assert_screen(
            "skipped to changelog after base branch",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? no
                Create git tags on release? yes
                Keep changeset files after release? no
                Select tag format: version-only - Tags like v1.0.0
                Default base branch for git comparisons: main
                Configure changelog settings? [Y/n]"},
        );
        session.send_raw("n");

        session.wait_for("Configure version settings?");
        session.assert_screen(
            "version settings prompt after git no-commit",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? no
                Create git tags on release? yes
                Keep changeset files after release? no
                Select tag format: version-only - Tags like v1.0.0
                Default base branch for git comparisons: main
                Configure changelog settings? no
                Configure version settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure file filtering?");
        session.assert_screen(
            "file filtering prompt after git no-commit",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? no
                Create git tags on release? yes
                Keep changeset files after release? no
                Select tag format: version-only - Tags like v1.0.0
                Default base branch for git comparisons: main
                Configure changelog settings? no
                Configure version settings? no
                Configure file filtering? [y/N]"},
        );
        session.send_raw("n");
        session.wait_for("Proceed with initialization?");
        session.assert_screen_ends_with(
            "final confirmation after git no-commit (end)",
            "Proceed with initialization? [Y/n]",
        );
        session.send_raw("y");
        session.wait_for_exit();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml.contains("commit = false"),
            "expected commit = false, got:\n{cargo_toml}"
        );
        assert!(
            !cargo_toml.contains("commit-title"),
            "commit-title should not appear when commit=false, got:\n{cargo_toml}"
        );
        assert!(
            !cargo_toml.contains("changes-in-body"),
            "changes-in-body should not appear when commit=false, got:\n{cargo_toml}"
        );
    }

    #[test]
    fn interactive_init_changelog_settings_full_flow() {
        let dir = setup_workspace();

        let mut session = TerminalSession::spawn(&bin_path(), &dir, &["changeset", "init"]);
        session.wait_for("Configure git settings?");
        session.assert_screen(
            "git settings prompt",
            indoc! {"
            Configure git settings? [Y/n]"},
        );
        session.send_raw("n");

        session.wait_for("Configure changelog settings?");
        session.assert_screen(
            "changelog settings prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? [Y/n]"},
        );
        session.send_raw("y");

        session.wait_for("Select changelog location");
        session.assert_screen(
            "changelog location menu",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? yes
                Select changelog location:
                > root - Single CHANGELOG.md at project root (default)
                  per-package - CHANGELOG.md in each package directory"},
        );
        session.select_item(0);

        session.wait_for("Select comparison links mode");
        session.assert_screen(
            "comparison links menu",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? yes
                Select changelog location: per-package - CHANGELOG.md in each package directory
                Select comparison links mode:
                > auto - Generate links if git remote detected (default)
                  enabled - Always generate comparison links
                  disabled - Never generate comparison links"},
        );
        session.select_item(0);

        session.wait_for("Comparison links template");
        session.assert_screen(
            "comparison links template prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? yes
                Select changelog location: per-package - CHANGELOG.md in each package directory
                Select comparison links mode: enabled - Always generate comparison links
                Comparison links template (empty=auto-detect, placeholders: {repository}, {base}, {target}) []:"},
        );
        session.confirm();

        session.wait_for("Dependency bump changelog template");
        session.assert_screen(
            "dependency bump template prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? yes
                Select changelog location: per-package - CHANGELOG.md in each package directory
                Select comparison links mode: enabled - Always generate comparison links
                Comparison links template (empty=auto-detect, placeholders: {repository}, {base}, {target}):
                Dependency bump changelog template (placeholders: {dependency}, {version}) [Updated dependency `{dependency}` to v{version}]:"},
        );
        session.confirm();

        session.wait_for("Configure version settings?");
        session.assert_screen_starts_with(
            "version settings prompt after changelog (start)",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? yes
                Select changelog location: per-package - CHANGELOG.md in each package directory
                Select comparison links mode: enabled - Always generate comparison links"},
        );
        session.assert_screen_ends_with(
            "version settings prompt after changelog (end)",
            "Configure version settings? [Y/n]",
        );
        session.send_raw("n");
        session.wait_for("Configure file filtering?");
        session.assert_screen_starts_with(
            "file filtering prompt after changelog (start)",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? yes
                Select changelog location: per-package - CHANGELOG.md in each package directory
                Select comparison links mode: enabled - Always generate comparison links"},
        );
        session.assert_screen_ends_with(
            "file filtering prompt after changelog (end)",
            "Configure file filtering? [y/N]",
        );
        session.send_raw("n");
        session.wait_for("Proceed with initialization?");
        session.assert_screen_ends_with(
            "final confirmation after changelog settings (end)",
            "Proceed with initialization? [Y/n]",
        );
        session.send_raw("y");
        session.wait_for_exit();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml.contains("[workspace.metadata.changeset]"),
            "expected changeset config, got:\n{cargo_toml}"
        );
    }

    #[test]
    fn interactive_init_version_settings_full_flow() {
        let dir = setup_single_package();

        let mut session = TerminalSession::spawn(&bin_path(), &dir, &["changeset", "init"]);
        session.wait_for("Configure git settings?");
        session.assert_screen(
            "git settings prompt",
            indoc! {"
            Configure git settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure changelog settings?");
        session.assert_screen(
            "changelog settings prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? [Y/n]"},
        );
        session.send_raw("n");

        session.wait_for("Configure version settings?");
        session.assert_screen(
            "version settings prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? [Y/n]"},
        );
        session.send_raw("y");

        session.wait_for("Select zero version");
        session.assert_screen(
            "zero version menu",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? yes
                Select zero version (0.x.y) behavior:
                > effective-minor - Major bump on 0.x increments minor (default)
                  auto-promote-on-major - Major bump on 0.x promotes to 1.0.0"},
        );
        session.confirm();

        session.wait_for("Select none bump behavior");
        session.assert_screen(
            "none bump menu",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? yes
                Select zero version (0.x.y) behavior: effective-minor - Major bump on 0.x increments minor (default)
                Select none bump behavior:
                > promote-to-patch - Treat none bumps as patch releases (default)
                  allow - Allow none bumps without version change
                  disallow - Reject changesets with none bump type"},
        );
        session.confirm();

        session.wait_for("Changelog message template");
        session.assert_screen(
            "promote message prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? yes
                Select zero version (0.x.y) behavior: effective-minor - Major bump on 0.x increments minor (default)
                Select none bump behavior: promote-to-patch - Treat none bumps as patch releases (default)
                Changelog message template for promoted none bumps [Internal architectural changes]:"},
        );
        session.confirm();

        session.wait_for("Configure file filtering?");
        session.assert_screen(
            "file filtering prompt after version settings",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? yes
                Select zero version (0.x.y) behavior: effective-minor - Major bump on 0.x increments minor (default)
                Select none bump behavior: promote-to-patch - Treat none bumps as patch releases (default)
                Changelog message template for promoted none bumps: Internal architectural changes
                Configure file filtering? [y/N]"},
        );
        session.send_raw("n");
        session.wait_for("Proceed with initialization?");
        session.assert_screen_starts_with(
            "final confirmation after version settings (start)",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? yes
                Select zero version (0.x.y) behavior: effective-minor - Major bump on 0.x increments minor (default)
                Select none bump behavior: promote-to-patch - Treat none bumps as patch releases (default)
                Changelog message template for promoted none bumps: Internal architectural changes
                Configure file filtering? no"},
        );
        session.assert_screen_ends_with(
            "final confirmation after version settings (end)",
            "Proceed with initialization? [Y/n]",
        );
        session.send_raw("y");
        session.wait_for_exit();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml.contains("[package.metadata.changeset]"),
            "expected changeset config, got:\n{cargo_toml}"
        );
    }

    #[test]
    fn interactive_init_version_allow_skips_promote_message() {
        let dir = setup_single_package();

        let mut session = TerminalSession::spawn(&bin_path(), &dir, &["changeset", "init"]);
        session.wait_for("Configure git settings?");
        session.assert_screen(
            "git settings prompt",
            indoc! {"
            Configure git settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure changelog settings?");
        session.assert_screen(
            "changelog settings prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? [Y/n]"},
        );
        session.send_raw("n");

        session.wait_for("Configure version settings?");
        session.assert_screen(
            "version settings prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? [Y/n]"},
        );
        session.send_raw("y");

        session.wait_for("Select zero version");
        session.assert_screen(
            "zero version menu",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? yes
                Select zero version (0.x.y) behavior:
                > effective-minor - Major bump on 0.x increments minor (default)
                  auto-promote-on-major - Major bump on 0.x promotes to 1.0.0"},
        );
        session.confirm();

        session.wait_for("Select none bump behavior");
        session.assert_screen(
            "none bump menu before selecting allow",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? yes
                Select zero version (0.x.y) behavior: effective-minor - Major bump on 0.x increments minor (default)
                Select none bump behavior:
                > promote-to-patch - Treat none bumps as patch releases (default)
                  allow - Allow none bumps without version change
                  disallow - Reject changesets with none bump type"},
        );
        session.select_item(0);

        session.wait_for("Configure file filtering?");
        session.assert_screen(
            "skipped to filtering after allow",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? yes
                Select zero version (0.x.y) behavior: effective-minor - Major bump on 0.x increments minor (default)
                Select none bump behavior: allow - Allow none bumps without version change
                Configure file filtering? [y/N]"},
        );
        session.send_raw("n");
        session.wait_for("Proceed with initialization?");
        session.assert_screen_starts_with(
            "final confirmation after version allow (start)",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? yes
                Select zero version (0.x.y) behavior: effective-minor - Major bump on 0.x increments minor (default)
                Select none bump behavior: allow - Allow none bumps without version change
                Configure file filtering? no"},
        );
        session.assert_screen_ends_with(
            "final confirmation after version allow (end)",
            "Proceed with initialization? [Y/n]",
        );
        session.send_raw("y");
        session.wait_for_exit();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            !cargo_toml.contains("none-bump-promote-message-template"),
            "promote message template should not appear when behavior=allow, got:\n{cargo_toml}"
        );
    }

    #[test]
    fn interactive_init_filtering_adds_patterns() {
        let dir = setup_single_package();

        let mut session = TerminalSession::spawn(&bin_path(), &dir, &["changeset", "init"]);
        session.wait_for("Configure git settings?");
        session.assert_screen(
            "git settings prompt",
            indoc! {"
            Configure git settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure changelog settings?");
        session.assert_screen(
            "changelog settings prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure version settings?");
        session.assert_screen(
            "version settings prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure file filtering?");
        session.assert_screen(
            "file filtering prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? no
                Configure file filtering? [y/N]"},
        );
        session.send_raw("y");
        session.wait_for("Ignore pattern");
        session.assert_screen(
            "first pattern prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? no
                Configure file filtering? yes
                Enter file patterns to exclude from change detection (one per line, empty line to finish):
                Ignore pattern:"},
        );
        session.type_line("*.log");
        session.wait_for("Additional pattern");
        session.assert_screen(
            "additional pattern prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? no
                Configure file filtering? yes
                Enter file patterns to exclude from change detection (one per line, empty line to finish):
                Ignore pattern: *.log
                Additional pattern:"},
        );
        session.type_line("tmp/**");
        session.wait_for("tmp/**\nAdditional pattern:");
        session.assert_screen(
            "second additional pattern prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? no
                Configure file filtering? yes
                Enter file patterns to exclude from change detection (one per line, empty line to finish):
                Ignore pattern: *.log
                Additional pattern: tmp/**
                Additional pattern:"},
        );
        session.type_line("");
        session.wait_for("Proceed with initialization?");
        session.assert_screen_starts_with(
            "final confirmation after filtering (start)",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? no
                Configure file filtering? yes
                Enter file patterns to exclude from change detection (one per line, empty line to finish):
                Ignore pattern: *.log
                Additional pattern: tmp/**
                Additional pattern:"},
        );
        session.assert_screen_ends_with(
            "final confirmation after filtering (end)",
            "Proceed with initialization? [Y/n]",
        );
        session.send_raw("y");
        session.wait_for_exit();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml.contains("*.log"),
            "expected *.log pattern in config, got:\n{cargo_toml}"
        );
        assert!(
            cargo_toml.contains("tmp/**"),
            "expected tmp/** pattern in config, got:\n{cargo_toml}"
        );
    }

    #[test]
    fn interactive_init_filtering_empty_skips() {
        let dir = setup_single_package();

        let mut session = TerminalSession::spawn(&bin_path(), &dir, &["changeset", "init"]);
        session.wait_for("Configure git settings?");
        session.assert_screen(
            "git settings prompt",
            indoc! {"
            Configure git settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure changelog settings?");
        session.assert_screen(
            "changelog settings prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure version settings?");
        session.assert_screen(
            "version settings prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? [Y/n]"},
        );
        session.send_raw("n");
        session.wait_for("Configure file filtering?");
        session.assert_screen(
            "file filtering prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? no
                Configure file filtering? [y/N]"},
        );
        session.send_raw("y");
        session.wait_for("Ignore pattern");
        session.assert_screen(
            "first pattern prompt",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? no
                Configure file filtering? yes
                Enter file patterns to exclude from change detection (one per line, empty line to finish):
                Ignore pattern:"},
        );
        session.type_line("");
        session.wait_for("Proceed with initialization?");
        session.assert_screen_starts_with(
            "final confirmation after empty filtering (start)",
            indoc! {"
                Configure git settings? no
                Configure changelog settings? no
                Configure version settings? no
                Configure file filtering? yes
                Enter file patterns to exclude from change detection (one per line, empty line to finish):
                Ignore pattern:"},
        );
        session.assert_screen_ends_with(
            "final confirmation after empty filtering (end)",
            "Proceed with initialization? [Y/n]",
        );
        session.send_raw("y");
        session.wait_for_exit();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            !cargo_toml.contains("ignored"),
            "expected no ignored_files in config, got:\n{cargo_toml}"
        );
    }

    #[test]
    fn interactive_init_full_flow_all_sections() {
        let dir = setup_workspace();

        let mut session = TerminalSession::spawn(&bin_path(), &dir, &["changeset", "init"]);

        session.wait_for("Configure git settings?");
        session.assert_screen(
            "configure git prompt",
            indoc! {"
            Configure git settings? [Y/n]"},
        );
        session.send_raw("y");

        session.wait_for("Create git commits on release?");
        session.assert_screen(
            "commits prompt",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? [Y/n]"},
        );
        session.send_raw("y");

        session.wait_for("Create git tags on release?");
        session.assert_screen(
            "tags prompt",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? [Y/n]"},
        );
        session.send_raw("y");

        session.wait_for("Keep changeset files after release?");
        session.assert_screen(
            "keep changesets prompt",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? yes
                Keep changeset files after release? [y/N]"},
        );
        session.send_raw("n");

        session.wait_for("Select tag format");
        session.assert_screen(
            "tag format menu (workspace default = crate-prefixed)",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? yes
                Keep changeset files after release? no
                Select tag format:
                  version-only - Tags like v1.0.0
                > crate-prefixed - Tags like crate-name@1.0.0"},
        );
        session.select_item(0);

        session.wait_for("Default base branch");
        session.assert_screen(
            "base branch prompt",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? yes
                Keep changeset files after release? no
                Select tag format: version-only - Tags like v1.0.0
                Default base branch for git comparisons [main]:"},
        );
        session.type_line("develop");

        session.wait_for("Commit title template");
        session.assert_screen(
            "commit title prompt",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? yes
                Keep changeset files after release? no
                Select tag format: version-only - Tags like v1.0.0
                Default base branch for git comparisons: develop
                Commit title template (placeholder: {new-version}) [{new-version}]:"},
        );
        session.type_line("release: {new-version}");

        session.wait_for("Include version details in commit body?");
        session.assert_screen(
            "commit body prompt",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? yes
                Keep changeset files after release? no
                Select tag format: version-only - Tags like v1.0.0
                Default base branch for git comparisons: develop
                Commit title template (placeholder: {new-version}): release: {new-version}
                Include version details in commit body? [Y/n]"},
        );
        session.send_raw("n");

        session.wait_for("Configure changelog settings?");
        session.assert_screen(
            "changelog section",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? yes
                Keep changeset files after release? no
                Select tag format: version-only - Tags like v1.0.0
                Default base branch for git comparisons: develop
                Commit title template (placeholder: {new-version}): release: {new-version}
                Include version details in commit body? no
                Configure changelog settings? [Y/n]"},
        );
        session.send_raw("y");

        session.wait_for("Select changelog location");
        session.assert_screen(
            "changelog location menu",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? yes
                Keep changeset files after release? no
                Select tag format: version-only - Tags like v1.0.0
                Default base branch for git comparisons: develop
                Commit title template (placeholder: {new-version}): release: {new-version}
                Include version details in commit body? no
                Configure changelog settings? yes
                Select changelog location:
                > root - Single CHANGELOG.md at project root (default)
                  per-package - CHANGELOG.md in each package directory"},
        );
        session.select_item(0);

        session.wait_for("Select comparison links mode");
        session.assert_screen(
            "comparison links menu",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? yes
                Keep changeset files after release? no
                Select tag format: version-only - Tags like v1.0.0
                Default base branch for git comparisons: develop
                Commit title template (placeholder: {new-version}): release: {new-version}
                Include version details in commit body? no
                Configure changelog settings? yes
                Select changelog location: per-package - CHANGELOG.md in each package directory
                Select comparison links mode:
                > auto - Generate links if git remote detected (default)
                  enabled - Always generate comparison links
                  disabled - Never generate comparison links"},
        );
        session.confirm();

        session.wait_for("Comparison links template");
        session.assert_screen(
            "comparison links template prompt",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? yes
                Keep changeset files after release? no
                Select tag format: version-only - Tags like v1.0.0
                Default base branch for git comparisons: develop
                Commit title template (placeholder: {new-version}): release: {new-version}
                Include version details in commit body? no
                Configure changelog settings? yes
                Select changelog location: per-package - CHANGELOG.md in each package directory
                Select comparison links mode: auto - Generate links if git remote detected (default)
                Comparison links template (empty=auto-detect, placeholders: {repository}, {base}, {target}) []:"},
        );
        session.confirm();

        session.wait_for("Dependency bump changelog template");
        session.assert_screen(
            "dependency bump template prompt",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? yes
                Keep changeset files after release? no
                Select tag format: version-only - Tags like v1.0.0
                Default base branch for git comparisons: develop
                Commit title template (placeholder: {new-version}): release: {new-version}
                Include version details in commit body? no
                Configure changelog settings? yes
                Select changelog location: per-package - CHANGELOG.md in each package directory
                Select comparison links mode: auto - Generate links if git remote detected (default)
                Comparison links template (empty=auto-detect, placeholders: {repository}, {base}, {target}):
                Dependency bump changelog template (placeholders: {dependency}, {version}) [Updated dependency `{dependency}` to v{version}]:"},
        );
        session.confirm();

        session.wait_for("Configure version settings?");
        session.assert_screen_starts_with(
            "version settings prompt in full flow (start)",
            indoc! {"
                Configure git settings? yes
                Create git commits on release? yes
                Create git tags on release? yes
                Keep changeset files after release? no
                Select tag format: version-only - Tags like v1.0.0
                Default base branch for git comparisons: develop
                Commit title template (placeholder: {new-version}): release: {new-version}
                Include version details in commit body? no
                Configure changelog settings? yes
                Select changelog location: per-package - CHANGELOG.md in each package directory"},
        );
        session.assert_screen_ends_with(
            "version settings prompt in full flow (end)",
            "Configure version settings? [Y/n]",
        );
        session.send_raw("y");

        session.wait_for("Select zero version");
        session.assert_screen_ends_with(
            "zero version menu in full flow",
            indoc! {"
                Configure version settings? yes
                Select zero version (0.x.y) behavior:
                > effective-minor - Major bump on 0.x increments minor (default)
                  auto-promote-on-major - Major bump on 0.x promotes to 1.0.0"},
        );
        session.select_item(0);

        session.wait_for("Select none bump behavior");
        session.assert_screen_ends_with(
            "none bump menu in full flow",
            indoc! {"
                Select zero version (0.x.y) behavior: auto-promote-on-major - Major bump on 0.x promotes to 1.0.0
                Select none bump behavior:
                > promote-to-patch - Treat none bumps as patch releases (default)
                  allow - Allow none bumps without version change
                  disallow - Reject changesets with none bump type"},
        );
        session.select_item(1);

        session.wait_for("Configure file filtering?");
        session.assert_screen_ends_with(
            "file filtering prompt in full flow",
            indoc! {"
                Select none bump behavior: disallow - Reject changesets with none bump type
                Configure file filtering? [y/N]"},
        );
        session.send_raw("y");

        session.wait_for("Ignore pattern");
        session.assert_screen_ends_with(
            "ignore pattern prompt in full flow",
            indoc! {"
                Configure file filtering? yes
                Enter file patterns to exclude from change detection (one per line, empty line to finish):
                Ignore pattern:"},
        );
        session.type_line("*.tmp");
        session.wait_for("Additional pattern");
        session.assert_screen_ends_with(
            "additional pattern prompt in full flow",
            indoc! {"
                Ignore pattern: *.tmp
                Additional pattern:"},
        );
        session.type_line("");

        session.wait_for("Proceed with initialization?");
        session.assert_screen_ends_with(
            "final confirmation in full flow",
            "Proceed with initialization? [Y/n]",
        );
        session.send_raw("y");
        session.wait_for_exit();

        let cargo_toml =
            fs::read_to_string(dir.path().join("Cargo.toml")).expect("read Cargo.toml");
        assert!(
            cargo_toml.contains("base-branch = \"develop\""),
            "expected base-branch = develop, got:\n{cargo_toml}"
        );
        assert!(
            cargo_toml.contains("release: {new-version}"),
            "expected custom commit title, got:\n{cargo_toml}"
        );
        assert!(
            cargo_toml.contains("changes-in-body = false"),
            "expected changes-in-body = false, got:\n{cargo_toml}"
        );
        assert!(
            cargo_toml.contains("*.tmp"),
            "expected *.tmp pattern, got:\n{cargo_toml}"
        );
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
