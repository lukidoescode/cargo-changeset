use std::fs;
use std::path::PathBuf;

use changeset_test_helpers::terminal_session::TerminalSession;
use changeset_test_helpers::workspaces::{WorkspaceBuilder, create_virtual_workspace};
use indoc::indoc;
use predicates::str::contains;
use tempfile::TempDir;

fn create_workspace_with_stable_version() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member_at("stable-crate", "1.2.3", "crates/stable")
        .build()
}

fn create_workspace_with_prerelease_version() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member_at("prerelease-crate", "0.1.0-alpha.1", "crates/pre")
        .build()
}

mod manage_prerelease {
    use super::*;

    #[test]
    fn add_creates_prerelease_toml() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release", "--add", "crate-a:alpha"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Added crate-a to pre-release configuration"));

        let prerelease_path = workspace.path().join(".changeset/pre-release.toml");
        assert!(prerelease_path.exists(), "pre-release.toml should exist");

        let content = fs::read_to_string(&prerelease_path).expect("read pre-release.toml");
        assert!(content.contains("crate-a"));
        assert!(content.contains("alpha"));
    }

    #[test]
    fn add_multiple_packages() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "manage",
                "pre-release",
                "--add",
                "crate-a:alpha",
                "--add",
                "crate-b:beta",
            ])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Added crate-a"))
            .stdout(contains("Added crate-b"));

        let content = fs::read_to_string(workspace.path().join(".changeset/pre-release.toml"))
            .expect("read file");
        assert!(content.contains("crate-a"));
        assert!(content.contains("alpha"));
        assert!(content.contains("crate-b"));
        assert!(content.contains("beta"));
    }

    #[test]
    fn add_updates_existing_tag() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release", "--add", "crate-a:alpha"])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release", "--add", "crate-a:beta"])
            .current_dir(workspace.path())
            .assert()
            .success();

        let content = fs::read_to_string(workspace.path().join(".changeset/pre-release.toml"))
            .expect("read file");
        assert!(content.contains("beta"));
        assert!(!content.contains("alpha"));
    }

    #[test]
    fn remove_entry() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "manage",
                "pre-release",
                "--add",
                "crate-a:alpha",
                "--add",
                "crate-b:beta",
            ])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release", "--remove", "crate-a"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Removed crate-a"));

        let content = fs::read_to_string(workspace.path().join(".changeset/pre-release.toml"))
            .expect("read file");
        assert!(!content.contains("crate-a"));
        assert!(content.contains("crate-b"));
    }

    #[test]
    fn remove_last_entry_deletes_file() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release", "--add", "crate-a:alpha"])
            .current_dir(workspace.path())
            .assert()
            .success();

        let prerelease_path = workspace.path().join(".changeset/pre-release.toml");
        assert!(prerelease_path.exists());

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release", "--remove", "crate-a"])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert!(
            !prerelease_path.exists(),
            "pre-release.toml should be deleted when empty"
        );
    }

    #[test]
    fn remove_nonexistent_silently_succeeds() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release", "--remove", "nonexistent"])
            .current_dir(workspace.path())
            .assert()
            .success();
    }

    #[test]
    fn list_shows_empty_state() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release", "--list"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("No packages in pre-release mode"));
    }

    #[test]
    fn list_shows_configured_packages() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release", "--add", "crate-a:alpha"])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release", "--list"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Pre-release configuration"))
            .stdout(contains("crate-a: alpha"));
    }

    #[test]
    fn no_args_in_non_tty_fails() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release"])
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("interactive mode requires a terminal"));
    }

    #[test]
    fn graduate_moves_to_graduation_queue() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release", "--add", "crate-a:alpha"])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release", "--graduate", "crate-a"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Moved crate-a to graduation queue"));

        let prerelease_path = workspace.path().join(".changeset/pre-release.toml");
        assert!(
            !prerelease_path.exists(),
            "pre-release.toml should be deleted"
        );

        let graduation_path = workspace.path().join(".changeset/graduation.toml");
        assert!(graduation_path.exists(), "graduation.toml should exist");

        let content = fs::read_to_string(&graduation_path).expect("read graduation.toml");
        assert!(content.contains("crate-a"));
    }

    #[test]
    fn add_with_invalid_format_fails() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release", "--add", "no-colon-here"])
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("invalid pre-release format"));
    }

    #[test]
    fn add_with_unknown_package_fails() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release", "--add", "nonexistent:alpha"])
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("package 'nonexistent' not found"));
    }

    #[test]
    fn add_with_invalid_tag_fails() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release", "--add", "crate-a:alpha.1"])
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("invalid prerelease tag"));
    }

    #[test]
    fn graduate_prerelease_version_fails() {
        let workspace = create_workspace_with_prerelease_version();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release", "--graduate", "prerelease-crate"])
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("cannot graduate"))
            .stderr(contains("prerelease"));
    }

    #[test]
    fn graduate_stable_version_fails() {
        let workspace = create_workspace_with_stable_version();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release", "--graduate", "stable-crate"])
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("cannot graduate"))
            .stderr(contains("stable"));
    }
}

