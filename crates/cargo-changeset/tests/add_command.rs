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
    fn add_single_crate_succeeds_without_tty() {
        let workspace = create_single_crate_workspace();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Using crate: test-crate"))
            .stdout(contains("Selected 1 crate"));
    }

    #[test]
    fn add_outside_workspace_fails() {
        let dir = TempDir::new().expect("failed to create temp dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .arg("add")
            .current_dir(dir.path())
            .assert()
            .failure()
            .stderr(contains("workspace error"));
    }
}

mod interactive {
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

    #[test]
    fn interactive_selection_shows_prompt() {
        let workspace = create_virtual_workspace();
        let mut session = spawn_add_in_workspace(&workspace);

        let result = session.expect("Select crates");
        assert!(result.is_ok(), "Expected to see 'Select crates' prompt");
    }

    #[test]
    fn interactive_shows_crate_names() {
        let workspace = create_virtual_workspace();
        let mut session = spawn_add_in_workspace(&workspace);

        session.expect("crate-a").expect("Expected to see crate-a");
    }

    #[test]
    fn interactive_select_and_confirm() {
        let workspace = create_virtual_workspace();
        let mut session = spawn_add_in_workspace(&workspace);

        session.expect("Select crates").expect("Expected prompt");

        session.send(" ").expect("failed to send space");
        session.send("\n").expect("failed to send enter");

        session
            .expect("Selected")
            .expect("Expected to see selection confirmation");
    }

    #[test]
    fn interactive_empty_selection_exits_cleanly() {
        let workspace = create_virtual_workspace();
        let mut session = spawn_add_in_workspace(&workspace);

        session.expect("Select crates").expect("Expected prompt");

        session.send("\n").expect("failed to send enter");

        let wait_result = session.expect(expectrl::Eof);
        assert!(wait_result.is_ok(), "Process should exit cleanly");
    }

    #[test]
    fn interactive_cancellation_exits_cleanly() {
        let workspace = create_virtual_workspace();
        let mut session = spawn_add_in_workspace(&workspace);

        session.expect("Select crates").expect("Expected prompt");

        session.send("\x1b").expect("failed to send escape");

        let wait_result = session.expect(expectrl::Eof);
        assert!(
            wait_result.is_ok(),
            "Process should exit cleanly after cancellation"
        );
    }
}
