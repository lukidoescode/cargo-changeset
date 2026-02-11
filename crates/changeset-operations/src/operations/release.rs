use std::path::{Path, PathBuf};

use changeset_core::{BumpType, PackageInfo};
use changeset_version::{bump_version, max_bump_type};
use indexmap::IndexMap;
use semver::Version;

use crate::Result;
use crate::error::OperationError;
use crate::traits::{ChangesetReader, ManifestWriter, ProjectProvider};

pub struct ReleaseInput {
    pub dry_run: bool,
    pub convert_inherited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageVersion {
    pub name: String,
    pub current_version: Version,
    pub new_version: Version,
    pub bump_type: BumpType,
}

#[derive(Debug, Clone)]
pub struct ReleaseOutput {
    pub planned_releases: Vec<PackageVersion>,
    pub unchanged_packages: Vec<String>,
    pub changesets_consumed: Vec<PathBuf>,
}

#[derive(Debug)]
pub enum ReleaseOutcome {
    DryRun(ReleaseOutput),
    Executed(ReleaseOutput),
    NoChangesets,
}

pub struct ReleaseOperation<P, R, M> {
    project_provider: P,
    changeset_reader: R,
    manifest_writer: M,
}

impl<P, R, M> ReleaseOperation<P, R, M>
where
    P: ProjectProvider,
    R: ChangesetReader,
    M: ManifestWriter,
{
    pub fn new(project_provider: P, changeset_reader: R, manifest_writer: M) -> Self {
        Self {
            project_provider,
            changeset_reader,
            manifest_writer,
        }
    }

    fn find_packages_with_inherited_versions(
        &self,
        packages: &[PackageInfo],
    ) -> Result<Vec<String>> {
        let mut inherited = Vec::new();
        for pkg in packages {
            let manifest_path = pkg.path.join("Cargo.toml");
            if self.manifest_writer.has_inherited_version(&manifest_path)? {
                inherited.push(pkg.name.clone());
            }
        }
        Ok(inherited)
    }

    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered, changeset files
    /// cannot be read, or manifest updates fail.
    pub fn execute(&self, start_path: &Path, input: &ReleaseInput) -> Result<ReleaseOutcome> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;

        let changeset_dir = project.root.join(root_config.changeset_dir());
        let changeset_files = self.changeset_reader.list_changesets(&changeset_dir)?;

        if changeset_files.is_empty() {
            return Ok(ReleaseOutcome::NoChangesets);
        }

        let inherited_packages = self.find_packages_with_inherited_versions(&project.packages)?;
        if !inherited_packages.is_empty() && !input.convert_inherited {
            return Err(OperationError::InheritedVersionsRequireConvert {
                packages: inherited_packages,
            });
        }

        let mut bumps_by_package: IndexMap<String, Vec<BumpType>> = IndexMap::new();

        for path in &changeset_files {
            let changeset = self.changeset_reader.read_changeset(path)?;
            for release in &changeset.releases {
                bumps_by_package
                    .entry(release.name.clone())
                    .or_default()
                    .push(release.bump_type);
            }
        }

        let package_lookup: IndexMap<_, _> = project
            .packages
            .iter()
            .map(|p| (p.name.clone(), p.clone()))
            .collect();

        let mut planned_releases = Vec::new();

        for (name, bumps) in &bumps_by_package {
            if let Some(bump_type) = max_bump_type(bumps) {
                if let Some(pkg) = package_lookup.get(name) {
                    let new_version = bump_version(&pkg.version, bump_type);
                    planned_releases.push(PackageVersion {
                        name: name.clone(),
                        current_version: pkg.version.clone(),
                        new_version,
                        bump_type,
                    });
                }
            }
        }

        let packages_with_releases: std::collections::HashSet<_> =
            planned_releases.iter().map(|r| r.name.clone()).collect();

        let unchanged_packages: Vec<String> = project
            .packages
            .iter()
            .filter(|p| !packages_with_releases.contains(&p.name))
            .map(|p| p.name.clone())
            .collect();

        let output = ReleaseOutput {
            planned_releases: planned_releases.clone(),
            unchanged_packages,
            changesets_consumed: changeset_files,
        };

        if input.dry_run {
            return Ok(ReleaseOutcome::DryRun(output));
        }

        if !inherited_packages.is_empty() {
            let root_manifest = project.root.join("Cargo.toml");
            self.manifest_writer
                .remove_workspace_version(&root_manifest)?;
        }

        for release in &planned_releases {
            if let Some(pkg) = package_lookup.get(&release.name) {
                let manifest_path = pkg.path.join("Cargo.toml");
                self.manifest_writer
                    .write_version(&manifest_path, &release.new_version)?;
                self.manifest_writer
                    .verify_version(&manifest_path, &release.new_version)?;
            }
        }

        Ok(ReleaseOutcome::Executed(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{
        MockChangesetReader, MockManifestWriter, MockProjectProvider, make_changeset,
    };

    fn default_input() -> ReleaseInput {
        ReleaseInput {
            dry_run: true,
            convert_inherited: false,
        }
    }

    #[test]
    fn returns_no_changesets_when_empty() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset_reader = MockChangesetReader::new();
        let manifest_writer = MockManifestWriter::new();

        let operation = ReleaseOperation::new(project_provider, changeset_reader, manifest_writer);

        let result = operation
            .execute(Path::new("/any"), &default_input())
            .expect("execute failed");

        assert!(matches!(result, ReleaseOutcome::NoChangesets));
    }

    #[test]
    fn calculates_single_patch_bump() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix a bug");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();

        let operation = ReleaseOperation::new(project_provider, changeset_reader, manifest_writer);

        let result = operation
            .execute(Path::new("/any"), &default_input())
            .expect("execute failed");

        let ReleaseOutcome::DryRun(output) = result else {
            panic!("expected DryRun outcome");
        };

        assert_eq!(output.planned_releases.len(), 1);
        let release = &output.planned_releases[0];
        assert_eq!(release.name, "my-crate");
        assert_eq!(release.current_version.to_string(), "1.0.0");
        assert_eq!(release.new_version.to_string(), "1.0.1");
        assert_eq!(release.bump_type, BumpType::Patch);
    }

    #[test]
    fn takes_maximum_bump_from_multiple_changesets() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.2.3");
        let changeset1 = make_changeset("my-crate", BumpType::Patch, "Fix bug");
        let changeset2 = make_changeset("my-crate", BumpType::Minor, "Add feature");

        let changeset_reader = MockChangesetReader::new().with_changesets(vec![
            (PathBuf::from(".changeset/fix.md"), changeset1),
            (PathBuf::from(".changeset/feature.md"), changeset2),
        ]);
        let manifest_writer = MockManifestWriter::new();

        let operation = ReleaseOperation::new(project_provider, changeset_reader, manifest_writer);

        let result = operation
            .execute(Path::new("/any"), &default_input())
            .expect("execute failed");

        let ReleaseOutcome::DryRun(output) = result else {
            panic!("expected DryRun outcome");
        };

        assert_eq!(output.planned_releases.len(), 1);
        let release = &output.planned_releases[0];
        assert_eq!(release.new_version.to_string(), "1.3.0");
        assert_eq!(release.bump_type, BumpType::Minor);
    }

