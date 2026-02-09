use std::path::PathBuf;

use changeset_workspace::{WorkspaceKind, discover_workspace};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn create_temp_single_crate() -> tempfile::TempDir {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    std::fs::write(
        temp_dir.path().join("Cargo.toml"),
        r#"[package]
name = "single"
version = "3.0.0"
"#,
    )
    .expect("write cargo toml");
    temp_dir
}

#[test]
fn discovers_virtual_workspace_from_root() {
    let fixture = fixtures_dir().join("virtual_workspace");
    let workspace = discover_workspace(&fixture).expect("should discover workspace");

    assert_eq!(workspace.kind, WorkspaceKind::Virtual);
    assert_eq!(workspace.root, fixture.canonicalize().expect("path exists"));
    assert_eq!(workspace.packages.len(), 2);

    let names: Vec<_> = workspace.packages.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"crate-a"));
    assert!(names.contains(&"crate-b"));
}

#[test]
fn discovers_workspace_from_nested_member() {
    let fixture = fixtures_dir().join("virtual_workspace/crates/crate_a");
    let workspace = discover_workspace(&fixture).expect("should discover workspace");

    assert_eq!(workspace.kind, WorkspaceKind::Virtual);
    assert_eq!(
        workspace.root,
        fixtures_dir()
            .join("virtual_workspace")
            .canonicalize()
            .expect("path exists")
    );
    assert_eq!(workspace.packages.len(), 2);
}

#[test]
fn discovers_root_package_workspace() {
    let fixture = fixtures_dir().join("root_package_workspace");
    let workspace = discover_workspace(&fixture).expect("should discover workspace");

    assert_eq!(workspace.kind, WorkspaceKind::RootPackage);
    assert_eq!(workspace.packages.len(), 2);

    let root_pkg = workspace
        .packages
        .iter()
        .find(|p| p.name == "root-pkg")
        .expect("should have root package");
    assert_eq!(root_pkg.version.to_string(), "0.1.0");

    let member = workspace
        .packages
        .iter()
        .find(|p| p.name == "member")
        .expect("should have member package");
    assert_eq!(member.version.to_string(), "0.2.0");
}

#[test]
fn discovers_single_crate() {
    let temp_dir = create_temp_single_crate();
    let workspace = discover_workspace(temp_dir.path()).expect("should discover workspace");

    assert_eq!(workspace.kind, WorkspaceKind::SingleCrate);
    assert_eq!(workspace.packages.len(), 1);
    assert_eq!(workspace.packages[0].name, "single");
    assert_eq!(workspace.packages[0].version.to_string(), "3.0.0");
}

#[test]
fn discovers_from_deeply_nested_path() {
    let fixture = fixtures_dir().join("nested/packages/inner");
    let workspace = discover_workspace(&fixture).expect("should discover workspace");

    assert_eq!(workspace.kind, WorkspaceKind::Virtual);
    assert_eq!(
        workspace.root,
        fixtures_dir()
            .join("nested")
            .canonicalize()
            .expect("path exists")
    );
}

#[test]
fn version_inheritance_works() {
    let fixture = fixtures_dir().join("virtual_workspace");
    let workspace = discover_workspace(&fixture).expect("should discover workspace");

    let crate_a = workspace
        .packages
        .iter()
        .find(|p| p.name == "crate-a")
        .expect("should have crate-a");
    assert_eq!(crate_a.version.to_string(), "1.0.0");

    let crate_b = workspace
        .packages
        .iter()
        .find(|p| p.name == "crate-b")
        .expect("should have crate-b");
    assert_eq!(crate_b.version.to_string(), "2.0.0");
}

#[test]
fn not_found_error_for_nonexistent_path() {
    let result = discover_workspace(&PathBuf::from("/nonexistent/path"));
    assert!(result.is_err());
}

#[test]
fn ensure_changeset_dir_creates_directory() {
    let temp_dir = create_temp_single_crate();
    let workspace = discover_workspace(temp_dir.path()).expect("should discover workspace");

    let changeset_dir =
        changeset_workspace::ensure_changeset_dir(&workspace).expect("should create dir");

    assert!(changeset_dir.exists());
    assert!(changeset_dir.is_dir());
    assert_eq!(changeset_dir, workspace.root.join(".changeset"));
}
