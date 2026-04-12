mod common;

use std::fs;
use std::fs::OpenOptions;
use std::io::Write as IoWrite;

use predicates::prelude::PredicateBooleanExt as _;
use predicates::str::contains;
use tempfile::TempDir;

use common::changesets::write_multi_changeset;
use common::git::{create_branch, git_add_and_commit, init_git_repo};
use common::workspaces::add_helm_chart_config;

fn create_virtual_workspace_with_git() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    init_git_repo(&dir);

    fs::create_dir_all(dir.path().join("crates/crate-a/src"))
        .expect("failed to create crate a dir");
    fs::create_dir_all(dir.path().join("crates/crate-b/src"))
        .expect("failed to create crate b dir");

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
        dir.path().join("crates/crate-a/Cargo.toml"),
        r#"
[package]
name = "crate-a"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("failed to write crate-a Cargo.toml");

    fs::write(dir.path().join("crates/crate-a/src/lib.rs"), "")
        .expect("failed to write crate-a lib.rs");

    fs::write(
        dir.path().join("crates/crate-b/Cargo.toml"),
        r#"
[package]
name = "crate-b"
version = "0.2.0"
edition = "2021"
"#,
    )
    .expect("failed to write crate-b Cargo.toml");

    fs::write(dir.path().join("crates/crate-b/src/lib.rs"), "")
        .expect("failed to write crate-b lib.rs");

    git_add_and_commit(&dir, "Initial commit");

    dir
}

fn add_changeset(dir: &TempDir, package_name: &str) {
    add_changeset_with_name(dir, package_name, &format!("{package_name}-changeset"));
}

fn add_changeset_with_name(dir: &TempDir, package_name: &str, changeset_name: &str) {
    fs::create_dir_all(dir.path().join(".changeset/changesets"))
        .expect("failed to create .changeset/changesets dir");
    let filename = format!(".changeset/changesets/{changeset_name}.md");
    fs::write(
        dir.path().join(&filename),
        format!(
            r#"---
"{package_name}": patch
---

Test changeset for {package_name}.
"#
        ),
    )
    .expect("failed to write changeset");
}

fn create_workspace_with_three_crates() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    init_git_repo(&dir);

    fs::create_dir_all(dir.path().join("crates/crate-a/src"))
        .expect("failed to create crate a dir");
    fs::create_dir_all(dir.path().join("crates/crate-b/src"))
        .expect("failed to create crate b dir");
    fs::create_dir_all(dir.path().join("crates/crate-c/src"))
        .expect("failed to create crate c dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"
[workspace]
members = ["crates/*"]
resolver = "2"
"#,
    )
    .expect("failed to write workspace Cargo.toml");

    for (name, version) in [
        ("crate-a", "0.1.0"),
        ("crate-b", "0.2.0"),
        ("crate-c", "0.3.0"),
    ] {
        fs::write(
            dir.path().join(format!("crates/{name}/Cargo.toml")),
            format!(
                r#"
[package]
name = "{name}"
version = "{version}"
edition = "2021"
"#
            ),
        )
        .expect("failed to write Cargo.toml");

        fs::write(dir.path().join(format!("crates/{name}/src/lib.rs")), "")
            .expect("failed to write lib.rs");
    }

    git_add_and_commit(&dir, "Initial commit");

    dir
}

#[test]
fn verify_exit_code_0_when_all_changes_covered() {
    let workspace = create_virtual_workspace_with_git();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// changed",
    )
    .expect("failed to modify lib.rs");

    add_changeset(&workspace, "crate-a");
    git_add_and_commit(&workspace, "Add changes with changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_exit_code_1_when_package_uncovered() {
    let workspace = create_virtual_workspace_with_git();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// changed",
    )
    .expect("failed to modify lib.rs");

    git_add_and_commit(&workspace, "Add changes without changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("crate-a"))
        .stderr(contains("without changeset coverage"));
}

