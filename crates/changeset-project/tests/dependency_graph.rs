use changeset_project::{WorkspaceDependencyGraph, discover_project};

fn create_workspace_with_target_dep(
    target_section: &str,
) -> (tempfile::TempDir, changeset_project::CargoProject) {
    let dir = tempfile::tempdir().expect("create temp dir");

    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"2\"\n",
    )
    .expect("write root Cargo.toml");

    let pkg_a = dir.path().join("crates/crate-a");
    std::fs::create_dir_all(pkg_a.join("src")).expect("create crate-a dirs");
    std::fs::write(
        pkg_a.join("Cargo.toml"),
        "[package]\nname = \"crate-a\"\nversion = \"1.0.0\"\nedition = \"2021\"\n",
    )
    .expect("write crate-a Cargo.toml");
    std::fs::write(pkg_a.join("src/lib.rs"), "").expect("write lib.rs");

    let pkg_b = dir.path().join("crates/crate-b");
    std::fs::create_dir_all(pkg_b.join("src")).expect("create crate-b dirs");
    std::fs::write(
        pkg_b.join("Cargo.toml"),
        format!(
            "[package]\nname = \"crate-b\"\nversion = \"1.0.0\"\nedition = \"2021\"\n\n{target_section}\n"
        ),
    )
    .expect("write crate-b Cargo.toml");
    std::fs::write(pkg_b.join("src/lib.rs"), "").expect("write lib.rs");

    let project = discover_project(dir.path()).expect("discover project");
    (dir, project)
}

#[test]
fn target_specific_dependency_detected() {
    let (_dir, project) = create_workspace_with_target_dep(
        r#"[target.'cfg(unix)'.dependencies]
crate-a = { path = "../crate-a" }"#,
    );

    let graph = WorkspaceDependencyGraph::build(&project).expect("build graph");
    let deps = graph.direct_dependencies("crate-b");
    assert!(deps.contains("crate-a"));
}

#[test]
fn target_specific_build_dependency_detected() {
    let (_dir, project) = create_workspace_with_target_dep(
        r#"[target.'cfg(unix)'.build-dependencies]
crate-a = { path = "../crate-a" }"#,
    );

    let graph = WorkspaceDependencyGraph::build(&project).expect("build graph");
    let deps = graph.direct_dependencies("crate-b");
    assert!(deps.contains("crate-a"));
}

#[test]
fn target_specific_dependency_with_rename() {
    let (_dir, project) = create_workspace_with_target_dep(
        r#"[target.'cfg(unix)'.dependencies]
my-alias = { path = "../crate-a", package = "crate-a" }"#,
    );

    let graph = WorkspaceDependencyGraph::build(&project).expect("build graph");
    let deps = graph.direct_dependencies("crate-b");
    assert!(deps.contains("crate-a"));
}

#[test]
fn target_specific_deps_included_in_transitive_dependents() {
    let dir = tempfile::tempdir().expect("create temp dir");

    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"2\"\n",
    )
    .expect("write root Cargo.toml");

    let pkg_a = dir.path().join("crates/crate-a");
    std::fs::create_dir_all(pkg_a.join("src")).expect("create dirs");
    std::fs::write(
        pkg_a.join("Cargo.toml"),
        "[package]\nname = \"crate-a\"\nversion = \"1.0.0\"\nedition = \"2021\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(pkg_a.join("src/lib.rs"), "").expect("write lib.rs");

    let pkg_b = dir.path().join("crates/crate-b");
    std::fs::create_dir_all(pkg_b.join("src")).expect("create dirs");
    std::fs::write(
        pkg_b.join("Cargo.toml"),
        "[package]\nname = \"crate-b\"\nversion = \"1.0.0\"\nedition = \"2021\"\n\n\
         [target.'cfg(unix)'.dependencies]\n\
         crate-a = { path = \"../crate-a\" }\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(pkg_b.join("src/lib.rs"), "").expect("write lib.rs");

    let pkg_c = dir.path().join("crates/crate-c");
    std::fs::create_dir_all(pkg_c.join("src")).expect("create dirs");
    std::fs::write(
        pkg_c.join("Cargo.toml"),
        "[package]\nname = \"crate-c\"\nversion = \"1.0.0\"\nedition = \"2021\"\n\n\
         [dependencies]\n\
         crate-b = { path = \"../crate-b\" }\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(pkg_c.join("src/lib.rs"), "").expect("write lib.rs");

    let project = discover_project(dir.path()).expect("discover project");
    let graph = WorkspaceDependencyGraph::build(&project).expect("build graph");

    let dependents = graph.transitive_dependents("crate-a");
    assert!(dependents.contains("crate-b"));
    assert!(dependents.contains("crate-c"));
}

#[test]
fn target_specific_dev_dependency_not_in_graph() {
    let (_dir, project) = create_workspace_with_target_dep(
        r#"[target.'cfg(unix)'.dev-dependencies]
crate-a = { path = "../crate-a" }"#,
    );

    let graph = WorkspaceDependencyGraph::build(&project).expect("build graph");
    let deps = graph.direct_dependencies("crate-b");
    assert!(!deps.contains("crate-a"));
}
