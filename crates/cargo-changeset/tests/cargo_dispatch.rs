use changeset_test_helpers::workspaces::WorkspaceBuilder;

#[test]
fn cargo_dispatch_verify_succeeds_with_changeset_prefix() {
    let workspace = WorkspaceBuilder::single_package("my-crate", "0.1.0")
        .with_git()
        .build();

    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("changeset")
        .arg("verify")
        .arg("--base")
        .arg("main")
        .current_dir(workspace.path())
        .assert()
        .success();
}

#[test]
fn cargo_dispatch_help_succeeds_with_changeset_prefix() {
    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("changeset")
        .arg("--help")
        .assert()
        .success();
}

#[test]
fn version_flag_succeeds() {
    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn cargo_dispatch_version_succeeds_with_changeset_prefix() {
    assert_cmd::cargo::cargo_bin_cmd!("cargo-changeset")
        .arg("changeset")
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::contains(env!("CARGO_PKG_VERSION")));
}
