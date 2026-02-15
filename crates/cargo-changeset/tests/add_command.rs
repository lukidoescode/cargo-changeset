use std::fs;
use std::process::Command;
use std::time::Duration;

use predicates::str::contains;
use tempfile::TempDir;

fn create_single_crate_workspace() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");
    fs::create_dir_all(dir.path().join("src")).expect("failed to create src dir");
    fs::write(
        dir.path().join("Cargo.toml"),
        r#"
[package]
name = "test-crate"
version = "1.0.0"
edition = "2021"
"#,
    )
    .expect("failed to write Cargo.toml");
    fs::write(dir.path().join("src/lib.rs"), "").expect("failed to write lib.rs");

    dir
}

fn create_virtual_workspace() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    fs::create_dir_all(dir.path().join("crates/a/src")).expect("failed to create crate a dir");
    fs::create_dir_all(dir.path().join("crates/b/src")).expect("failed to create crate b dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"
[workspace]
members = ["crates/*"]
resolver = "2"
"#,
    )
    .expect("failed to write workspace Cargo.toml");

    fs::write(
        dir.path().join("crates/a/Cargo.toml"),
        r#"
[package]
name = "crate-a"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("failed to write crate-a Cargo.toml");

    fs::write(dir.path().join("crates/a/src/lib.rs"), "").expect("failed to write crate-a lib.rs");

    fs::write(
        dir.path().join("crates/b/Cargo.toml"),
        r#"
[package]
name = "crate-b"
version = "0.2.0"
edition = "2021"
"#,
    )
    .expect("failed to write crate-b Cargo.toml");

    fs::write(dir.path().join("crates/b/src/lib.rs"), "").expect("failed to write crate-b lib.rs");

    dir
}

fn create_workspace_with_underscored_crate() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    fs::create_dir_all(dir.path().join("crates/one/src")).expect("failed to create crate one dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"
[workspace]
members = ["crates/*"]
resolver = "2"
"#,
    )
    .expect("failed to write workspace Cargo.toml");

    fs::write(
        dir.path().join("crates/one/Cargo.toml"),
        r#"
[package]
name = "crate_one"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("failed to write crate_one Cargo.toml");

    fs::write(dir.path().join("crates/one/src/lib.rs"), "")
        .expect("failed to write crate_one lib.rs");

    dir
}

mod non_interactive {
    use super::*;

    #[test]
    fn add_in_non_tty_multi_crate_workspace_fails() {
        let workspace = create_virtual_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
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
            .stdout(contains("Major"));
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
            .stdout(contains("Major"))
            .stdout(contains("crate-b"))
            .stdout(contains("Patch"));
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
            .stdout(contains("Major"))
            .stdout(contains("crate-b"))
            .stdout(contains("Minor"));
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
}

#[cfg(not(windows))]
mod interactive {
    use std::os::unix::fs::PermissionsExt;

    use expectrl::Expect;
    use expectrl::session::OsSession;

    use super::*;

    fn spawn_add_in_workspace(workspace: &TempDir) -> OsSession {
        let bin_path = assert_cmd::cargo::cargo_bin!("cargo-changeset");

        let mut cmd = Command::new(bin_path);
        cmd.arg("add");
        cmd.current_dir(workspace.path());
        cmd.env("CARGO_CHANGESET_FORCE_TTY", "1");

        let mut session = OsSession::spawn(cmd).expect("failed to spawn session");
        session.set_expect_timeout(Some(Duration::from_secs(30)));
        session
    }

    fn spawn_add_with_editor(workspace: &TempDir, editor_path: &std::path::Path) -> OsSession {
        let bin_path = assert_cmd::cargo::cargo_bin!("cargo-changeset");

        let mut cmd = Command::new(bin_path);
        cmd.arg("add");
        cmd.arg("--editor");
        cmd.current_dir(workspace.path());
        cmd.env("CARGO_CHANGESET_FORCE_TTY", "1");
        cmd.env("EDITOR", editor_path);

        let mut session = OsSession::spawn(cmd).expect("failed to spawn session");
        session.set_expect_timeout(Some(Duration::from_secs(30)));
        session
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

        let result = session.expect("Select packages");
        assert!(result.is_ok(), "Expected to see 'Select packages' prompt");
    }

    #[test]
    fn interactive_shows_crate_names() {
        let workspace = create_virtual_workspace();
        let mut session = spawn_add_in_workspace(&workspace);

        session.expect("crate-a").expect("Expected to see crate-a");
    }

