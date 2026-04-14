#![allow(dead_code)]

use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use tempfile::TempDir;

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
version-field-path = "version"
"#
    )
    .expect("failed to append helm chart config to Cargo.toml");
}

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
version-field-path = "version"
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

pub fn append_to_cargo_toml(dir: &TempDir, content: &str) {
    let mut file = OpenOptions::new()
        .append(true)
        .open(dir.path().join("Cargo.toml"))
        .expect("failed to open Cargo.toml for appending");

    write!(file, "{content}").expect("failed to append to Cargo.toml");
}

pub fn create_workspace_with_version_tracking_additional_to_cargo() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    fs::create_dir_all(dir.path().join("crates/my-rust-crate/src"))
        .expect("failed to create my-rust-crate dir");
    fs::create_dir_all(dir.path().join("charts")).expect("failed to create charts dir");
    fs::create_dir_all(dir.path().join(".changeset/changesets"))
        .expect("failed to create .changeset/changesets dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"

[[workspace.metadata.changeset.additional-packages]]
name = "my-helm-chart"
path = "charts"
influence = ["charts/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "charts/Chart.yaml"
format = "yaml"
version-field-path = "version"

[[workspace.metadata.changeset.additional-packages.dependencies]]
dependency-name = "my-rust-crate"

[workspace.metadata.changeset.additional-packages.dependencies.version-tracking-manifest]
file-path = "charts/Chart.yaml"
format = "yaml"
version-field-path = "appVersion"
"#,
    )
    .expect("failed to write workspace Cargo.toml");

    fs::write(
        dir.path().join("crates/my-rust-crate/Cargo.toml"),
        r#"[package]
name = "my-rust-crate"
version = "1.0.0"
edition = "2021"
"#,
    )
    .expect("failed to write my-rust-crate Cargo.toml");

    fs::write(dir.path().join("crates/my-rust-crate/src/lib.rs"), "")
        .expect("failed to write my-rust-crate lib.rs");

    fs::write(
        dir.path().join("charts/Chart.yaml"),
        "apiVersion: v2\nname: my-helm-chart\nversion: \"0.1.0\"\nappVersion: \"1.0.0\"\n",
    )
    .expect("failed to write Chart.yaml");

    dir
}

pub fn create_workspace_with_version_tracking_cargo_to_additional() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    fs::create_dir_all(dir.path().join("crates/my-rust-crate/src"))
        .expect("failed to create my-rust-crate dir");
    fs::create_dir_all(dir.path().join("packages/my-lib"))
        .expect("failed to create packages/my-lib dir");
    fs::create_dir_all(dir.path().join(".changeset/changesets"))
        .expect("failed to create .changeset/changesets dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"

[[workspace.metadata.changeset.additional-packages]]
name = "my-lib"
path = "packages/my-lib"
influence = ["packages/my-lib/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "packages/my-lib/package.json"
format = "json"
version-field-path = "version"
"#,
    )
    .expect("failed to write workspace Cargo.toml");

    fs::write(
        dir.path().join("crates/my-rust-crate/Cargo.toml"),
        r#"[package]
name = "my-rust-crate"
version = "1.0.0"
edition = "2021"

[[package.metadata.changeset.additional-package-dependencies]]
dependency-name = "my-lib"

[package.metadata.changeset.additional-package-dependencies.version-tracking-manifest]
file-path = "crates/my-rust-crate/src/upstream_version.json"
format = "json"
version-field-path = "version"
"#,
    )
    .expect("failed to write my-rust-crate Cargo.toml");

    fs::write(dir.path().join("crates/my-rust-crate/src/lib.rs"), "")
        .expect("failed to write my-rust-crate lib.rs");

    fs::write(
        dir.path().join("packages/my-lib/package.json"),
        r#"{"name": "my-lib", "version": "2.0.0"}"#,
    )
    .expect("failed to write package.json");

    fs::write(
        dir.path()
            .join("crates/my-rust-crate/src/upstream_version.json"),
        r#"{"version": "2.0.0"}"#,
    )
    .expect("failed to write upstream_version.json");

    dir
}