mod concurrent_manage_operations {
    use super::*;

    #[test]
    fn concurrent_manage_prerelease_and_graduation_operations() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release", "--add", "crate-a:alpha"])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "graduation", "--add", "crate-b"])
            .current_dir(workspace.path())
            .assert()
            .success();

        let prerelease_path = workspace.path().join(".changeset/pre-release.toml");
        assert!(prerelease_path.exists(), "pre-release.toml should exist");
        let prerelease_content =
            fs::read_to_string(&prerelease_path).expect("read pre-release.toml");
        assert!(
            prerelease_content.contains("crate-a"),
            "pre-release.toml should contain crate-a"
        );
        assert!(
            prerelease_content.contains("alpha"),
            "pre-release.toml should contain alpha tag"
        );

        let graduation_path = workspace.path().join(".changeset/graduation.toml");
        assert!(graduation_path.exists(), "graduation.toml should exist");
        let graduation_content =
            fs::read_to_string(&graduation_path).expect("read graduation.toml");
        assert!(
            graduation_content.contains("crate-b"),
            "graduation.toml should contain crate-b"
        );

        assert!(
            !prerelease_content.contains("crate-b"),
            "crate-b should NOT be in pre-release.toml"
        );
        assert!(
            !graduation_content.contains("crate-a"),
            "crate-a should NOT be in graduation.toml"
        );
    }
}

mod manage_graduation {
    use super::*;

    #[test]
    fn add_creates_graduation_toml() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "graduation", "--add", "crate-a"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Added crate-a to graduation queue"));

        let graduation_path = workspace.path().join(".changeset/graduation.toml");
        assert!(graduation_path.exists(), "graduation.toml should exist");