    #[test]
    fn interactive_empty_selection_exits_cleanly() {
        let workspace = create_virtual_workspace();
        let mut session = spawn_add_in_workspace(&workspace);

        session.expect("Select packages").expect("Expected prompt");

        session.send("\n").expect("failed to send enter");

        let wait_result = session.expect(expectrl::Eof);
        assert!(wait_result.is_ok(), "Process should exit cleanly");
    }

    #[test]
    fn interactive_cancellation_exits_cleanly() {
        let workspace = create_virtual_workspace();
        let mut session = spawn_add_in_workspace(&workspace);

        session.expect("Select packages").expect("Expected prompt");

        session.send("\x1b").expect("failed to send escape");

        let wait_result = session.expect(expectrl::Eof);
        assert!(
            wait_result.is_ok(),
            "Process should exit cleanly after cancellation"
        );
    }

    #[test]
    fn interactive_select_package_and_bump_type() {
        let workspace = create_virtual_workspace();
        let mut session = spawn_add_in_workspace(&workspace);

        session
            .expect("Select packages")
            .expect("Expected package selection prompt");
        session.send(" ").expect("failed to select first package");
        session.send("\n").expect("failed to confirm selection");

        session
            .expect("bump type")
            .expect("Expected bump type prompt");
        session.send("\n").expect("failed to select bump type");

        session
            .expect("category")
            .expect("Expected category prompt");
    }

    #[test]
    fn interactive_full_flow_single_package() {
        let workspace = create_single_crate_workspace();
        let mut session = spawn_add_in_workspace(&workspace);

        session
            .expect("Using package: test-crate")
            .expect("Expected single package auto-selection");

        session
            .expect("bump type")
            .expect("Expected bump type prompt");
        session.send("\n").expect("failed to select bump type");

        session
            .expect("category")
            .expect("Expected category prompt");
        session.send("\n").expect("failed to select category");

        session
            .expect("description")
            .expect("Expected description prompt");

        session
            .send_line("Test description line 1")
            .expect("failed to send line 1");
        session
            .send_line("Test description line 2")
            .expect("failed to send line 2");
        session.send_line("").expect("failed to send empty line 1");
        session.send_line("").expect("failed to send empty line 2");

        session
            .expect("Created changeset")
            .expect("Expected success message");

        let wait_result = session.expect(expectrl::Eof);
        assert!(wait_result.is_ok(), "Process should exit cleanly");

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

        session
            .expect("Select packages")
            .expect("Expected package selection prompt");
        session.send(" ").expect("failed to select first package");
        session.send("\n").expect("failed to confirm selection");

        session
            .expect("bump type")
            .expect("Expected bump type prompt");
        session.send("\n").expect("failed to select bump type");

        session
            .expect("category")
            .expect("Expected category prompt");
        session.send("\n").expect("failed to select category");

        session
            .expect("description")
            .expect("Expected description prompt");

        session
            .send_line("Multi-package changeset")
            .expect("failed to send description");
        session.send_line("").expect("failed to send empty line 1");
        session.send_line("").expect("failed to send empty line 2");

        session
            .expect("Created changeset")
            .expect("Expected success message");

        let wait_result = session.expect(expectrl::Eof);
        assert!(wait_result.is_ok(), "Process should exit cleanly");
    }

    #[test]
    fn interactive_with_editor_flag() {
        let workspace = create_single_crate_workspace();
        let editor = create_mock_editor(&workspace, "Description from mock editor");

        let mut session = spawn_add_with_editor(&workspace, &editor);

        session
            .expect("Using package: test-crate")
            .expect("Expected single package auto-selection");

        session
            .expect("bump type")
            .expect("Expected bump type prompt");
        session.send("\n").expect("failed to select bump type");

        session
            .expect("category")
            .expect("Expected category prompt");
        session.send("\n").expect("failed to select category");

        session
            .expect("Created changeset")
            .expect("Expected success message");

        let wait_result = session.expect(expectrl::Eof);
        assert!(wait_result.is_ok(), "Process should exit cleanly");

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

        session
            .expect("Using package: test-crate")
            .expect("Expected single package auto-selection");

        session
            .expect("bump type")
            .expect("Expected bump type prompt");
        session.send("\n").expect("failed to select bump type");

        session
            .expect("category")
            .expect("Expected category prompt");
        session.send("\n").expect("failed to select category");

        session
            .expect("Created changeset")
            .expect("Expected success message");

        session.expect(expectrl::Eof).ok();

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