#[test]
fn verify_exit_code_0_when_only_changeset_directory_changes() {
    let workspace = create_virtual_workspace_with_git();
    create_branch(&workspace, "feature");

    add_changeset(&workspace, "crate-a");
    git_add_and_commit(&workspace, "Add changeset only");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_exit_code_0_for_workspace_ignored_files() {
    let dir = TempDir::new().expect("failed to create temp dir");

    init_git_repo(&dir);

    fs::create_dir_all(dir.path().join("crates/my-crate/src")).expect("failed to create crate dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.metadata.changeset]
ignored-files = ["*.md", "docs/**"]
"#,
    )
    .expect("failed to write workspace Cargo.toml");

    fs::write(
        dir.path().join("crates/my-crate/Cargo.toml"),
        r#"
[package]
name = "my-crate"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("failed to write crate Cargo.toml");

    fs::write(dir.path().join("crates/my-crate/src/lib.rs"), "").expect("failed to write lib.rs");

    git_add_and_commit(&dir, "Initial commit");
    create_branch(&dir, "feature");

    fs::write(dir.path().join("README.md"), "# README").expect("failed to write README.md");
    git_add_and_commit(&dir, "Add README");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn verify_exit_code_0_for_package_ignored_files() {
    let dir = TempDir::new().expect("failed to create temp dir");

    init_git_repo(&dir);

    fs::create_dir_all(dir.path().join("crates/my-crate/src")).expect("failed to create crate dir");
    fs::create_dir_all(dir.path().join("crates/my-crate/benches"))
        .expect("failed to create benches dir");

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
        dir.path().join("crates/my-crate/Cargo.toml"),
        r#"
[package]
name = "my-crate"
version = "0.1.0"
edition = "2021"

[package.metadata.changeset]
ignored-files = ["benches/**"]
"#,
    )
    .expect("failed to write crate Cargo.toml");

    fs::write(dir.path().join("crates/my-crate/src/lib.rs"), "").expect("failed to write lib.rs");

    fs::write(dir.path().join("crates/my-crate/benches/bench.rs"), "")
        .expect("failed to write bench.rs");

    git_add_and_commit(&dir, "Initial commit");
    create_branch(&dir, "feature");

    fs::write(
        dir.path().join("crates/my-crate/benches/bench.rs"),
        "// updated benchmark",
    )
    .expect("failed to update bench.rs");
    git_add_and_commit(&dir, "Update benchmark");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn verify_exit_code_1_for_nonexistent_base_branch() {
    let workspace = create_virtual_workspace_with_git();

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("nonexistent-branch")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("failed to resolve reference"));
}

#[test]
fn verify_default_output_shows_details() {
    let workspace = create_virtual_workspace_with_git();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// changed",
    )
    .expect("failed to modify lib.rs");

    add_changeset(&workspace, "crate-a");
    git_add_and_commit(&workspace, "Add changes with changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .success()
        .stdout(contains("Changed packages"))
        .stdout(contains("crate-a"));
}

#[test]
fn verify_with_custom_base_branch() {
    let workspace = create_virtual_workspace_with_git();

    create_branch(&workspace, "develop");
    git_add_and_commit(&workspace, "Develop base");

    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-b/src/lib.rs"),
        "// changed",
    )
    .expect("failed to modify lib.rs");

    add_changeset(&workspace, "crate-b");
    git_add_and_commit(&workspace, "Add changes with changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("develop")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_no_changes_passes() {
    let workspace = create_virtual_workspace_with_git();
    create_branch(&workspace, "feature");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_multiple_packages_one_uncovered() {
    let workspace = create_virtual_workspace_with_git();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// changed a",
    )
    .expect("failed to modify crate-a lib.rs");

    fs::write(
        workspace.path().join("crates/crate-b/src/lib.rs"),
        "// changed b",
    )
    .expect("failed to modify crate-b lib.rs");

    add_changeset(&workspace, "crate-a");
    git_add_and_commit(&workspace, "Add changes with partial changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("crate-b"))
        .stderr(contains("without changeset coverage"));
}

#[test]
fn verify_project_level_changes_pass_without_changeset() {
    let workspace = create_virtual_workspace_with_git();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("Cargo.toml"),
        r#"
[workspace]
members = ["crates/*"]
resolver = "2"

# Added a comment
"#,
    )
    .expect("failed to modify workspace Cargo.toml");

    git_add_and_commit(&workspace, "Update workspace Cargo.toml");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_fails_on_malformed_changeset() {
    let workspace = create_virtual_workspace_with_git();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// changed",
    )
    .expect("failed to modify lib.rs");

    fs::create_dir_all(workspace.path().join(".changeset/changesets"))
        .expect("failed to create .changeset/changesets");
    fs::write(
        workspace.path().join(".changeset/changesets/malformed.md"),
        r#"---
invalid yaml {{{ not closed
---

This changeset is malformed.
"#,
    )
    .expect("failed to write malformed changeset");

    git_add_and_commit(&workspace, "Add malformed changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("failed to parse changeset file"));
}

#[test]
fn verify_quiet_suppresses_output_on_success() {
    let workspace = create_virtual_workspace_with_git();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// changed",
    )
    .expect("failed to modify lib.rs");

    add_changeset(&workspace, "crate-a");
    git_add_and_commit(&workspace, "Add changes with changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .arg("--quiet")
        .current_dir(workspace.path())
        .assert()
        .success()
        .stdout(predicates::str::is_empty())
        .stderr(predicates::str::is_empty());
}

#[test]
fn verify_quiet_suppresses_output_on_failure() {
    let workspace = create_virtual_workspace_with_git();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// changed",
    )
    .expect("failed to modify lib.rs");

    git_add_and_commit(&workspace, "Add changes without changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .arg("--quiet")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stdout(predicates::str::is_empty())
        .stderr(predicates::str::is_empty());
}

#[test]
fn verify_preexisting_changeset_does_not_cover_new_changes() {
    let workspace = create_virtual_workspace_with_git();

    add_changeset(&workspace, "crate-a");
    git_add_and_commit(&workspace, "Add changeset for crate-a");

    create_branch(&workspace, "feature");
    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// new changes",
    )
    .expect("failed to modify lib.rs");
    git_add_and_commit(&workspace, "Modify crate-a");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .arg("--quiet")
        .current_dir(workspace.path())
        .assert()
        .failure();
}

#[test]
fn verify_changeset_in_same_branch_covers_changes() {
    let workspace = create_virtual_workspace_with_git();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// new changes",
    )
    .expect("failed to modify lib.rs");
    add_changeset(&workspace, "crate-a");
    git_add_and_commit(&workspace, "Add changes with changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .arg("--quiet")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_deleted_changeset_fails() {
    let workspace = create_virtual_workspace_with_git();

    add_changeset(&workspace, "crate-a");
    git_add_and_commit(&workspace, "Add changeset");

    create_branch(&workspace, "feature");
    fs::remove_file(
        workspace
            .path()
            .join(".changeset/changesets/crate-a-changeset.md"),
    )
    .expect("failed to delete changeset");
    git_add_and_commit(&workspace, "Delete changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("deleted"))
        .stderr(contains("--allow-deleted-changesets"));
}

#[test]
fn verify_deleted_changeset_with_allow_flag_passes() {
    let workspace = create_virtual_workspace_with_git();

    add_changeset(&workspace, "crate-a");
    git_add_and_commit(&workspace, "Add changeset");

    create_branch(&workspace, "feature");
    fs::remove_file(
        workspace
            .path()
            .join(".changeset/changesets/crate-a-changeset.md"),
    )
    .expect("failed to delete changeset");
    git_add_and_commit(&workspace, "Delete changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .arg("--allow-deleted-changesets")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_multiple_commits_on_feature_branch_all_covered() {
    let workspace = create_workspace_with_three_crates();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// commit 1",
    )
    .expect("failed to modify lib.rs");
    add_changeset_with_name(&workspace, "crate-a", "first-change");
    git_add_and_commit(&workspace, "First commit with changeset");

    fs::write(
        workspace.path().join("crates/crate-b/src/lib.rs"),
        "// commit 2",
    )
    .expect("failed to modify lib.rs");
    add_changeset_with_name(&workspace, "crate-b", "second-change");
    git_add_and_commit(&workspace, "Second commit with changeset");

    fs::write(
        workspace.path().join("crates/crate-c/src/lib.rs"),
        "// commit 3",
    )
    .expect("failed to modify lib.rs");
    add_changeset_with_name(&workspace, "crate-c", "third-change");
    git_add_and_commit(&workspace, "Third commit with changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .arg("--quiet")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_multiple_commits_on_feature_branch_last_uncovered() {
    let workspace = create_workspace_with_three_crates();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// commit 1",
    )
    .expect("failed to modify lib.rs");
    add_changeset_with_name(&workspace, "crate-a", "first-change");
    git_add_and_commit(&workspace, "First commit with changeset");

    fs::write(
        workspace.path().join("crates/crate-b/src/lib.rs"),
        "// commit 2",
    )
    .expect("failed to modify lib.rs");
    add_changeset_with_name(&workspace, "crate-b", "second-change");
    git_add_and_commit(&workspace, "Second commit with changeset");

    fs::write(
        workspace.path().join("crates/crate-c/src/lib.rs"),
        "// commit 3 - no changeset",
    )
    .expect("failed to modify lib.rs");
    git_add_and_commit(&workspace, "Third commit without changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("crate-c"));
}

#[test]
fn verify_main_has_unreleased_changesets_feature_has_different_changes() {
    let workspace = create_workspace_with_three_crates();

    add_changeset_with_name(&workspace, "crate-a", "unreleased-on-main");
    git_add_and_commit(&workspace, "Add unreleased changeset for crate-a on main");

    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-b/src/lib.rs"),
        "// feature work",
    )
    .expect("failed to modify lib.rs");
    add_changeset_with_name(&workspace, "crate-b", "feature-change");
    git_add_and_commit(&workspace, "Add feature change with changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .arg("--quiet")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_main_has_unreleased_changesets_feature_modifies_same_crate_without_new_changeset() {
    let workspace = create_workspace_with_three_crates();

    add_changeset_with_name(&workspace, "crate-a", "unreleased-on-main");
    git_add_and_commit(&workspace, "Add unreleased changeset for crate-a on main");

    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// additional changes to crate-a",
    )
    .expect("failed to modify lib.rs");
    git_add_and_commit(&workspace, "Modify crate-a without new changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .arg("--quiet")
        .current_dir(workspace.path())
        .assert()
        .failure();
}

#[test]
fn verify_feature_modifies_existing_changeset_from_main() {
    let workspace = create_workspace_with_three_crates();

    add_changeset_with_name(&workspace, "crate-a", "shared-changeset");
    git_add_and_commit(&workspace, "Add changeset on main");

    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// feature work on crate-a",
    )
    .expect("failed to modify lib.rs");

    fs::write(
        workspace
            .path()
            .join(".changeset/changesets/shared-changeset.md"),
        r#"---
"crate-a": minor
---

Updated description with additional changes.
"#,
    )
    .expect("failed to modify changeset");
    git_add_and_commit(&workspace, "Modify changeset and code");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .arg("--quiet")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_single_changeset_covers_multiple_packages() {
    let workspace = create_workspace_with_three_crates();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// changed a",
    )
    .expect("failed to modify lib.rs");

    fs::write(
        workspace.path().join("crates/crate-b/src/lib.rs"),
        "// changed b",
    )
    .expect("failed to modify lib.rs");

    write_multi_changeset(
        &workspace,
        "multi-package-change.md",
        &[("crate-a", "patch"), ("crate-b", "patch")],
        "Test changeset for multiple packages.",
    );
    git_add_and_commit(&workspace, "Change multiple packages with single changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .arg("--quiet")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_single_changeset_covers_multiple_packages_but_misses_one() {
    let workspace = create_workspace_with_three_crates();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// changed a",
    )
    .expect("failed to modify lib.rs");

    fs::write(
        workspace.path().join("crates/crate-b/src/lib.rs"),
        "// changed b",
    )
    .expect("failed to modify lib.rs");

    fs::write(
        workspace.path().join("crates/crate-c/src/lib.rs"),
        "// changed c",
    )
    .expect("failed to modify lib.rs");

    write_multi_changeset(
        &workspace,
        "multi-package-change.md",
        &[("crate-a", "patch"), ("crate-b", "patch")],
        "Test changeset for multiple packages.",
    );
    git_add_and_commit(
        &workspace,
        "Change three packages but only two in changeset",
    );

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("crate-c"));
}

#[test]
fn verify_changeset_added_in_later_commit_covers_earlier_changes() {
    let workspace = create_workspace_with_three_crates();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// first change",
    )
    .expect("failed to modify lib.rs");
    git_add_and_commit(&workspace, "Change without changeset");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// second change",
    )
    .expect("failed to modify lib.rs");
    git_add_and_commit(&workspace, "More changes");

    add_changeset(&workspace, "crate-a");
    git_add_and_commit(&workspace, "Add changeset after changes");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .arg("--quiet")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_multiple_changesets_for_same_package() {
    let workspace = create_workspace_with_three_crates();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// big refactor",
    )
    .expect("failed to modify lib.rs");

    add_changeset_with_name(&workspace, "crate-a", "feature-one");
    add_changeset_with_name(&workspace, "crate-a", "feature-two");
    git_add_and_commit(&workspace, "Add multiple changesets for same package");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .arg("--quiet")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_code_changes_across_multiple_commits_single_changeset() {
    let workspace = create_workspace_with_three_crates();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// first batch",
    )
    .expect("failed to modify lib.rs");
    git_add_and_commit(&workspace, "First batch of changes");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// second batch",
    )
    .expect("failed to modify lib.rs");
    git_add_and_commit(&workspace, "Second batch of changes");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// final version",
    )
    .expect("failed to modify lib.rs");
    add_changeset(&workspace, "crate-a");
    git_add_and_commit(&workspace, "Final changes with changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .arg("--quiet")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_changeset_added_then_code_changed_later_still_covered() {
    let workspace = create_workspace_with_three_crates();
    create_branch(&workspace, "feature");

    add_changeset(&workspace, "crate-a");
    git_add_and_commit(&workspace, "Add changeset first");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// actual implementation",
    )
    .expect("failed to modify lib.rs");
    git_add_and_commit(&workspace, "Implement the change");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .arg("--quiet")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_mixed_scenario_some_from_main_some_new() {
    let workspace = create_workspace_with_three_crates();

    add_changeset_with_name(&workspace, "crate-a", "main-changeset-a");
    add_changeset_with_name(&workspace, "crate-b", "main-changeset-b");
    git_add_and_commit(&workspace, "Add changesets on main");

    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// modify crate-a",
    )
    .expect("failed to modify lib.rs");

    fs::write(
        workspace.path().join("crates/crate-c/src/lib.rs"),
        "// modify crate-c",
    )
    .expect("failed to modify lib.rs");

    add_changeset_with_name(&workspace, "crate-a", "feature-changeset-a");
    add_changeset_with_name(&workspace, "crate-c", "feature-changeset-c");
    git_add_and_commit(&workspace, "Modify a and c with new changesets");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .arg("--quiet")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_preexisting_changeset_for_different_package_does_not_help() {
    let workspace = create_workspace_with_three_crates();

    add_changeset(&workspace, "crate-a");
    git_add_and_commit(&workspace, "Add changeset for crate-a on main");

    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-b/src/lib.rs"),
        "// modify crate-b",
    )
    .expect("failed to modify lib.rs");
    git_add_and_commit(&workspace, "Modify crate-b without changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("crate-b"));
}

#[test]
fn verify_changeset_covers_package_not_actually_changed() {
    let workspace = create_workspace_with_three_crates();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// only crate-a changed",
    )
    .expect("failed to modify lib.rs");

    write_multi_changeset(
        &workspace,
        "overly-broad-changeset.md",
        &[("crate-a", "patch"), ("crate-b", "patch")],
        "Test changeset for multiple packages.",
    );
    git_add_and_commit(&workspace, "Changeset covers more than needed");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .arg("--quiet")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_feature_branch_deletes_code_needs_changeset() {
    let workspace = create_workspace_with_three_crates();

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "pub fn deprecated_function() {}",
    )
    .expect("failed to write lib.rs");
    git_add_and_commit(&workspace, "Add deprecated function");

    create_branch(&workspace, "feature");

    fs::write(workspace.path().join("crates/crate-a/src/lib.rs"), "")
        .expect("failed to clear lib.rs");
    git_add_and_commit(&workspace, "Remove deprecated function");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("crate-a"));
}