pub fn create_workspace_with_unknown_dependency() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    fs::create_dir_all(dir.path().join("crates/crate-a/src"))
        .expect("failed to create crate-a dir");
    fs::create_dir_all(dir.path().join("charts")).expect("failed to create charts dir");
    fs::create_dir_all(dir.path().join(".changeset/changesets"))
        .expect("failed to create .changeset/changesets dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"

[[workspace.metadata.changeset.additional-packages]]
name = "my-chart"
path = "charts"
influence = ["charts/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "charts/Chart.yaml"
format = "yaml"
version-field-path = "version"

[[workspace.metadata.changeset.additional-packages.dependencies]]
dependency-name = "unknown-pkg"

[workspace.metadata.changeset.additional-packages.dependencies.version-tracking-manifest]
file-path = "charts/upstream.json"
format = "json"
version-field-path = "version"
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
        dir.path().join("charts/Chart.yaml"),
        "apiVersion: v2\nname: my-chart\nversion: \"1.0.0\"\n",
    )
    .expect("failed to write Chart.yaml");

    write_file(
        dir.path(),
        "charts/upstream.json",
        r#"{"version": "1.0.0"}"#,
    );

    dir
}

pub fn create_workspace_with_circular_version_tracking() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    fs::create_dir_all(dir.path().join("crates/crate-a/src"))
        .expect("failed to create crate-a dir");
    fs::create_dir_all(dir.path().join("charts")).expect("failed to create charts dir");
    fs::create_dir_all(dir.path().join(".changeset/changesets"))
        .expect("failed to create .changeset/changesets dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"

[[workspace.metadata.changeset.additional-packages]]
name = "my-helm-chart"
path = "charts"
influence = ["charts/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "charts/Chart.yaml"
format = "yaml"
version-field-path = "version"

[[workspace.metadata.changeset.additional-packages.dependencies]]
dependency-name = "crate-a"

[workspace.metadata.changeset.additional-packages.dependencies.version-tracking-manifest]
file-path = "charts/Chart.yaml"
format = "yaml"
version-field-path = "appVersion"
"#,
    )
    .expect("failed to write workspace Cargo.toml");

    fs::write(
        dir.path().join("crates/crate-a/Cargo.toml"),
        r#"[package]
name = "crate-a"
version = "1.0.0"
edition = "2021"

[[package.metadata.changeset.additional-package-dependencies]]
dependency-name = "my-helm-chart"

[package.metadata.changeset.additional-package-dependencies.version-tracking-manifest]
file-path = "crates/crate-a/src/chart_version.json"
format = "json"
version-field-path = "chartVersion"
"#,
    )
    .expect("failed to write crate-a Cargo.toml");

    fs::write(dir.path().join("crates/crate-a/src/lib.rs"), "")
        .expect("failed to write crate-a lib.rs");

    write_file(
        dir.path(),
        "crates/crate-a/src/chart_version.json",
        r#"{"chartVersion": "1.0.0"}"#,
    );

    fs::write(
        dir.path().join("charts/Chart.yaml"),
        "apiVersion: v2\nname: my-helm-chart\nversion: \"1.0.0\"\nappVersion: \"1.0.0\"\n",
    )
    .expect("failed to write Chart.yaml");

    dir
}