    #[test]
    fn handles_workspace_with_multiple_packages() {
        let project_provider =
            MockProjectProvider::workspace(vec![("crate-a", "1.0.0"), ("crate-b", "2.0.0")]);

        let changeset1 = make_changeset("crate-a", BumpType::Minor, "Add feature to A");
        let changeset2 = make_changeset("crate-b", BumpType::Major, "Breaking change in B");

        let changeset_reader = MockChangesetReader::new().with_changesets(vec![
            (PathBuf::from(".changeset/feature-a.md"), changeset1),
            (PathBuf::from(".changeset/breaking-b.md"), changeset2),
        ]);
        let manifest_writer = MockManifestWriter::new();

        let operation = ReleaseOperation::new(project_provider, changeset_reader, manifest_writer);

        let result = operation
            .execute(Path::new("/any"), &default_input())
            .expect("execute failed");

        let ReleaseOutcome::DryRun(output) = result else {
            panic!("expected DryRun outcome");
        };

        assert_eq!(output.planned_releases.len(), 2);
        assert!(output.unchanged_packages.is_empty());

        let crate_a = output
            .planned_releases
            .iter()
            .find(|r| r.name == "crate-a")
            .expect("crate-a should be in releases");
        assert_eq!(crate_a.new_version.to_string(), "1.1.0");

        let crate_b = output
            .planned_releases
            .iter()
            .find(|r| r.name == "crate-b")
            .expect("crate-b should be in releases");
        assert_eq!(crate_b.new_version.to_string(), "3.0.0");
    }

