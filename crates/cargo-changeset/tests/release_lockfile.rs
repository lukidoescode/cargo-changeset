use std::process::Command;

use changeset_test_helpers::changesets::write_changeset;
use changeset_test_helpers::git::{git_add_and_commit, lockfile_hash};
use changeset_test_helpers::workspaces::WorkspaceBuilder;
use tempfile::TempDir;

fn setup_workspace_with_dependency() -> TempDir {
    let dir = WorkspaceBuilder::virtual_workspace()
        .crate_member("crate-a", "1.0.0")
        .crate_member("crate-b", "2.0.0")
        .crate_toml_extra(
            "crate-b",
            "[dependencies]\ncrate-a = { workspace = true }\n",
        )
        .workspace_toml_extra(
            "[workspace.dependencies]\ncrate-a = { path = \"crates/crate-a\", version = \"1.0.0\" }\n",
        )
        .with_changeset_dir()
        .with_git()
        .build();

    let output = Command::new("cargo")
        .args(["generate-lockfile"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run cargo generate-lockfile");
    assert!(
        output.status.success(),
        "cargo generate-lockfile failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    git_add_and_commit(&dir, "Generate lockfile");

    dir
}

#[test]
fn release_lockfile_not_dirtied_by_cargo_check() {
    let workspace = setup_workspace_with_dependency();
    write_changeset(&workspace, "fix.md", "crate-a", "patch", "Fix a bug");
    git_add_and_commit(&workspace, "Add changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .success();

    let hash_before = lockfile_hash(&workspace);

    let check_output = Command::new("cargo")
        .args(["check"])
        .current_dir(workspace.path())
        .output()
        .expect("failed to run cargo check");
    assert!(
        check_output.status.success(),
        "cargo check failed: {}",
        String::from_utf8_lossy(&check_output.stderr)
    );

    let hash_after = lockfile_hash(&workspace);

    assert_eq!(
        hash_before, hash_after,
        "cargo check should not change Cargo.lock after release — \
         the release commit must include an up-to-date lockfile"
    );
}

#[test]
fn release_lockfile_correct_after_minor_bump_with_workspace_dep() {
    let workspace = setup_workspace_with_dependency();
    write_changeset(&workspace, "feat.md", "crate-a", "minor", "Add a feature");
    git_add_and_commit(&workspace, "Add changeset");

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("release")
        .current_dir(workspace.path())
        .assert()
        .success();

    let hash_before = lockfile_hash(&workspace);

    let check_output = Command::new("cargo")
        .args(["check"])
        .current_dir(workspace.path())
        .output()
        .expect("failed to run cargo check");
    assert!(
        check_output.status.success(),
        "cargo check failed: {}",
        String::from_utf8_lossy(&check_output.stderr)
    );

    let hash_after = lockfile_hash(&workspace);

    assert_eq!(
        hash_before, hash_after,
        "cargo check should not change Cargo.lock after minor bump release"
    );
}
