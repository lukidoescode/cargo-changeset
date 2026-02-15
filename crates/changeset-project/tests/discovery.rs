use std::path::PathBuf;

use changeset_project::{ProjectError, ProjectKind, discover_project, parse_root_config};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn create_temp_single_package() -> tempfile::TempDir {
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
    let project = discover_project(&fixture).expect("should discover project");

    assert_eq!(project.kind, ProjectKind::VirtualWorkspace);
    assert_eq!(project.root, fixture.canonicalize().expect("path exists"));
    assert_eq!(project.packages.len(), 2);

    let names: Vec<_> = project.packages.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"crate-a"));
    assert!(names.contains(&"crate-b"));
}

#[test]
fn discovers_project_from_nested_member() {
    let fixture = fixtures_dir().join("virtual_workspace/crates/crate_a");
    let project = discover_project(&fixture).expect("should discover project");

    assert_eq!(project.kind, ProjectKind::VirtualWorkspace);
    assert_eq!(
        project.root,
        fixtures_dir()
            .join("virtual_workspace")
            .canonicalize()
            .expect("path exists")
    );
    assert_eq!(project.packages.len(), 2);
}

#[test]
fn discovers_workspace_with_root_package() {
    let fixture = fixtures_dir().join("root_package_workspace");
    let project = discover_project(&fixture).expect("should discover project");

    assert_eq!(project.kind, ProjectKind::WorkspaceWithRoot);
    assert_eq!(project.packages.len(), 2);

    let root_pkg = project
        .packages
        .iter()
        .find(|p| p.name == "root-pkg")
        .expect("should have root package");
    assert_eq!(root_pkg.version.to_string(), "0.1.0");

    let member = project
        .packages
        .iter()
        .find(|p| p.name == "member")
        .expect("should have member package");
    assert_eq!(member.version.to_string(), "0.2.0");
}

#[test]
fn discovers_single_package() {
    let temp_dir = create_temp_single_package();
    let project = discover_project(temp_dir.path()).expect("should discover project");

    assert_eq!(project.kind, ProjectKind::SinglePackage);
    assert_eq!(project.packages.len(), 1);
    assert_eq!(project.packages[0].name, "single");
    assert_eq!(project.packages[0].version.to_string(), "3.0.0");
}

#[test]
fn discovers_from_deeply_nested_path() {
    let fixture = fixtures_dir().join("nested/packages/inner");
    let project = discover_project(&fixture).expect("should discover project");

    assert_eq!(project.kind, ProjectKind::VirtualWorkspace);
    assert_eq!(
        project.root,
        fixtures_dir()
            .join("nested")
            .canonicalize()
            .expect("path exists")
    );
}

#[test]
fn version_inheritance_works() {
    let fixture = fixtures_dir().join("virtual_workspace");
    let project = discover_project(&fixture).expect("should discover project");

    let crate_a = project
        .packages
        .iter()
        .find(|p| p.name == "crate-a")
        .expect("should have crate-a");
    assert_eq!(crate_a.version.to_string(), "1.0.0");

    let crate_b = project
        .packages
        .iter()
        .find(|p| p.name == "crate-b")
        .expect("should have crate-b");
    assert_eq!(crate_b.version.to_string(), "2.0.0");
}

#[test]
fn not_found_error_for_nonexistent_path() {
    let result = discover_project(&PathBuf::from("/nonexistent/path"));
    assert!(result.is_err());
}

#[test]
fn ensure_changeset_dir_creates_directory() {
    let temp_dir = create_temp_single_package();
    let project = discover_project(temp_dir.path()).expect("should discover project");
    let config = parse_root_config(&project).expect("should parse config");

    let changeset_dir =
        changeset_project::ensure_changeset_dir(&project, &config).expect("should create dir");

    assert!(changeset_dir.exists());
    assert!(changeset_dir.is_dir());
    assert_eq!(changeset_dir, project.root.join(".changeset"));

    let changesets_subdir = changeset_dir.join("changesets");
    assert!(
        changesets_subdir.exists(),
        "changesets subdirectory should be created"
    );
    assert!(changesets_subdir.is_dir());
}

#[test]
fn malformed_toml_returns_manifest_parse_error() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    std::fs::write(
        temp_dir.path().join("Cargo.toml"),
        "this is not valid toml {{{",
    )
    .expect("write cargo toml");

    let result = discover_project(temp_dir.path());
    assert!(matches!(result, Err(ProjectError::ManifestParse { .. })));
}

#[test]
fn missing_package_version_returns_missing_field_error() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    std::fs::write(
        temp_dir.path().join("Cargo.toml"),
        r#"[package]
name = "no-version"
"#,
    )
    .expect("write cargo toml");

    let result = discover_project(temp_dir.path());
    assert!(matches!(
        result,
        Err(ProjectError::MissingField {
            field: "package.version",
            ..
        })
    ));
}

#[test]
fn invalid_semver_returns_invalid_version_error() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    std::fs::write(
        temp_dir.path().join("Cargo.toml"),
        r#"[package]
name = "bad-version"
version = "not.a.version"
"#,
    )
    .expect("write cargo toml");

    let result = discover_project(temp_dir.path());
    assert!(
        matches!(result, Err(ProjectError::InvalidVersion { version, .. }) if version == "not.a.version")
    );
}

#[test]
fn invalid_glob_pattern_returns_glob_pattern_error() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    std::fs::write(
        temp_dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["[invalid"]
"#,
    )
    .expect("write cargo toml");

    let result = discover_project(temp_dir.path());
    assert!(
        matches!(result, Err(ProjectError::GlobPattern { pattern, .. }) if pattern == "[invalid")
    );
}

#[test]
fn workspace_exclude_patterns_work() {
    let fixture = fixtures_dir().join("workspace_with_exclude");
    let project = discover_project(&fixture).expect("should discover project");

    assert_eq!(project.kind, ProjectKind::VirtualWorkspace);
    assert_eq!(project.packages.len(), 1);

    let names: Vec<_> = project.packages.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"included"));
    assert!(!names.contains(&"excluded"));
}

#[test]
fn ensure_changeset_dir_is_idempotent() {
    let temp_dir = create_temp_single_package();
    let project = discover_project(temp_dir.path()).expect("should discover project");
    let config = parse_root_config(&project).expect("should parse config");

    let first = changeset_project::ensure_changeset_dir(&project, &config).expect("first call");
    let second = changeset_project::ensure_changeset_dir(&project, &config).expect("second call");

    assert_eq!(first, second);
    assert!(first.exists());
}

#[test]
fn discover_project_from_current_directory_works() {
    let cwd = std::env::current_dir().expect("current_dir should succeed");
    let result = discover_project(&cwd);
    assert!(result.is_ok());
}
