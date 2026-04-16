use std::fmt::Write as FmtWrite;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use tempfile::TempDir;

use crate::git::{git_add_and_commit, init_git_repo};

struct CrateSpec {
    name: String,
    version: String,
    dir_name: String,
    inherited_version: bool,
    cargo_toml_extra: Option<String>,
}

struct ExtraFile {
    relative_path: String,
    content: String,
}

#[must_use]
pub struct WorkspaceBuilder {
    crates: Vec<CrateSpec>,
    with_git: bool,
    with_changeset_dir: bool,
    root_package: Option<(String, String)>,
    workspace_toml_extra: Option<String>,
    workspace_package: Option<String>,
    single_package: bool,
    extra_files: Vec<ExtraFile>,
}

impl WorkspaceBuilder {
    pub fn single_package(name: &str, version: &str) -> Self {
        Self {
            crates: Vec::new(),
            with_git: false,
            with_changeset_dir: false,
            root_package: Some((name.to_owned(), version.to_owned())),
            workspace_toml_extra: None,
            workspace_package: None,
            single_package: true,
            extra_files: Vec::new(),
        }
    }

    pub fn virtual_workspace() -> Self {
        Self {
            crates: Vec::new(),
            with_git: false,
            with_changeset_dir: false,
            root_package: None,
            workspace_toml_extra: None,
            workspace_package: None,
            single_package: false,
            extra_files: Vec::new(),
        }
    }

    pub fn crate_member(mut self, name: &str, version: &str) -> Self {
        self.crates.push(CrateSpec {
            name: name.to_owned(),
            version: version.to_owned(),
            dir_name: format!("crates/{name}"),
            inherited_version: false,
            cargo_toml_extra: None,
        });
        self
    }

    pub fn crate_member_at(mut self, name: &str, version: &str, dir: &str) -> Self {
        self.crates.push(CrateSpec {
            name: name.to_owned(),
            version: version.to_owned(),
            dir_name: dir.to_owned(),
            inherited_version: false,
            cargo_toml_extra: None,
        });
        self
    }

    pub fn crate_member_with_inherited_version(mut self, name: &str, dir: &str) -> Self {
        self.crates.push(CrateSpec {
            name: name.to_owned(),
            version: String::new(),
            dir_name: dir.to_owned(),
            inherited_version: true,
            cargo_toml_extra: None,
        });
        self
    }

    pub fn root_package(mut self, name: &str, version: &str) -> Self {
        self.root_package = Some((name.to_owned(), version.to_owned()));
        self
    }

    pub fn with_git(mut self) -> Self {
        self.with_git = true;
        self
    }

    pub fn with_changeset_dir(mut self) -> Self {
        self.with_changeset_dir = true;
        self
    }

    pub fn workspace_toml_extra(mut self, content: &str) -> Self {
        self.workspace_toml_extra = Some(content.to_owned());
        self
    }

    pub fn workspace_package(mut self, content: &str) -> Self {
        self.workspace_package = Some(content.to_owned());
        self
    }

    pub fn crate_toml_extra(mut self, name: &str, content: &str) -> Self {
        for spec in &mut self.crates {
            if spec.name == name {
                spec.cargo_toml_extra = Some(content.to_owned());
                return self;
            }
        }
        panic!("crate_toml_extra: no crate named '{name}' found in builder");
    }

    pub fn extra_file(mut self, relative_path: &str, content: &str) -> Self {
        self.extra_files.push(ExtraFile {
            relative_path: relative_path.to_owned(),
            content: content.to_owned(),
        });
        self
    }