#[test]
fn verify_feature_branch_deletes_code_with_changeset_passes() {
    let workspace = create_workspace_with_three_crates();

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "pub fn deprecated_function() {}",
    )
    .expect("failed to write lib.rs");
    git_add_and_commit(&workspace, "Add deprecated function");

    create_branch(&workspace, "feature");

    fs::write(workspace.path().join("crates/crate-a/src/lib.rs"), "")
        .expect("failed to clear lib.rs");
    add_changeset(&workspace, "crate-a");
    git_add_and_commit(&workspace, "Remove deprecated function with changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .arg("--quiet")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_new_file_in_package_needs_changeset() {
    let workspace = create_workspace_with_three_crates();
    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/new_module.rs"),
        "pub fn new_function() {}",
    )
    .expect("failed to write new module");
    git_add_and_commit(&workspace, "Add new module without changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("crate-a"));
}

#[test]
fn verify_deleted_and_added_changeset_in_same_branch() {
    let workspace = create_workspace_with_three_crates();

    add_changeset_with_name(&workspace, "crate-a", "old-changeset");
    git_add_and_commit(&workspace, "Add old changeset");

    create_branch(&workspace, "feature");

    fs::remove_file(
        workspace
            .path()
            .join(".changeset/changesets/old-changeset.md"),
    )
    .expect("failed to delete old changeset");
    add_changeset_with_name(&workspace, "crate-a", "new-changeset");
    git_add_and_commit(&workspace, "Replace old changeset with new one");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("deleted"));
}