pub fn create_workspace_with_duplicate_dependency() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    fs::create_dir_all(dir.path().join("crates/crate-a/src"))
        .expect("failed to create crate-a dir");
    fs::create_dir_all(dir.path().join("charts")).expect("failed to create charts dir");
    fs::create_dir_all(dir.path().join(".changeset/changesets"))
        .expect("failed to create .changeset/changesets dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"

[[workspace.metadata.changeset.additional-packages]]
name = "my-helm-chart"
path = "charts"
influence = ["charts/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "charts/Chart.yaml"
format = "yaml"
version-field-path = "version"

[[workspace.metadata.changeset.additional-packages.dependencies]]
dependency-name = "crate-a"

[workspace.metadata.changeset.additional-packages.dependencies.version-tracking-manifest]
file-path = "charts/Chart.yaml"
format = "yaml"
version-field-path = "appVersion"

[[workspace.metadata.changeset.additional-packages.dependencies]]
dependency-name = "crate-a"

[workspace.metadata.changeset.additional-packages.dependencies.version-tracking-manifest]
file-path = "charts/upstream.json"
format = "json"
version-field-path = "version"
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
        dir.path().join("charts/Chart.yaml"),
        "apiVersion: v2\nname: my-helm-chart\nversion: \"1.0.0\"\nappVersion: \"1.0.0\"\n",
    )
    .expect("failed to write Chart.yaml");

    write_file(
        dir.path(),
        "charts/upstream.json",
        r#"{"version": "1.0.0"}"#,
    );

    dir
}

pub fn create_workspace_with_cascade_chain() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    fs::create_dir_all(dir.path().join("crates/crate-a/src"))
        .expect("failed to create crate-a dir");
    fs::create_dir_all(dir.path().join("packages/pkg-b"))
        .expect("failed to create packages/pkg-b dir");
    fs::create_dir_all(dir.path().join("packages/pkg-c"))
        .expect("failed to create packages/pkg-c dir");
    fs::create_dir_all(dir.path().join(".changeset/changesets"))
        .expect("failed to create .changeset/changesets dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"

[[workspace.metadata.changeset.additional-packages]]
name = "pkg-b"
path = "packages/pkg-b"
influence = ["packages/pkg-b/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "packages/pkg-b/manifest.json"
format = "json"
version-field-path = "version"

[[workspace.metadata.changeset.additional-packages.dependencies]]
dependency-name = "crate-a"

[workspace.metadata.changeset.additional-packages.dependencies.version-tracking-manifest]
file-path = "packages/pkg-b/manifest.json"
format = "json"
version-field-path = "upstreamVersion"

[[workspace.metadata.changeset.additional-packages]]
name = "pkg-c"
path = "packages/pkg-c"
influence = ["packages/pkg-c/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "packages/pkg-c/manifest.json"
format = "json"
version-field-path = "version"

[[workspace.metadata.changeset.additional-packages.dependencies]]
dependency-name = "pkg-b"

[workspace.metadata.changeset.additional-packages.dependencies.version-tracking-manifest]
file-path = "packages/pkg-c/manifest.json"
format = "json"
version-field-path = "upstreamVersion"
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
        dir.path().join("packages/pkg-b/manifest.json"),
        r#"{"name": "pkg-b", "version": "2.0.0", "upstreamVersion": "1.0.0"}"#,
    )
    .expect("failed to write pkg-b manifest.json");

    fs::write(
        dir.path().join("packages/pkg-c/manifest.json"),
        r#"{"name": "pkg-c", "version": "3.0.0", "upstreamVersion": "2.0.0"}"#,
    )
    .expect("failed to write pkg-c manifest.json");

    dir
}

pub fn create_workspace_with_json_extra_fields() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    fs::create_dir_all(dir.path().join("crates/crate-a/src"))
        .expect("failed to create crate-a dir");
    fs::create_dir_all(dir.path().join("packages/my-app"))
        .expect("failed to create packages/my-app dir");
    fs::create_dir_all(dir.path().join(".changeset/changesets"))
        .expect("failed to create .changeset/changesets dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"

[[workspace.metadata.changeset.additional-packages]]
name = "my-app"
path = "packages/my-app"
influence = ["packages/my-app/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "packages/my-app/app.json"
format = "json"
version-field-path = "version"

[[workspace.metadata.changeset.additional-packages.dependencies]]
dependency-name = "crate-a"

[workspace.metadata.changeset.additional-packages.dependencies.version-tracking-manifest]
file-path = "packages/my-app/app.json"
format = "json"
version-field-path = "appVersion"
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
        dir.path().join("packages/my-app/app.json"),
        r#"{
  "version": "0.1.0",
  "appVersion": "1.0.0",
  "description": "my app",
  "extraField": 42,
  "nested": {
    "key": "value"
  }
}"#,
    )
    .expect("failed to write app.json");

    dir
}