    pub fn build(self) -> TempDir {
        let dir = TempDir::new().expect("failed to create temp dir");

        if self.with_git {
            init_git_repo(&dir);
        }

        let mut cargo_toml = String::new();

        if self.single_package {
            if let Some((ref name, ref version)) = self.root_package {
                let _ = write!(
                    cargo_toml,
                    "[package]\nname = \"{name}\"\nversion = \"{version}\"\nedition = \"2021\"\n"
                );
            }
        } else {
            if let Some((ref name, ref version)) = self.root_package {
                let _ = write!(
                    cargo_toml,
                    "[package]\nname = \"{name}\"\nversion = \"{version}\"\nedition = \"2021\"\n\n"
                );
            }

            cargo_toml.push_str("[workspace]\nmembers = [\"crates/*\"]\n");

            if self.root_package.is_none() {
                cargo_toml.push_str("resolver = \"2\"\n");
            }
        }

        if let Some(ref wp) = self.workspace_package {
            cargo_toml.push('\n');
            cargo_toml.push_str(wp);
            if !wp.ends_with('\n') {
                cargo_toml.push('\n');
            }
        }

        if let Some(ref extra) = self.workspace_toml_extra {
            cargo_toml.push('\n');
            cargo_toml.push_str(extra);
            if !extra.ends_with('\n') {
                cargo_toml.push('\n');
            }
        }

        fs::write(dir.path().join("Cargo.toml"), &cargo_toml).expect("failed to write Cargo.toml");

        if self.single_package || self.root_package.is_some() {
            fs::create_dir_all(dir.path().join("src")).expect("failed to create src dir");
            fs::write(dir.path().join("src/lib.rs"), "").expect("failed to write lib.rs");
        }

        for spec in &self.crates {
            let crate_dir = dir.path().join(&spec.dir_name);
            fs::create_dir_all(crate_dir.join("src")).expect("failed to create crate src dir");

            let version_line = if spec.inherited_version {
                "version.workspace = true".to_owned()
            } else {
                format!("version = \"{}\"", spec.version)
            };

            let edition_line = if spec.inherited_version {
                "edition.workspace = true"
            } else {
                "edition = \"2021\""
            };

            let mut crate_toml = format!(
                "[package]\nname = \"{}\"\n{version_line}\n{edition_line}\n",
                spec.name
            );

            if let Some(ref extra) = spec.cargo_toml_extra {
                crate_toml.push('\n');
                crate_toml.push_str(extra);
                if !extra.ends_with('\n') {
                    crate_toml.push('\n');
                }
            }

            fs::write(crate_dir.join("Cargo.toml"), &crate_toml)
                .expect("failed to write crate Cargo.toml");

            fs::write(crate_dir.join("src/lib.rs"), "").expect("failed to write crate lib.rs");
        }

        for file in &self.extra_files {
            write_file(dir.path(), &file.relative_path, &file.content);
        }

        if self.with_changeset_dir {
            fs::create_dir_all(dir.path().join(".changeset/changesets"))
                .expect("failed to create .changeset/changesets dir");
        }

        if self.with_git {
            git_add_and_commit(&dir, "Initial commit");
        }

        dir
    }
}

pub fn create_single_crate_workspace() -> TempDir {
    WorkspaceBuilder::single_package("test-crate", "1.0.0").build()
}

pub fn create_virtual_workspace() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member("crate-a", "0.1.0")
        .crate_member("crate-b", "0.2.0")
        .build()
}

pub fn create_workspace_with_helm_chart() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member("crate-a", "1.0.0")
        .with_changeset_dir()
        .extra_file(
            "charts/my-chart/Chart.yaml",
            "# Helm chart for my-chart\napiVersion: v2\nname: my-chart\ndescription: A test Helm chart\n# This comment should survive release\nversion: \"2.0.0\"\nappVersion: \"1.0.0\"\n",
        )
        .extra_file("charts/my-chart/values.yaml", "replicaCount: 1\n")
        .build()
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
    WorkspaceBuilder::virtual_workspace()
        .crate_member("crate-a", "1.0.0")
        .with_changeset_dir()
        .workspace_toml_extra(
            r#"[[workspace.metadata.changeset.additional-packages]]
name = "my-helm-chart"
path = "charts/my-chart"
influence = ["charts/my-chart/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "charts/my-chart/Chart.yaml"
format = "yaml"
version-field-path = "version"
"#,
        )
        .extra_file(
            "charts/my-chart/Chart.yaml",
            "apiVersion: v2\nname: my-chart\nversion: \"2.0.0\"\n",
        )
        .build()
}

pub fn append_to_cargo_toml(dir: &TempDir, content: &str) {
    let mut file = OpenOptions::new()
        .append(true)
        .open(dir.path().join("Cargo.toml"))
        .expect("failed to open Cargo.toml for appending");

    write!(file, "{content}").expect("failed to append to Cargo.toml");
}

pub fn create_workspace_with_version_tracking_additional_to_cargo() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member_at("my-rust-crate", "1.0.0", "crates/my-rust-crate")
        .with_changeset_dir()
        .workspace_toml_extra(
            r#"[[workspace.metadata.changeset.additional-packages]]
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
        .extra_file(
            "charts/Chart.yaml",
            "apiVersion: v2\nname: my-helm-chart\nversion: \"0.1.0\"\nappVersion: \"1.0.0\"\n",
        )
        .build()
}

