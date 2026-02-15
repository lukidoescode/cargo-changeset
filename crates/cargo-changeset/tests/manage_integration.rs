use std::fs;

use predicates::str::contains;
use tempfile::TempDir;

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

fn create_workspace_with_stable_version() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    fs::create_dir_all(dir.path().join("crates/stable/src"))
        .expect("failed to create stable crate dir");

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
        dir.path().join("crates/stable/Cargo.toml"),
        r#"
[package]
name = "stable-crate"
version = "1.2.3"
edition = "2021"
"#,
    )
    .expect("failed to write stable-crate Cargo.toml");

    fs::write(dir.path().join("crates/stable/src/lib.rs"), "")
        .expect("failed to write stable-crate lib.rs");

    dir
}

fn create_workspace_with_prerelease_version() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    fs::create_dir_all(dir.path().join("crates/pre/src"))
        .expect("failed to create prerelease crate dir");

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
        dir.path().join("crates/pre/Cargo.toml"),
        r#"
[package]
name = "prerelease-crate"
version = "0.1.0-alpha.1"
edition = "2021"
"#,
    )
    .expect("failed to write prerelease-crate Cargo.toml");

    fs::write(dir.path().join("crates/pre/src/lib.rs"), "")
        .expect("failed to write prerelease-crate lib.rs");

    dir
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