pub fn create_workspace_with_multiple_deps_on_same_crate() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    fs::create_dir_all(dir.path().join("crates/crate-a/src"))
        .expect("failed to create crate-a dir");
    fs::create_dir_all(dir.path().join("charts")).expect("failed to create charts dir");
    fs::create_dir_all(dir.path().join("config")).expect("failed to create config dir");
    fs::create_dir_all(dir.path().join(".changeset/changesets"))
        .expect("failed to create .changeset/changesets dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"

[[workspace.metadata.changeset.additional-packages]]
name = "pkg-b"
path = "charts"
influence = ["charts/**", "config/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "charts/Chart.yaml"
format = "yaml"
version-field-path = "version"

[[workspace.metadata.changeset.additional-packages.dependencies]]
dependency-name = "crate-a"

[workspace.metadata.changeset.additional-packages.dependencies.version-tracking-manifest]
file-path = "charts/Chart.yaml"
format = "yaml"
version-field-path = "appVersion"

[[workspace.metadata.changeset.additional-packages.dependencies]]
dependency-name = "crate-a"

[workspace.metadata.changeset.additional-packages.dependencies.version-tracking-manifest]
file-path = "config/deps.json"
format = "json"
version-field-path = "crateA.version"
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
        dir.path().join("charts/Chart.yaml"),
        "apiVersion: v2\nname: pkg-b\nversion: \"0.1.0\"\nappVersion: \"1.0.0\"\n",
    )
    .expect("failed to write Chart.yaml");

    fs::write(
        dir.path().join("config/deps.json"),
        r#"{"crateA": {"version": "1.0.0"}}"#,
    )
    .expect("failed to write deps.json");

    dir
}

pub fn create_workspace_with_invalid_version_field_path() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    fs::create_dir_all(dir.path().join("crates/crate-a/src"))
        .expect("failed to create crate-a dir");
    fs::create_dir_all(dir.path().join("packages/pkg-b"))
        .expect("failed to create packages/pkg-b dir");
    fs::create_dir_all(dir.path().join(".changeset/changesets"))
        .expect("failed to create .changeset/changesets dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"

[[workspace.metadata.changeset.additional-packages]]
name = "pkg-b"
path = "packages/pkg-b"
influence = ["packages/pkg-b/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "packages/pkg-b/manifest.json"
format = "json"
version-field-path = "version"

[[workspace.metadata.changeset.additional-packages.dependencies]]
dependency-name = "crate-a"

[workspace.metadata.changeset.additional-packages.dependencies.version-tracking-manifest]
file-path = "packages/pkg-b/manifest.json"
format = "json"
version-field-path = "nonexistent.deeply.nested.path"
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
        dir.path().join("packages/pkg-b/manifest.json"),
        r#"{"name": "pkg-b", "version": "1.0.0"}"#,
    )
    .expect("failed to write manifest.json");

    dir
}