        let content = fs::read_to_string(&graduation_path).expect("read graduation.toml");
        assert!(content.contains("crate-a"));
    }

    #[test]
    fn add_multiple_packages() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "manage",
                "graduation",
                "--add",
                "crate-a",
                "--add",
                "crate-b",
            ])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Added crate-a"))
            .stdout(contains("Added crate-b"));

        let content = fs::read_to_string(workspace.path().join(".changeset/graduation.toml"))
            .expect("read file");
        assert!(content.contains("crate-a"));
        assert!(content.contains("crate-b"));
    }

    #[test]
    fn add_duplicate_is_idempotent() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "graduation", "--add", "crate-a"])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "graduation", "--add", "crate-a"])
            .current_dir(workspace.path())
            .assert()
            .success();

        let content = fs::read_to_string(workspace.path().join(".changeset/graduation.toml"))
            .expect("read file");
        let count = content.matches("crate-a").count();
        assert_eq!(count, 1, "crate-a should appear only once");
    }

    #[test]
    fn remove_entry() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args([
                "manage",
                "graduation",
                "--add",
                "crate-a",
                "--add",
                "crate-b",
            ])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "graduation", "--remove", "crate-a"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Removed crate-a"));

        let content = fs::read_to_string(workspace.path().join(".changeset/graduation.toml"))
            .expect("read file");
        assert!(!content.contains("crate-a"));
        assert!(content.contains("crate-b"));
    }

    #[test]
    fn remove_last_entry_deletes_file() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "graduation", "--add", "crate-a"])
            .current_dir(workspace.path())
            .assert()
            .success();

        let graduation_path = workspace.path().join(".changeset/graduation.toml");
        assert!(graduation_path.exists());

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "graduation", "--remove", "crate-a"])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert!(
            !graduation_path.exists(),
            "graduation.toml should be deleted when empty"
        );
    }

    #[test]
    fn remove_nonexistent_silently_succeeds() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "graduation", "--remove", "nonexistent"])
            .current_dir(workspace.path())
            .assert()
            .success();
    }

    #[test]
    fn list_shows_empty_state() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "graduation", "--list"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("No packages queued for graduation"));
    }

    #[test]
    fn list_shows_queued_packages() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "graduation", "--add", "crate-a"])
            .current_dir(workspace.path())
            .assert()
            .success();

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "graduation", "--list"])
            .current_dir(workspace.path())
            .assert()
            .success()
            .stdout(contains("Graduation queue"))
            .stdout(contains("crate-a"));
    }

    #[test]
    fn no_args_in_non_tty_fails() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "graduation"])
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("interactive mode requires a terminal"));
    }

    #[test]
    fn add_with_unknown_package_fails() {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "graduation", "--add", "nonexistent"])
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("package 'nonexistent' not found"));
    }

    #[test]
    fn add_prerelease_version_fails() {
        let workspace = create_workspace_with_prerelease_version();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "graduation", "--add", "prerelease-crate"])
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("cannot graduate"))
            .stderr(contains("prerelease"));
    }

    #[test]
    fn add_stable_version_fails() {
        let workspace = create_workspace_with_stable_version();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");

        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "graduation", "--add", "stable-crate"])
            .current_dir(workspace.path())
            .assert()
            .failure()
            .stderr(contains("cannot graduate"))
            .stderr(contains("stable"));
    }
}

#[cfg(not(windows))]
mod interactive_prerelease_tests {
    use super::*;

    fn bin_path() -> PathBuf {
        assert_cmd::cargo::cargo_bin("cargo-changeset")
    }

    fn spawn_prerelease(workspace: &TempDir) -> TerminalSession {
        TerminalSession::spawn(&bin_path(), workspace, &["manage", "pre-release"])
    }

    fn workspace_with_changeset_dir() -> TempDir {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");
        workspace
    }