#[test]
fn verify_deleted_and_added_changeset_with_allow_flag() {
    let workspace = create_workspace_with_three_crates();

    add_changeset_with_name(&workspace, "crate-a", "old-changeset");
    git_add_and_commit(&workspace, "Add old changeset");

    create_branch(&workspace, "feature");

    fs::write(
        workspace.path().join("crates/crate-a/src/lib.rs"),
        "// changes",
    )
    .expect("failed to modify lib.rs");

    fs::remove_file(
        workspace
            .path()
            .join(".changeset/changesets/old-changeset.md"),
    )
    .expect("failed to delete old changeset");
    add_changeset_with_name(&workspace, "crate-a", "new-changeset");
    git_add_and_commit(&workspace, "Replace old changeset with new one plus code");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .arg("--allow-deleted-changesets")
        .arg("--quiet")
        .current_dir(workspace.path())
        .assert()
        .success();
}

fn create_workspace_with_helm_chart_and_git() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"
"#,
    )
    .expect("failed to write workspace Cargo.toml");

    fs::create_dir_all(dir.path().join("crates/crate-a/src"))
        .expect("failed to create crate-a src dir");

    fs::write(dir.path().join("crates/crate-a/src/lib.rs"), "")
        .expect("failed to write crate-a lib.rs");

    fs::write(
        dir.path().join("crates/crate-a/Cargo.toml"),
        r#"[package]
name = "crate-a"
version = "1.0.0"
edition = "2021"
"#,
    )
    .expect("failed to write crate-a Cargo.toml");

    fs::create_dir_all(dir.path().join("charts/my-chart"))
        .expect("failed to create charts/my-chart dir");

    fs::write(
        dir.path().join("charts/my-chart/Chart.yaml"),
        r#"# Helm chart for my-chart
apiVersion: v2
name: my-chart
description: A test Helm chart
# This comment should survive release
version: "2.0.0"
appVersion: "1.0.0"
"#,
    )
    .expect("failed to write Chart.yaml");

    fs::write(
        dir.path().join("charts/my-chart/values.yaml"),
        "replicaCount: 1",
    )
    .expect("failed to write values.yaml");

    fs::create_dir_all(dir.path().join(".changeset/changesets"))
        .expect("failed to create .changeset/changesets dir");

    add_helm_chart_config(&dir);

    init_git_repo(&dir);
    git_add_and_commit(&dir, "Initial commit");

    dir
}