pub fn create_workspace_with_prerelease_dependency() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    fs::create_dir_all(dir.path().join("crates/crate-a/src"))
        .expect("failed to create crate-a dir");
    fs::create_dir_all(dir.path().join("packages/pkg-b"))
        .expect("failed to create packages/pkg-b dir");
    fs::create_dir_all(dir.path().join(".changeset/changesets"))
        .expect("failed to create .changeset/changesets dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"

[[workspace.metadata.changeset.additional-packages]]
name = "pkg-b"
path = "packages/pkg-b"
influence = ["packages/pkg-b/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "packages/pkg-b/manifest.json"
format = "json"
version-field-path = "version"

[[workspace.metadata.changeset.additional-packages.dependencies]]
dependency-name = "crate-a"

[workspace.metadata.changeset.additional-packages.dependencies.version-tracking-manifest]
file-path = "packages/pkg-b/manifest.json"
format = "json"
version-field-path = "upstreamVersion"
"#,
    )
    .expect("failed to write workspace Cargo.toml");

    fs::write(
        dir.path().join("crates/crate-a/Cargo.toml"),
        r#"[package]
name = "crate-a"
version = "1.0.0-alpha.1"
edition = "2021"
"#,
    )
    .expect("failed to write crate-a Cargo.toml");

    fs::write(dir.path().join("crates/crate-a/src/lib.rs"), "")
        .expect("failed to write crate-a lib.rs");

    fs::write(
        dir.path().join("packages/pkg-b/manifest.json"),
        r#"{"name": "pkg-b", "version": "1.0.0", "upstreamVersion": "1.0.0-alpha.1"}"#,
    )
    .expect("failed to write manifest.json");

    dir
}

pub fn create_workspace_with_deeply_nested_json_field() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");

    fs::create_dir_all(dir.path().join("crates/crate-a/src"))
        .expect("failed to create crate-a dir");
    fs::create_dir_all(dir.path().join("packages/pkg-b"))
        .expect("failed to create packages/pkg-b dir");
    fs::create_dir_all(dir.path().join(".changeset/changesets"))
        .expect("failed to create .changeset/changesets dir");

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[workspace]
members = ["crates/*"]
resolver = "2"

[[workspace.metadata.changeset.additional-packages]]
name = "pkg-b"
path = "packages/pkg-b"
influence = ["packages/pkg-b/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "packages/pkg-b/manifest.json"
format = "json"
version-field-path = "version"

[[workspace.metadata.changeset.additional-packages.dependencies]]
dependency-name = "crate-a"

[workspace.metadata.changeset.additional-packages.dependencies.version-tracking-manifest]
file-path = "packages/pkg-b/manifest.json"
format = "json"
version-field-path = "metadata.versions.upstream_crate"
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
        dir.path().join("packages/pkg-b/manifest.json"),
        r#"{"name": "pkg-b", "version": "1.0.0", "metadata": {"versions": {"upstream_crate": "1.0.0"}}}"#,
    )
    .expect("failed to write manifest.json");

    dir
}

pub fn add_helm_chart_config_with_three_deps(dir: &TempDir) {
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
version-field-path = "version"

[[workspace.metadata.changeset.additional-packages.dependencies]]
dependency-name = "dep-alpha"

[workspace.metadata.changeset.additional-packages.dependencies.version-tracking-manifest]
file-path = "charts/my-chart/Chart.yaml"
format = "yaml"
version-field-path = "alphaVersion"

[[workspace.metadata.changeset.additional-packages.dependencies]]
dependency-name = "dep-beta"

[workspace.metadata.changeset.additional-packages.dependencies.version-tracking-manifest]
file-path = "charts/my-chart/Chart.yaml"
format = "yaml"
version-field-path = "betaVersion"

[[workspace.metadata.changeset.additional-packages.dependencies]]
dependency-name = "dep-gamma"

[workspace.metadata.changeset.additional-packages.dependencies.version-tracking-manifest]
file-path = "charts/my-chart/Chart.yaml"
format = "yaml"
version-field-path = "gammaVersion"
"#
    )
    .expect("failed to append helm chart config with three deps to Cargo.toml");
}

fn write_file(base: &Path, relative: &str, content: &str) {
    let path = base.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create parent dirs");
    }
    fs::write(&path, content).expect("failed to write file");
}