pub fn create_workspace_with_version_tracking_cargo_to_additional() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member_at("my-rust-crate", "1.0.0", "crates/my-rust-crate")
        .crate_toml_extra(
            "my-rust-crate",
            r#"[[package.metadata.changeset.additional-package-dependencies]]
dependency-name = "my-lib"

[package.metadata.changeset.additional-package-dependencies.version-tracking-manifest]
file-path = "crates/my-rust-crate/src/upstream_version.json"
format = "json"
version-field-path = "version"
"#,
        )
        .with_changeset_dir()
        .workspace_toml_extra(
            r#"[[workspace.metadata.changeset.additional-packages]]
name = "my-lib"
path = "packages/my-lib"
influence = ["packages/my-lib/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "packages/my-lib/package.json"
format = "json"
version-field-path = "version"
"#,
        )
        .extra_file(
            "packages/my-lib/package.json",
            r#"{"name": "my-lib", "version": "2.0.0"}"#,
        )
        .extra_file(
            "crates/my-rust-crate/src/upstream_version.json",
            r#"{"version": "2.0.0"}"#,
        )
        .build()
}

pub fn create_workspace_with_unknown_dependency() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member("crate-a", "1.0.0")
        .with_changeset_dir()
        .workspace_toml_extra(
            r#"[[workspace.metadata.changeset.additional-packages]]
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
        .extra_file(
            "charts/Chart.yaml",
            "apiVersion: v2\nname: my-chart\nversion: \"1.0.0\"\n",
        )
        .extra_file("charts/upstream.json", r#"{"version": "1.0.0"}"#)
        .build()
}

pub fn create_workspace_with_circular_version_tracking() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member("crate-a", "1.0.0")
        .crate_toml_extra(
            "crate-a",
            r#"[[package.metadata.changeset.additional-package-dependencies]]
dependency-name = "my-helm-chart"

[package.metadata.changeset.additional-package-dependencies.version-tracking-manifest]
file-path = "crates/crate-a/src/chart_version.json"
format = "json"
version-field-path = "chartVersion"
"#,
        )
        .with_changeset_dir()
        .workspace_toml_extra(
            r#"[[workspace.metadata.changeset.additional-packages]]
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
        .extra_file(
            "crates/crate-a/src/chart_version.json",
            r#"{"chartVersion": "1.0.0"}"#,
        )
        .extra_file(
            "charts/Chart.yaml",
            "apiVersion: v2\nname: my-helm-chart\nversion: \"1.0.0\"\nappVersion: \"1.0.0\"\n",
        )
        .build()
}

pub fn create_workspace_with_duplicate_dependency() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member("crate-a", "1.0.0")
        .with_changeset_dir()
        .workspace_toml_extra(
            r#"[[workspace.metadata.changeset.additional-packages]]
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
        .extra_file(
            "charts/Chart.yaml",
            "apiVersion: v2\nname: my-helm-chart\nversion: \"1.0.0\"\nappVersion: \"1.0.0\"\n",
        )
        .extra_file("charts/upstream.json", r#"{"version": "1.0.0"}"#)
        .build()
}

pub fn create_workspace_with_cascade_chain() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member("crate-a", "1.0.0")
        .with_changeset_dir()
        .workspace_toml_extra(
            r#"[[workspace.metadata.changeset.additional-packages]]
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
        .extra_file(
            "packages/pkg-b/manifest.json",
            r#"{"name": "pkg-b", "version": "2.0.0", "upstreamVersion": "1.0.0"}"#,
        )
        .extra_file(
            "packages/pkg-c/manifest.json",
            r#"{"name": "pkg-c", "version": "3.0.0", "upstreamVersion": "2.0.0"}"#,
        )
        .build()
}

pub fn create_workspace_with_json_extra_fields() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member("crate-a", "1.0.0")
        .with_changeset_dir()
        .workspace_toml_extra(
            r#"[[workspace.metadata.changeset.additional-packages]]
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
        .extra_file(
            "packages/my-app/app.json",
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
        .build()
}

pub fn create_workspace_with_multiple_deps_on_same_crate() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member("crate-a", "1.0.0")
        .with_changeset_dir()
        .workspace_toml_extra(
            r#"[[workspace.metadata.changeset.additional-packages]]
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
        .extra_file(
            "charts/Chart.yaml",
            "apiVersion: v2\nname: pkg-b\nversion: \"0.1.0\"\nappVersion: \"1.0.0\"\n",
        )
        .extra_file("config/deps.json", r#"{"crateA": {"version": "1.0.0"}}"#)
        .build()
}

pub fn create_workspace_with_invalid_version_field_path() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member("crate-a", "1.0.0")
        .with_changeset_dir()
        .workspace_toml_extra(
            r#"[[workspace.metadata.changeset.additional-packages]]
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
        .extra_file(
            "packages/pkg-b/manifest.json",
            r#"{"name": "pkg-b", "version": "1.0.0"}"#,
        )
        .build()
}

pub fn create_workspace_with_prerelease_dependency() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member("crate-a", "1.0.0-alpha.1")
        .with_changeset_dir()
        .workspace_toml_extra(
            r#"[[workspace.metadata.changeset.additional-packages]]
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
        .extra_file(
            "packages/pkg-b/manifest.json",
            r#"{"name": "pkg-b", "version": "1.0.0", "upstreamVersion": "1.0.0-alpha.1"}"#,
        )
        .build()
}

pub fn create_workspace_with_deeply_nested_json_field() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member("crate-a", "1.0.0")
        .with_changeset_dir()
        .workspace_toml_extra(
            r#"[[workspace.metadata.changeset.additional-packages]]
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
        .extra_file(
            "packages/pkg-b/manifest.json",
            r#"{"name": "pkg-b", "version": "1.0.0", "metadata": {"versions": {"upstream_crate": "1.0.0"}}}"#,
        )
        .build()
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

pub fn create_virtual_workspace_with_git() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member("crate-a", "0.1.0")
        .crate_member("crate-b", "0.2.0")
        .with_git()
        .build()
}

pub fn create_workspace_with_three_crates_and_git() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member("crate-a", "0.1.0")
        .crate_member("crate-b", "0.2.0")
        .crate_member("crate-c", "0.3.0")
        .with_git()
        .build()
}

pub fn create_workspace_with_helm_chart_and_git() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member("crate-a", "1.0.0")
        .with_changeset_dir()
        .with_git()
        .workspace_toml_extra(
            r#"[[workspace.metadata.changeset.additional-packages]]
name = "my-helm-chart"
path = "charts/my-chart"
influence = ["charts/my-chart/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "charts/my-chart/Chart.yaml"
format = "yaml"
version-field-path = "version"
"#,
        )
        .extra_file(
            "charts/my-chart/Chart.yaml",
            "# Helm chart for my-chart\napiVersion: v2\nname: my-chart\ndescription: A test Helm chart\n# This comment should survive release\nversion: \"2.0.0\"\nappVersion: \"1.0.0\"\n",
        )
        .extra_file("charts/my-chart/values.yaml", "replicaCount: 1")
        .build()
}

pub fn create_workspace_with_version_tracking_and_git() -> TempDir {
    WorkspaceBuilder::virtual_workspace()
        .crate_member("crate-a", "1.0.0")
        .with_changeset_dir()
        .with_git()
        .workspace_toml_extra(
            r#"[[workspace.metadata.changeset.additional-packages]]
name = "my-helm-chart"
path = "charts/my-chart"
influence = ["charts/my-chart/**"]

[workspace.metadata.changeset.additional-packages.manifest]
file-path = "charts/my-chart/Chart.yaml"
format = "yaml"
version-field-path = "version"

[[workspace.metadata.changeset.additional-packages.dependencies]]
dependency-name = "crate-a"

[workspace.metadata.changeset.additional-packages.dependencies.version-tracking-manifest]
file-path = "charts/my-chart/Chart.yaml"
format = "yaml"
version-field-path = "appVersion"
"#,
        )
        .extra_file(
            "charts/my-chart/Chart.yaml",
            "apiVersion: v2\nname: my-chart\nversion: \"2.0.0\"\nappVersion: \"1.0.0\"\n",
        )
        .extra_file("charts/my-chart/values.yaml", "replicaCount: 1\n")
        .build()
}

pub fn write_file(base: &Path, relative: &str, content: &str) {
    let path = base.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create parent dirs");
    }
    fs::write(&path, content).expect("failed to write file");
}