    #[test]
    fn interactive_prerelease_action_menu_rendering() {
        let workspace = workspace_with_changeset_dir();

        let mut session = spawn_prerelease(&workspace);
        session.wait_for("What would you like to do?");
        session.assert_screen(
            "prerelease action menu",
            indoc! {"
                What would you like to do?:
                > Add crate to pre-release
                  Remove crate from pre-release
                  Graduate crate (move to graduation queue)
                  Done"},
        );
        session.cancel();
        session.wait_for_exit();
    }

    #[test]
    fn interactive_prerelease_done_exits_cleanly() {
        let workspace = workspace_with_changeset_dir();

        let mut session = spawn_prerelease(&workspace);
        session.wait_for("What would you like to do?");
        session.assert_screen(
            "action menu before Done",
            indoc! {"
                What would you like to do?:
                > Add crate to pre-release
                  Remove crate from pre-release
                  Graduate crate (move to graduation queue)
                  Done"},
        );
        session.select_item(2);
        session.wait_for_exit();

        assert!(
            !workspace
                .path()
                .join(".changeset/pre-release.toml")
                .exists(),
            "pre-release.toml must not be created"
        );
    }

    #[test]
    fn interactive_prerelease_cancel_at_action_menu() {
        let workspace = workspace_with_changeset_dir();

        let mut session = spawn_prerelease(&workspace);
        session.wait_for("What would you like to do?");
        session.assert_screen(
            "action menu before cancel",
            indoc! {"
                What would you like to do?:
                > Add crate to pre-release
                  Remove crate from pre-release
                  Graduate crate (move to graduation queue)
                  Done"},
        );
        session.cancel();
        session.wait_for_exit();

        assert!(
            !workspace
                .path()
                .join(".changeset/pre-release.toml")
                .exists(),
            "pre-release.toml must not be created after cancel"
        );
    }

    #[test]
    fn interactive_prerelease_add_cancel_at_crate_selection() {
        let workspace = workspace_with_changeset_dir();

        let mut session = spawn_prerelease(&workspace);
        session.wait_for("What would you like to do?");
        session.assert_screen(
            "action menu",
            indoc! {"
                What would you like to do?:
                > Add crate to pre-release
                  Remove crate from pre-release
                  Graduate crate (move to graduation queue)
                  Done"},
        );
        session.confirm();
        session.wait_for("Select a crate to add to pre-release");
        session.assert_screen(
            "crate selection",
            indoc! {"
                What would you like to do?: Add crate to pre-release
                Select a crate to add to pre-release:
                  crate-a (0.1.0)
                  crate-b (0.2.0)"},
        );
        session.cancel();
        session.wait_for("> Add crate to pre-release");
        session.assert_screen(
            "action menu after cancel",
            indoc! {"
                What would you like to do?: Add crate to pre-release
                What would you like to do?:
                > Add crate to pre-release
                  Remove crate from pre-release
                  Graduate crate (move to graduation queue)
                  Done"},
        );
        session.cancel();
        session.wait_for_exit();

        assert!(
            !workspace
                .path()
                .join(".changeset/pre-release.toml")
                .exists(),
            "pre-release.toml must not be created after cancel"
        );
    }

    #[test]
    fn interactive_prerelease_add_full_flow() {
        let workspace = workspace_with_changeset_dir();

        let mut session = spawn_prerelease(&workspace);
        session.wait_for("What would you like to do?");
        session.assert_screen(
            "action menu",
            indoc! {"
                What would you like to do?:
                > Add crate to pre-release
                  Remove crate from pre-release
                  Graduate crate (move to graduation queue)
                  Done"},
        );
        session.confirm();
        session.wait_for("Select a crate to add to pre-release");
        session.assert_screen(
            "crate selection",
            indoc! {"
                What would you like to do?: Add crate to pre-release
                Select a crate to add to pre-release:
                  crate-a (0.1.0)
                  crate-b (0.2.0)"},
        );
        session.select_item(0);
        session.wait_for("Enter pre-release tag");
        session.assert_screen(
            "tag input",
            indoc! {"
                What would you like to do?: Add crate to pre-release
                Select a crate to add to pre-release: crate-a (0.1.0)
                Enter pre-release tag (e.g., alpha, beta, rc):"},
        );
        session.type_line("alpha");
        session.wait_for("> Add crate to pre-release");
        session.assert_screen(
            "action menu after add",
            indoc! {"
                What would you like to do?: Add crate to pre-release
                Select a crate to add to pre-release: crate-a (0.1.0)
                Enter pre-release tag (e.g., alpha, beta, rc): alpha
                What would you like to do?:
                > Add crate to pre-release
                  Remove crate from pre-release
                  Graduate crate (move to graduation queue)
                  Done"},
        );
        session.select_item(2);
        session.wait_for_exit();

        let path = workspace.path().join(".changeset/pre-release.toml");
        assert!(path.exists(), "pre-release.toml should be created");
        let content = fs::read_to_string(&path).expect("read pre-release.toml");
        assert!(content.contains("crate-a"));
        assert!(content.contains("alpha"));
    }

    #[test]
    fn interactive_prerelease_remove_full_flow() {
        let workspace = workspace_with_changeset_dir();
        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release", "--add", "crate-a:alpha"])
            .current_dir(workspace.path())
            .assert()
            .success();

        let mut session = spawn_prerelease(&workspace);
        session.wait_for("> Add crate to pre-release");
        session.assert_screen(
            "action menu",
            indoc! {"
                What would you like to do?:
                > Add crate to pre-release
                  Remove crate from pre-release
                  Graduate crate (move to graduation queue)
                  Done"},
        );
        session.select_item(0);
        session.wait_for("Select a crate to remove from pre-release");
        session.assert_screen(
            "crate selection",
            indoc! {"
                What would you like to do?: Remove crate from pre-release
                Select a crate to remove from pre-release:
                  crate-a: alpha"},
        );
        session.select_item(0);
        session.wait_for("> Add crate to pre-release");
        session.assert_screen(
            "action menu after remove",
            indoc! {"
                What would you like to do?: Remove crate from pre-release
                Select a crate to remove from pre-release: crate-a: alpha
                What would you like to do?:
                > Add crate to pre-release
                  Remove crate from pre-release
                  Graduate crate (move to graduation queue)
                  Done"},
        );
        session.select_item(2);
        session.wait_for_exit();

        assert!(
            !workspace
                .path()
                .join(".changeset/pre-release.toml")
                .exists(),
            "pre-release.toml should be deleted after removing last entry"
        );
    }

    #[test]
    fn interactive_prerelease_graduate_flow() {
        let workspace = workspace_with_changeset_dir();
        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "pre-release", "--add", "crate-a:alpha"])
            .current_dir(workspace.path())
            .assert()
            .success();

        let mut session = spawn_prerelease(&workspace);
        session.wait_for("> Add crate to pre-release");
        session.assert_screen(
            "action menu",
            indoc! {"
                What would you like to do?:
                > Add crate to pre-release
                  Remove crate from pre-release
                  Graduate crate (move to graduation queue)
                  Done"},
        );
        session.select_item(1);
        session.wait_for("Select a crate to graduate (move to graduation queue)");
        session.assert_screen(
            "crate selection",
            indoc! {"
                What would you like to do?: Graduate crate (move to graduation queue)
                Select a crate to graduate (move to graduation queue):
                  crate-a (0.1.0)
                  crate-b (0.2.0)"},
        );
        session.select_item(0);
        session.wait_for("> Add crate to pre-release");
        session.assert_screen(
            "action menu after graduate",
            indoc! {"
                What would you like to do?: Graduate crate (move to graduation queue)
                Select a crate to graduate (move to graduation queue): crate-a (0.1.0)
                What would you like to do?:
                > Add crate to pre-release
                  Remove crate from pre-release
                  Graduate crate (move to graduation queue)
                  Done"},
        );
        session.select_item(2);
        session.wait_for_exit();

        assert!(
            !workspace
                .path()
                .join(".changeset/pre-release.toml")
                .exists(),
            "pre-release.toml should be removed after graduation"
        );
        let graduation_path = workspace.path().join(".changeset/graduation.toml");
        assert!(
            graduation_path.exists(),
            "graduation.toml should be created"
        );
        let content = fs::read_to_string(&graduation_path).expect("read graduation.toml");
        assert!(content.contains("crate-a"));
    }

    #[test]
    fn interactive_prerelease_remove_no_packages_shows_message() {
        let workspace = workspace_with_changeset_dir();

        let mut session = spawn_prerelease(&workspace);
        session.wait_for("What would you like to do?");
        session.assert_screen(
            "action menu",
            indoc! {"
                What would you like to do?:
                > Add crate to pre-release
                  Remove crate from pre-release
                  Graduate crate (move to graduation queue)
                  Done"},
        );
        session.select_item(0);
        session.wait_for("What would you like to do?");
        session.assert_screen(
            "action menu after remove attempt",
            indoc! {"
                What would you like to do?: Remove crate from pre-release
                What would you like to do?:
                > Add crate to pre-release
                  Remove crate from pre-release
                  Graduate crate (move to graduation queue)
                  Done"},
        );
        session.select_item(2);
        session.wait_for("No packages are currently in pre-release mode");
        session.wait_for_exit();
    }
}

#[cfg(not(windows))]
mod interactive_graduation_tests {
    use super::*;

    fn bin_path() -> PathBuf {
        assert_cmd::cargo::cargo_bin("cargo-changeset")
    }

    fn spawn_graduation(workspace: &TempDir) -> TerminalSession {
        TerminalSession::spawn(&bin_path(), workspace, &["manage", "graduation"])
    }

    fn workspace_with_changeset_dir() -> TempDir {
        let workspace = create_virtual_workspace();
        fs::create_dir_all(workspace.path().join(".changeset")).expect("create changeset dir");
        workspace
    }

    #[test]
    fn interactive_graduation_action_menu_rendering() {
        let workspace = workspace_with_changeset_dir();

        let mut session = spawn_graduation(&workspace);
        session.wait_for("What would you like to do?");
        session.assert_screen(
            "graduation action menu",
            indoc! {"
                What would you like to do?:
                > Add crate to graduation queue
                  Remove crate from graduation queue
                  Done"},
        );
        session.cancel();
        session.wait_for_exit();
    }

    #[test]
    fn interactive_graduation_done_exits_cleanly() {
        let workspace = workspace_with_changeset_dir();

        let mut session = spawn_graduation(&workspace);
        session.wait_for("What would you like to do?");
        session.assert_screen(
            "action menu before Done",
            indoc! {"
                What would you like to do?:
                > Add crate to graduation queue
                  Remove crate from graduation queue
                  Done"},
        );
        session.select_item(1);
        session.wait_for_exit();

        assert!(
            !workspace.path().join(".changeset/graduation.toml").exists(),
            "graduation.toml must not be created"
        );
    }

    #[test]
    fn interactive_graduation_cancel_at_action_menu() {
        let workspace = workspace_with_changeset_dir();

        let mut session = spawn_graduation(&workspace);
        session.wait_for("What would you like to do?");
        session.assert_screen(
            "action menu before cancel",
            indoc! {"
                What would you like to do?:
                > Add crate to graduation queue
                  Remove crate from graduation queue
                  Done"},
        );
        session.cancel();
        session.wait_for_exit();

        assert!(
            !workspace.path().join(".changeset/graduation.toml").exists(),
            "graduation.toml must not be created after cancel"
        );
    }

    #[test]
    fn interactive_graduation_add_cancel_at_crate_selection() {
        let workspace = workspace_with_changeset_dir();

        let mut session = spawn_graduation(&workspace);
        session.wait_for("What would you like to do?");
        session.assert_screen(
            "action menu",
            indoc! {"
                What would you like to do?:
                > Add crate to graduation queue
                  Remove crate from graduation queue
                  Done"},
        );
        session.confirm();
        session.wait_for("Select a crate to graduate (move to graduation queue)");
        session.assert_screen(
            "crate selection",
            indoc! {"
                What would you like to do?: Add crate to graduation queue
                Select a crate to graduate (move to graduation queue):
                  crate-a (0.1.0)
                  crate-b (0.2.0)"},
        );
        session.cancel();
        session.wait_for("> Add crate to graduation queue");
        session.assert_screen(
            "action menu after cancel",
            indoc! {"
                What would you like to do?: Add crate to graduation queue
                What would you like to do?:
                > Add crate to graduation queue
                  Remove crate from graduation queue
                  Done"},
        );
        session.cancel();
        session.wait_for_exit();

        assert!(
            !workspace.path().join(".changeset/graduation.toml").exists(),
            "graduation.toml must not be created after cancel"
        );
    }

    #[test]
    fn interactive_graduation_add_full_flow() {
        let workspace = workspace_with_changeset_dir();

        let mut session = spawn_graduation(&workspace);
        session.wait_for("What would you like to do?");
        session.assert_screen(
            "action menu",
            indoc! {"
                What would you like to do?:
                > Add crate to graduation queue
                  Remove crate from graduation queue
                  Done"},
        );
        session.confirm();
        session.wait_for("Select a crate to graduate (move to graduation queue)");
        session.assert_screen(
            "crate selection",
            indoc! {"
                What would you like to do?: Add crate to graduation queue
                Select a crate to graduate (move to graduation queue):
                  crate-a (0.1.0)
                  crate-b (0.2.0)"},
        );
        session.select_item(0);
        session.wait_for("> Add crate to graduation queue");
        session.assert_screen(
            "action menu after add",
            indoc! {"
                What would you like to do?: Add crate to graduation queue
                Select a crate to graduate (move to graduation queue): crate-a (0.1.0)
                What would you like to do?:
                > Add crate to graduation queue
                  Remove crate from graduation queue
                  Done"},
        );
        session.select_item(1);
        session.wait_for_exit();

        let path = workspace.path().join(".changeset/graduation.toml");
        assert!(path.exists(), "graduation.toml should be created");
        let content = fs::read_to_string(&path).expect("read graduation.toml");
        assert!(content.contains("crate-a"));
    }

    #[test]
    fn interactive_graduation_remove_full_flow() {
        let workspace = workspace_with_changeset_dir();
        assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
            .args(["manage", "graduation", "--add", "crate-a"])
            .current_dir(workspace.path())
            .assert()
            .success();

        let mut session = spawn_graduation(&workspace);
        session.wait_for("> Add crate to graduation queue");
        session.assert_screen(
            "action menu",
            indoc! {"
                What would you like to do?:
                > Add crate to graduation queue
                  Remove crate from graduation queue
                  Done"},
        );
        session.select_item(0);
        session.wait_for("Select a crate to remove from graduation queue");
        session.assert_screen(
            "crate selection",
            indoc! {"
                What would you like to do?: Remove crate from graduation queue
                Select a crate to remove from graduation queue:
                  crate-a"},
        );
        session.select_item(0);
        session.wait_for("> Add crate to graduation queue");
        session.assert_screen(
            "action menu after remove",
            indoc! {"
                What would you like to do?: Remove crate from graduation queue
                Select a crate to remove from graduation queue: crate-a
                What would you like to do?:
                > Add crate to graduation queue
                  Remove crate from graduation queue
                  Done"},
        );
        session.select_item(1);
        session.wait_for_exit();

        assert!(
            !workspace.path().join(".changeset/graduation.toml").exists(),
            "graduation.toml should be deleted after removing last entry"
        );
    }

    #[test]
    fn interactive_graduation_remove_no_packages_shows_message() {
        let workspace = workspace_with_changeset_dir();

        let mut session = spawn_graduation(&workspace);
        session.wait_for("What would you like to do?");
        session.assert_screen(
            "action menu",
            indoc! {"
                What would you like to do?:
                > Add crate to graduation queue
                  Remove crate from graduation queue
                  Done"},
        );
        session.select_item(0);
        session.wait_for("What would you like to do?");
        session.assert_screen(
            "action menu after remove attempt",
            indoc! {"
                What would you like to do?: Remove crate from graduation queue
                What would you like to do?:
                > Add crate to graduation queue
                  Remove crate from graduation queue
                  Done"},
        );
        session.select_item(1);
        session.wait_for("No packages are currently queued for graduation");
        session.wait_for_exit();
    }
}