#[test]
fn verify_detects_additional_package_changes_requiring_coverage() {
    let workspace = create_workspace_with_helm_chart_and_git();
    create_branch(&workspace, "feature");

    let mut values = OpenOptions::new()
        .append(true)
        .open(workspace.path().join("charts/my-chart/values.yaml"))
        .expect("failed to open values.yaml for appending");
    writeln!(values, "image: nginx").expect("failed to append to values.yaml");

    git_add_and_commit(&workspace, "Modify helm values");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("my-helm-chart"));
}

#[test]
fn verify_passes_when_additional_package_change_covered() {
    let workspace = create_workspace_with_helm_chart_and_git();
    create_branch(&workspace, "feature");

    let mut values = OpenOptions::new()
        .append(true)
        .open(workspace.path().join("charts/my-chart/values.yaml"))
        .expect("failed to open values.yaml for appending");
    writeln!(values, "image: nginx").expect("failed to append to values.yaml");

    add_changeset(&workspace, "my-helm-chart");
    git_add_and_commit(&workspace, "Add changeset for helm chart");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_ignores_files_outside_additional_package_influence() {
    let workspace = create_workspace_with_helm_chart_and_git();

    fs::create_dir_all(workspace.path().join("docs")).expect("failed to create docs dir");
    fs::write(workspace.path().join("docs/README.md"), "# Documentation")
        .expect("failed to write README.md");
    git_add_and_commit(&workspace, "Add docs");

    create_branch(&workspace, "feature");

    let mut readme = OpenOptions::new()
        .append(true)
        .open(workspace.path().join("docs/README.md"))
        .expect("failed to open README.md for appending");
    writeln!(readme, "## Additional section").expect("failed to append to README.md");

    git_add_and_commit(&workspace, "Update docs");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn verify_both_rust_and_additional_package_uncovered() {
    let workspace = create_workspace_with_helm_chart_and_git();
    create_branch(&workspace, "feature");

    let mut lib = OpenOptions::new()
        .append(true)
        .open(workspace.path().join("crates/crate-a/src/lib.rs"))
        .expect("failed to open lib.rs for appending");
    writeln!(lib, "// changed").expect("failed to append to lib.rs");
    drop(lib);

    let mut values = OpenOptions::new()
        .append(true)
        .open(workspace.path().join("charts/my-chart/values.yaml"))
        .expect("failed to open values.yaml for appending");
    writeln!(values, "image: nginx").expect("failed to append to values.yaml");
    drop(values);

    git_add_and_commit(&workspace, "Modify both");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("crate-a"))
        .stderr(contains("my-helm-chart"));
}

#[test]
fn verify_mixed_coverage_only_rust_uncovered() {
    let workspace = create_workspace_with_helm_chart_and_git();
    create_branch(&workspace, "feature");

    let mut lib = OpenOptions::new()
        .append(true)
        .open(workspace.path().join("crates/crate-a/src/lib.rs"))
        .expect("failed to open lib.rs for appending");
    writeln!(lib, "// changed").expect("failed to append to lib.rs");
    drop(lib);

    let mut values = OpenOptions::new()
        .append(true)
        .open(workspace.path().join("charts/my-chart/values.yaml"))
        .expect("failed to open values.yaml for appending");
    writeln!(values, "image: nginx").expect("failed to append to values.yaml");
    drop(values);

    add_changeset(&workspace, "my-helm-chart");
    git_add_and_commit(&workspace, "Add helm changeset only");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .failure()
        .stderr(contains("crate-a"))
        .stderr(contains("✗ my-helm-chart").not());
}
