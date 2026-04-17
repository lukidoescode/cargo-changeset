use std::fs;
use std::path::MAIN_SEPARATOR_STR;

use changeset_test_helpers::workspaces::{
    WorkspaceBuilder, create_single_crate_workspace, create_virtual_workspace,
    create_workspace_with_additional_package,
};
use predicates::str::contains;
use tempfile::TempDir;

fn create_workspace_with_underscored_crate() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member_at("crate_one", "0.1.0", "crates/one")
        .build()
}

mod non_interactive {
    use super::*;

    #[test]
    fn add_in_non_tty_multi_crate_workspace_fails() {
        let workspace = create_virtual_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .env_remove("CI")
            .env_remove("GITHUB_ACTIONS")
            .env_remove("GITLAB_CI")
            .env_remove("CIRCLECI")
            .env_remove("TRAVIS")
            .env_remove("JENKINS_URL")
            .env_remove("BUILDKITE")
            .env_remove("TF_BUILD")
            .env_remove("CARGO_CHANGESET_FORCE_TTY")
            .env_remove("CARGO_CHANGESET_NO_TTY")
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("terminal"));
    }

    #[test]
    fn add_single_crate_without_bump_fails() {
        let workspace = create_single_crate_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("missing bump type"));
    }

    #[test]
    fn add_single_crate_without_message_fails() {
        let workspace = create_single_crate_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .arg("--bump")
            .arg("patch")
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("missing description"));
    }

    #[test]
    fn add_single_crate_with_bump_and_message_succeeds() {
        let workspace = create_single_crate_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .arg("--bump")
            .arg("patch")
            .arg("-m")
            .arg("Fixed a bug")
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Using package: test-crate"))
            .stdout(contains("Created changeset"))
            .stdout(contains(format!(
                ".changeset{sep}changesets{sep}",
                sep = MAIN_SEPARATOR_STR
            )))
            .stdout(contains("Fixed a bug"));

        let changeset_dir = workspace.path().join(".changeset/changesets");
        assert!(
            changeset_dir.exists(),
            ".changeset/changesets directory should exist"
        );

        let files: Vec<_> = fs::read_dir(&changeset_dir)
            .expect("read dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
            .collect();

        assert_eq!(files.len(), 1, "should have one changeset file");

        let content = fs::read_to_string(files[0].path()).expect("read changeset file");
        assert!(content.contains("test-crate"), "should contain crate name");
        assert!(content.contains("patch"), "should contain bump type");
        assert!(content.contains("Fixed a bug"), "should contain message");
    }

    #[test]
    fn add_outside_workspace_fails() {
        let dir = TempDir::new().expect("failed to create temp dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .current_dir(dir.path())
            .assert()
            .failure()
            .stderr(contains("project error"));
    }

    #[test]
    fn add_with_single_package_flag_and_bump_selects_specified_package() {
        let workspace = create_virtual_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .arg("--package")
            .arg("crate-a")
            .arg("--bump")
            .arg("minor")
            .arg("-m")
            .arg("Added feature")
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Created changeset"))
            .stdout(contains("crate-a"));
    }

    #[test]
    fn add_with_multiple_package_flags_and_bump_selects_all_specified_packages() {
        let workspace = create_virtual_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .arg("--package")
            .arg("crate-a")
            .arg("--package")
            .arg("crate-b")
            .arg("--bump")
            .arg("patch")
            .arg("-m")
            .arg("Multiple packages")
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Created changeset"))
            .stdout(contains("crate-a"))
            .stdout(contains("crate-b"));
    }

    #[test]
    fn add_with_package_bump_flag() {
        let workspace = create_virtual_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .arg("--package-bump")
            .arg("crate-a:major")
            .arg("-m")
            .arg("Breaking change")
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Created changeset"))
            .stdout(contains("crate-a"))
            .stdout(contains("major"));
    }

    #[test]
    fn add_with_multiple_package_bump_flags() {
        let workspace = create_virtual_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .arg("--package-bump")
            .arg("crate-a:major")
            .arg("--package-bump")
            .arg("crate-b:patch")
            .arg("-m")
            .arg("Mixed changes")
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("crate-a"))
            .stdout(contains("major"))
            .stdout(contains("crate-b"))
            .stdout(contains("patch"));
    }

    #[test]
    fn add_mixing_package_and_package_bump_flags() {
        let workspace = create_virtual_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .arg("--package")
            .arg("crate-b")
            .arg("--package-bump")
            .arg("crate-a:major")
            .arg("--bump")
            .arg("minor")
            .arg("-m")
            .arg("Mixed")
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("crate-a"))
            .stdout(contains("major"))
            .stdout(contains("crate-b"))
            .stdout(contains("minor"));
    }

    #[test]
    fn add_with_unknown_package_fails_with_helpful_error() {
        let workspace = create_virtual_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .arg("--package")
            .arg("nonexistent")
            .arg("--bump")
            .arg("patch")
            .arg("-m")
            .arg("test")
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("unknown package 'nonexistent'"))
            .stderr(contains("crate-a"))
            .stderr(contains("crate-b"));
    }

    #[test]
    fn add_with_invalid_package_bump_format_fails() {
        let workspace = create_virtual_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .arg("--package-bump")
            .arg("no-colon-here")
            .arg("-m")
            .arg("test")
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("invalid --package-bump format"));
    }

    #[test]
    fn add_with_invalid_bump_type_fails() {
        let workspace = create_virtual_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .arg("--package-bump")
            .arg("crate-a:huge")
            .arg("-m")
            .arg("test")
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("invalid bump type 'huge'"));
    }

    #[test]
    fn add_with_empty_message_fails() {
        let workspace = create_single_crate_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .arg("--bump")
            .arg("patch")
            .arg("-m")
            .arg("   ")
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("empty"));
    }

    #[test]
    fn add_with_category_flag() {
        let workspace = create_single_crate_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .arg("--bump")
            .arg("patch")
            .arg("-c")
            .arg("fixed")
            .arg("-m")
            .arg("Fixed a bug")
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Category: Fixed"));

        let changeset_dir = workspace.path().join(".changeset/changesets");
        let files: Vec<_> = fs::read_dir(&changeset_dir)
            .expect("read dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
            .collect();

        let content = fs::read_to_string(files[0].path()).expect("read file");
        assert!(content.contains("category: fixed"));
    }

    #[test]
    fn add_with_package_flag_case_sensitivity() {
        let workspace = create_virtual_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .arg("--package")
            .arg("Crate-A")
            .arg("--bump")
            .arg("patch")
            .arg("-m")
            .arg("test")
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("unknown package 'Crate-A'"));
    }

    #[test]
    fn add_with_package_flag_hyphen_underscore_distinction() {
        let workspace = create_workspace_with_underscored_crate();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .arg("--package")
            .arg("crate-one")
            .arg("--bump")
            .arg("patch")
            .arg("-m")
            .arg("test")
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("unknown package 'crate-one'"))
            .stderr(contains("crate_one"));
    }

    #[test]
    fn add_generates_unique_filenames() {
        let workspace = create_single_crate_workspace();

        for i in 0..3 {
            assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
                .arg("add")
                .arg("--bump")
                .arg("patch")
                .arg("-m")
                .arg(format!("Change {i}"))
                .current_dir(workspace.path())
                .assert()
                .success();
        }

        let changeset_dir = workspace.path().join(".changeset/changesets");
        let files: Vec<_> = fs::read_dir(&changeset_dir)
            .expect("read dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
            .collect();

        assert_eq!(files.len(), 3, "should have three unique changeset files");
    }

    #[test]
    fn add_single_crate_with_bump_none_succeeds() {
        let workspace = create_single_crate_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .arg("--bump")
            .arg("none")
            .arg("-m")
            .arg("Internal refactoring")
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Created changeset"))
            .stdout(contains("Internal refactoring"));

        let changeset_dir = workspace.path().join(".changeset/changesets");
        let files: Vec<_> = fs::read_dir(&changeset_dir)
            .expect("read dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
            .collect();

        assert_eq!(files.len(), 1, "should have one changeset file");

        let content = fs::read_to_string(files[0].path()).expect("read changeset file");
        assert!(content.contains("none"), "should contain none bump type");
        assert!(
            content.contains("Internal refactoring"),
            "should contain message"
        );
    }

    #[test]
    fn add_with_stdin_message() {
        let workspace = create_single_crate_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .arg("--bump")
            .arg("patch")
            .arg("-m")
            .arg("-")
            .write_stdin("Message from stdin")
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Message from stdin"));
    }

    #[test]
    fn add_creates_changeset_for_additional_package() {
        let workspace = create_workspace_with_additional_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .arg("--package-bump")
            .arg("my-helm-chart:minor")
            .arg("-m")
            .arg("Add new endpoint")
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Created changeset"));

        let changeset_dir = workspace.path().join(".changeset/changesets");
        let files: Vec<_> = fs::read_dir(&changeset_dir)
            .expect("read dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
            .collect();

        assert_eq!(files.len(), 1, "should have one changeset file");

        let content = fs::read_to_string(files[0].path()).expect("read changeset file");
        assert!(
            content.contains("my-helm-chart: minor"),
            "should contain package bump entry, got:\n{content}"
        );
        assert!(
            content.contains("Add new endpoint"),
            "should contain message"
        );
    }

    #[test]
    fn add_rejects_unknown_additional_package_name() {
        let workspace = create_workspace_with_additional_package();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .arg("--package-bump")
            .arg("nonexistent-pkg:patch")
            .arg("-m")
            .arg("test")
            .current_dir(workspace.path())
            .assert()
            .failure();
    }
}

#[cfg(not(windows))]
mod interactive {
    use std::os::unix::fs::PermissionsExt;

    use changeset_test_helpers::terminal_session::TerminalSession;

    use super::*;

    fn spawn_add_in_workspace(workspace: &TempDir) -> TerminalSession {
        let bin_path = assert_cmd::cargo::cargo_bin!("cargo-changeset");
        TerminalSession::spawn(bin_path, workspace, &["add"])
    }

    fn spawn_add_with_editor(
        workspace: &TempDir,
        editor_path: &std::path::Path,
    ) -> TerminalSession {
        let bin_path = assert_cmd::cargo::cargo_bin!("cargo-changeset");
        TerminalSession::builder(bin_path, workspace, &["add", "--editor"])
            .env("EDITOR", editor_path)
            .spawn()
    }

    fn create_mock_editor(workspace: &TempDir, content: &str) -> std::path::PathBuf {
        let script_path = workspace.path().join("mock_editor.sh");
        let script_content = format!(
            r#"#!/bin/sh
cat > "$1" << 'MOCK_EDITOR_EOF'
{content}
MOCK_EDITOR_EOF
"#
        );
        fs::write(&script_path, script_content).expect("write mock editor");
        fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
            .expect("make executable");
        script_path
    }

    #[test]
    fn interactive_selection_shows_prompt() {
        let workspace = create_virtual_workspace();
        let mut session = spawn_add_in_workspace(&workspace);

        session.wait_for("Select packages");
        session.cancel();
        session.wait_for_exit();
    }

    #[test]
    fn interactive_shows_crate_names() {
        let workspace = create_virtual_workspace();
        let mut session = spawn_add_in_workspace(&workspace);

        session.wait_for("crate-a");
        session.cancel();
        session.wait_for_exit();
    }

    #[test]
    fn interactive_empty_selection_exits_cleanly() {
        let workspace = create_virtual_workspace();
        let mut session = spawn_add_in_workspace(&workspace);

        session.wait_for("Select packages");
        session.send_raw("\n");
        session.wait_for_exit();
    }

    #[test]
    fn interactive_cancellation_exits_cleanly() {
        let workspace = create_virtual_workspace();
        let mut session = spawn_add_in_workspace(&workspace);

        session.wait_for("Select packages");
        session.cancel();
        session.wait_for_exit();
    }

    #[test]
    fn interactive_select_package_and_bump_type() {
        let workspace = create_virtual_workspace();
        let mut session = spawn_add_in_workspace(&workspace);

        session.wait_for("Select packages");
        session.send_raw(" ");
        session.send_raw("\n");

        session.wait_for("bump type");
        session.send_raw("\n");

        session.wait_for("category");
        session.cancel();
        session.wait_for_exit();
    }

    #[test]
    fn interactive_full_flow_single_package() {
        let workspace = create_single_crate_workspace();
        let mut session = spawn_add_in_workspace(&workspace);

        session.wait_for("Using package: test-crate");

        session.wait_for("bump type");
        session.send_raw("\n");

        session.wait_for("category");
        session.send_raw("\n");

        session.wait_for("description");

        session.send_line("Test description line 1");
        session.send_line("Test description line 2");
        session.send_line("");
        session.send_line("");

        session.wait_for("Created changeset");
        session.wait_for_exit();

        let changeset_dir = workspace.path().join(".changeset/changesets");
        assert!(
            changeset_dir.exists(),
            ".changeset/changesets directory should exist"
        );

        let files: Vec<_> = fs::read_dir(&changeset_dir)
            .expect("read dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
            .collect();
        assert_eq!(files.len(), 1, "should have one changeset file");

        let content = fs::read_to_string(files[0].path()).expect("read file");
        assert!(content.contains("test-crate"));
        assert!(content.contains("patch"));
        assert!(content.contains("Test description line 1"));
        assert!(content.contains("Test description line 2"));
    }

    #[test]
    fn interactive_full_flow_multi_package() {
        let workspace = create_virtual_workspace();
        let mut session = spawn_add_in_workspace(&workspace);

        session.wait_for("Select packages");
        session.send_raw(" ");
        session.send_raw("\n");

        session.wait_for("bump type");
        session.send_raw("\n");

        session.wait_for("category");
        session.send_raw("\n");

        session.wait_for("description");

        session.send_line("Multi-package changeset");
        session.send_line("");
        session.send_line("");

        session.wait_for("Created changeset");
        session.wait_for_exit();
    }

    #[test]
    fn interactive_with_editor_flag() {
        let workspace = create_single_crate_workspace();
        let editor = create_mock_editor(&workspace, "Description from mock editor");

        let mut session = spawn_add_with_editor(&workspace, &editor);

        session.wait_for("Using package: test-crate");

        session.wait_for("bump type");
        session.send_raw("\n");

        session.wait_for("category");
        session.send_raw("\n");

        session.wait_for("Created changeset");
        session.wait_for_exit();

        let changeset_dir = workspace.path().join(".changeset/changesets");
        let files: Vec<_> = fs::read_dir(&changeset_dir)
            .expect("read dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
            .collect();

        let content = fs::read_to_string(files[0].path()).expect("read file");
        assert!(
            content.contains("Description from mock editor"),
            "File should contain editor content: {content}"
        );
    }

    #[test]
    fn interactive_editor_filters_comments() {
        let workspace = create_single_crate_workspace();
        let editor = create_mock_editor(
            &workspace,
            "# This is a comment\nActual description\n# Another comment",
        );

        let mut session = spawn_add_with_editor(&workspace, &editor);

        session.wait_for("Using package: test-crate");

        session.wait_for("bump type");
        session.send_raw("\n");

        session.wait_for("category");
        session.send_raw("\n");

        session.wait_for("Created changeset");
        session.wait_for_exit();

        let changeset_dir = workspace.path().join(".changeset/changesets");
        let files: Vec<_> = fs::read_dir(&changeset_dir)
            .expect("read dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
            .collect();

        let content = fs::read_to_string(files[0].path()).expect("read file");
        assert!(content.contains("Actual description"));
        assert!(
            !content.contains("# This is a comment"),
            "Comments should be filtered"
        );
    }
}

mod ci_detection {
    use super::*;

    #[test]
    fn add_in_ci_environment_without_package_fails() {
        let workspace = create_virtual_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .env("CI", "true")
            .env_remove("CARGO_CHANGESET_FORCE_TTY")
            .env_remove("CARGO_CHANGESET_NO_TTY")
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("CI environment"))
            .stderr(contains("$CI"));
    }

    #[test]
    fn add_with_no_tty_env_requires_message() {
        let workspace = create_single_crate_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .arg("--bump")
            .arg("patch")
            .env("CARGO_CHANGESET_NO_TTY", "1")
            .env_remove("CARGO_CHANGESET_FORCE_TTY")
            .env_remove("CI")
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("missing description"));
    }

    #[test]
    fn add_with_all_flags_succeeds_in_ci() {
        let workspace = create_single_crate_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .arg("--bump")
            .arg("patch")
            .arg("-m")
            .arg("CI change")
            .env("CI", "true")
            .env_remove("CARGO_CHANGESET_FORCE_TTY")
            .env_remove("CARGO_CHANGESET_NO_TTY")
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Created changeset"));
    }

    #[test]
    fn error_message_includes_helpful_guidance() {
        let workspace = create_virtual_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .env("GITHUB_ACTIONS", "true")
            .env_remove("CARGO_CHANGESET_FORCE_TTY")
            .env_remove("CARGO_CHANGESET_NO_TTY")
            .env_remove("CI")
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("$GITHUB_ACTIONS"))
            .stderr(contains("--package"))
            .stderr(contains("--bump"))
            .stderr(contains("-m"));
    }

    #[test]
    fn force_tty_overrides_ci_detection() {
        let workspace = create_virtual_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .env("CI", "true")
            .env("CARGO_CHANGESET_FORCE_TTY", "1")
            .env_remove("CARGO_CHANGESET_NO_TTY")
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("terminal"));
    }

    #[test]
    fn no_tty_takes_priority_over_ci_and_force_tty() {
        let workspace = create_virtual_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .env("CARGO_CHANGESET_NO_TTY", "1")
            .env("CI", "true")
            .env("CARGO_CHANGESET_FORCE_TTY", "1")
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("CARGO_CHANGESET_NO_TTY"));
    }
}
