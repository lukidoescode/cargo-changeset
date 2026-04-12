use std::fs;
use std::fs::OpenOptions;
use std::io::Write;

use tempfile::TempDir;

#[allow(dead_code)]
pub fn create_single_crate_workspace() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");
    fs::create_dir_all(dir.path().join("src")).expect("failed to create src dir");
    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[package]
name = "test-crate"
version = "1.0.0"
edition = "2021"
"#,
    )
    .expect("failed to write Cargo.toml");
    fs::write(dir.path().join("src/lib.rs"), "").expect("failed to write lib.rs");
    dir
}

#[allow(dead_code)]
pub fn create_virtual_workspace() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    fs::create_dir_all(dir.path().join("crates/a/src")).expect("failed to create crate a dir");
    fs::create_dir_all(dir.path().join("crates/b/src")).expect("failed to create crate b dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"
"#,
    )
    .expect("failed to write workspace Cargo.toml");

    fs::write(
        dir.path().join("crates/a/Cargo.toml"),
        r#"[package]
name = "crate-a"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("failed to write crate-a Cargo.toml");

    fs::write(dir.path().join("crates/a/src/lib.rs"), "").expect("failed to write crate-a lib.rs");

    fs::write(
        dir.path().join("crates/b/Cargo.toml"),
        r#"[package]
name = "crate-b"
version = "0.2.0"
edition = "2021"
"#,
    )
    .expect("failed to write crate-b Cargo.toml");

    fs::write(dir.path().join("crates/b/src/lib.rs"), "").expect("failed to write crate-b lib.rs");

    dir
}

#[allow(dead_code)]
pub fn create_workspace_with_helm_chart() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    fs::create_dir_all(dir.path().join("crates/crate-a/src"))
        .expect("failed to create crate-a dir");
    fs::create_dir_all(dir.path().join("charts/my-chart"))
        .expect("failed to create charts/my-chart dir");
    fs::create_dir_all(dir.path().join(".changeset/changesets"))
        .expect("failed to create .changeset/changesets dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"
"#,
    )
    .expect("failed to write workspace Cargo.toml");

    fs::write(
        dir.path().join("crates/crate-a/Cargo.toml"),
        r#"[package]
name = "crate-a"
version = "1.0.0"
edition = "2021"
"#,
    )
    .expect("failed to write crate-a Cargo.toml");

    fs::write(dir.path().join("crates/crate-a/src/lib.rs"), "")
        .expect("failed to write crate-a lib.rs");

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
        "replicaCount: 1\n",
    )
    .expect("failed to write values.yaml");

    dir
}

#[allow(dead_code)]
pub fn add_helm_chart_config(dir: &TempDir) {
    let mut file = OpenOptions::new()
        .append(true)
        .open(dir.path().join("Cargo.toml"))
        .expect("failed to open Cargo.toml for appending");

    write!(
        file,
        r#"
[[workspace.metadata.changeset.additional-packages]]
name = "my-helm-chart"
path = "charts/my-chart"
influence = ["charts/my-chart/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "charts/my-chart/Chart.yaml"
format = "yaml"
version-path = "version"
"#
    )
    .expect("failed to append helm chart config to Cargo.toml");
}

#[allow(dead_code)]
pub fn create_workspace_with_additional_package() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    fs::create_dir_all(dir.path().join("crates/crate-a/src"))
        .expect("failed to create crate-a dir");
    fs::create_dir_all(dir.path().join("charts/my-chart"))
        .expect("failed to create charts/my-chart dir");
    fs::create_dir_all(dir.path().join(".changeset/changesets"))
        .expect("failed to create .changeset/changesets dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"

[[workspace.metadata.changeset.additional-packages]]
name = "my-helm-chart"
path = "charts/my-chart"
influence = ["charts/my-chart/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "charts/my-chart/Chart.yaml"
format = "yaml"
version-path = "version"
"#,
    )
    .expect("failed to write workspace Cargo.toml");

    fs::write(
        dir.path().join("crates/crate-a/Cargo.toml"),
        r#"[package]
name = "crate-a"
version = "1.0.0"
edition = "2021"
"#,
    )
    .expect("failed to write crate-a Cargo.toml");

    fs::write(dir.path().join("crates/crate-a/src/lib.rs"), "")
        .expect("failed to write crate-a lib.rs");

    fs::write(
        dir.path().join("charts/my-chart/Chart.yaml"),
        r#"apiVersion: v2
name: my-chart
version: "2.0.0"
"#,
    )
    .expect("failed to write Chart.yaml");

    dir
}