    #[test]
    fn identifies_unchanged_packages() {
        let project_provider = MockProjectProvider::workspace(vec![
            ("crate-a", "1.0.0"),
            ("crate-b", "2.0.0"),
            ("crate-c", "3.0.0"),
        ]);

        let changeset = make_changeset("crate-a", BumpType::Patch, "Fix crate-a");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();

        let operation = ReleaseOperation::new(project_provider, changeset_reader, manifest_writer);

        let result = operation
            .execute(Path::new("/any"), &default_input())
            .expect("execute failed");

        let ReleaseOutcome::DryRun(output) = result else {
            panic!("expected DryRun outcome");
        };

        assert_eq!(output.planned_releases.len(), 1);
        assert_eq!(output.unchanged_packages.len(), 2);
        assert!(output.unchanged_packages.contains(&"crate-b".to_string()));
        assert!(output.unchanged_packages.contains(&"crate-c".to_string()));
    }

    #[test]
    fn tracks_consumed_changeset_files() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset1 = make_changeset("my-crate", BumpType::Patch, "Fix 1");
        let changeset2 = make_changeset("my-crate", BumpType::Patch, "Fix 2");

        let changeset_reader = MockChangesetReader::new().with_changesets(vec![
            (PathBuf::from(".changeset/fix1.md"), changeset1),
            (PathBuf::from(".changeset/fix2.md"), changeset2),
        ]);
        let manifest_writer = MockManifestWriter::new();

        let operation = ReleaseOperation::new(project_provider, changeset_reader, manifest_writer);

        let result = operation
            .execute(Path::new("/any"), &default_input())
            .expect("execute failed");

        let ReleaseOutcome::DryRun(output) = result else {
            panic!("expected DryRun outcome");
        };

        assert_eq!(output.changesets_consumed.len(), 2);
    }

    #[test]
    fn returns_executed_when_not_dry_run() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new();

        let operation = ReleaseOperation::new(project_provider, changeset_reader, manifest_writer);
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
        };

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed");

        assert!(matches!(result, ReleaseOutcome::Executed(_)));
    }

    #[test]
    fn writes_versions_when_not_dry_run() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Minor, "Add feature");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/feature.md"), changeset);
        let manifest_writer = Arc::new(MockManifestWriter::new());

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            Arc::clone(&manifest_writer),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
        };

        let ReleaseOutcome::Executed(output) = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed")
        else {
            panic!("expected Executed outcome");
        };

        assert_eq!(output.planned_releases.len(), 1);
        assert_eq!(output.planned_releases[0].new_version.to_string(), "1.1.0");

        let written = manifest_writer.written_versions();
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].0, PathBuf::from("/mock/project/Cargo.toml"));
        assert_eq!(written[0].1.to_string(), "1.1.0");
    }

    #[test]
    fn returns_error_when_inherited_without_convert_flag() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new()
            .with_inherited(vec![PathBuf::from("/mock/project/Cargo.toml")]);

        let operation = ReleaseOperation::new(project_provider, changeset_reader, manifest_writer);
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: false,
        };

        let result = operation.execute(Path::new("/any"), &input);

        assert!(matches!(
            result,
            Err(OperationError::InheritedVersionsRequireConvert { .. })
        ));
    }

    #[test]
    fn allows_inherited_with_convert_flag() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);
        let manifest_writer = MockManifestWriter::new()
            .with_inherited(vec![PathBuf::from("/mock/project/Cargo.toml")]);

        let operation = ReleaseOperation::new(project_provider, changeset_reader, manifest_writer);
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: true,
        };

        let result = operation.execute(Path::new("/any"), &input);

        assert!(result.is_ok());
    }

    #[test]
    fn removes_workspace_version_when_converting_inherited() {
        use std::sync::Arc;

        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let changeset = make_changeset("my-crate", BumpType::Patch, "Fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/fix.md"), changeset);
        let manifest_writer = Arc::new(
            MockManifestWriter::new()
                .with_inherited(vec![PathBuf::from("/mock/project/Cargo.toml")]),
        );

        let operation = ReleaseOperation::new(
            project_provider,
            changeset_reader,
            Arc::clone(&manifest_writer),
        );
        let input = ReleaseInput {
            dry_run: false,
            convert_inherited: true,
        };

        let ReleaseOutcome::Executed(_) = operation
            .execute(Path::new("/any"), &input)
            .expect("execute failed")
        else {
            panic!("expected Executed outcome");
        };

        assert!(
            manifest_writer.workspace_version_removed(),
            "workspace version should be removed"
        );
    }
}
