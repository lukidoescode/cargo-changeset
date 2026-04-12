use std::path::{Path, PathBuf};

use changeset_core::{
    AdditionalPackageDeclaration, AdditionalPackageManifest, CARGO_MANIFEST_FILENAME,
    ManifestFormat,
};
use changeset_manifest::{AdditionalPackageUpdate, MetadataSection};
use changeset_project::ProjectKind;
use globset::GlobBuilder;

use crate::Result;
use crate::error::OperationError;
use crate::traits::{
    AdditionalPackageConfigWriter, AdditionalPackageField, AdditionalPackageInteractionProvider,
    MenuSelection, ProjectProvider,
};

pub struct AdditionalPackageAddInput {
    pub name: String,
    pub path: PathBuf,
    pub influence: Vec<String>,
    pub manifest_file_path: PathBuf,
    pub manifest_format: ManifestFormat,
    pub manifest_version_field_path: String,
}

pub struct AdditionalPackageEditInput {
    pub name: String,
    pub updates: AdditionalPackageUpdate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdditionalPackageEvent {
    Added { name: String },
    Removed { name: String },
    Updated { name: String, field: String },
    Listed(Vec<AdditionalPackageSummaryData>),
    NotFound { name: String },
    AlreadyExists { name: String },
    NoAdditionalPackages,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdditionalPackageSummaryData {
    pub name: String,
    pub path: PathBuf,
    pub manifest_file_path: PathBuf,
    pub manifest_format: ManifestFormat,
}

pub struct AdditionalPackageDirectAddOperation<P, W> {
    project_provider: P,
    writer: W,
}

impl<P, W> AdditionalPackageDirectAddOperation<P, W>
where
    P: ProjectProvider,
    W: AdditionalPackageConfigWriter,
{
    #[must_use]
    pub fn new(project_provider: P, writer: W) -> Self {
        Self {
            project_provider,
            writer,
        }
    }

    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered, configs cannot be loaded,
    /// glob patterns are invalid, the manifest file does not exist, or the manifest
    /// cannot be written.
    pub fn execute(
        &self,
        start_path: &Path,
        input: AdditionalPackageAddInput,
    ) -> Result<Vec<AdditionalPackageEvent>> {
        let project = self.project_provider.discover_project(start_path)?;
        require_workspace(&project)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;
        write_additional_package(&self.writer, &project, &root_config, input)
    }
}

pub struct AdditionalPackageDirectRemoveOperation<P, W> {
    project_provider: P,
    writer: W,
}

impl<P, W> AdditionalPackageDirectRemoveOperation<P, W>
where
    P: ProjectProvider,
    W: AdditionalPackageConfigWriter,
{
    #[must_use]
    pub fn new(project_provider: P, writer: W) -> Self {
        Self {
            project_provider,
            writer,
        }
    }

    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered, configs cannot be loaded,
    /// or the manifest cannot be written.
    pub fn execute(&self, start_path: &Path, name: &str) -> Result<Vec<AdditionalPackageEvent>> {
        let project = self.project_provider.discover_project(start_path)?;
        require_workspace(&project)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;

        if !root_config
            .additional_packages()
            .iter()
            .any(|p| p.name() == name)
        {
            return Err(OperationError::AdditionalPackageNotFound {
                name: name.to_string(),
            });
        }

        let (manifest_path, section) = resolve_manifest_and_section(&project);

        self.writer
            .remove_additional_package(&manifest_path, section, name)?;

        Ok(vec![AdditionalPackageEvent::Removed {
            name: name.to_string(),
        }])
    }
}

pub struct AdditionalPackageDirectEditOperation<P, W> {
    project_provider: P,
    writer: W,
}

impl<P, W> AdditionalPackageDirectEditOperation<P, W>
where
    P: ProjectProvider,
    W: AdditionalPackageConfigWriter,
{
    #[must_use]
    pub fn new(project_provider: P, writer: W) -> Self {
        Self {
            project_provider,
            writer,
        }
    }

    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered, configs cannot be loaded,
    /// glob patterns are invalid, the new manifest file path does not exist, or the
    /// manifest cannot be written.
    pub fn execute(
        &self,
        start_path: &Path,
        input: AdditionalPackageEditInput,
    ) -> Result<Vec<AdditionalPackageEvent>> {
        let project = self.project_provider.discover_project(start_path)?;
        require_workspace(&project)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;

        if !root_config
            .additional_packages()
            .iter()
            .any(|p| p.name() == &input.name)
        {
            return Err(OperationError::AdditionalPackageNotFound { name: input.name });
        }

        if let Some(ref influence) = input.updates.influence {
            validate_influence_patterns(&input.name, influence)?;
        }

        if let Some(ref manifest_path) = input.updates.manifest_file_path
            && !manifest_path.exists()
        {
            return Err(OperationError::AdditionalPackageManifestNotFound {
                name: input.name,
                path: manifest_path.clone(),
            });
        }

        let (manifest_path, section) = resolve_manifest_and_section(&project);

        let changed_fields = describe_updated_fields(&input.updates);

        self.writer.update_additional_package(
            &manifest_path,
            section,
            &input.name,
            &input.updates,
        )?;

        Ok(vec![AdditionalPackageEvent::Updated {
            name: input.name,
            field: changed_fields,
        }])
    }
}

pub struct AdditionalPackageListOperation<P> {
    project_provider: P,
}

impl<P> AdditionalPackageListOperation<P>
where
    P: ProjectProvider,
{
    #[must_use]
    pub fn new(project_provider: P) -> Self {
        Self { project_provider }
    }

    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered or configs cannot be loaded.
    pub fn execute(&self, start_path: &Path) -> Result<Vec<AdditionalPackageEvent>> {
        let project = self.project_provider.discover_project(start_path)?;
        require_workspace(&project)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;

        let packages = root_config.additional_packages();

        if packages.is_empty() {
            return Ok(vec![AdditionalPackageEvent::NoAdditionalPackages]);
        }

        let summaries = packages
            .iter()
            .map(|p| AdditionalPackageSummaryData {
                name: p.name().clone(),
                path: p.path().clone(),
                manifest_file_path: p.manifest().file_path().clone(),
                manifest_format: p.manifest().format(),
            })
            .collect();

        Ok(vec![AdditionalPackageEvent::Listed(summaries)])
    }
}

pub struct AdditionalPackageInteractiveAddOperation<P, W, I> {
    project_provider: P,
    writer: W,
    interaction: I,
}

impl<P, W, I> AdditionalPackageInteractiveAddOperation<P, W, I>
where
    P: ProjectProvider,
    W: AdditionalPackageConfigWriter,
    I: AdditionalPackageInteractionProvider,
{
    #[must_use]
    pub fn new(project_provider: P, writer: W, interaction: I) -> Self {
        Self {
            project_provider,
            writer,
            interaction,
        }
    }

    /// # Errors
    ///
    /// Returns an error if the project is not a workspace, any user prompt fails,
    /// glob patterns are invalid, the manifest file does not exist, or the manifest
    /// cannot be written.
    pub fn execute(&self, start_path: &Path) -> Result<Vec<AdditionalPackageEvent>> {
        let project = self.project_provider.discover_project(start_path)?;
        require_workspace(&project)?;

        let name = self.interaction.prompt_package_name()?;
        let path = self.interaction.prompt_package_path()?;
        let influence = self.interaction.prompt_influence_patterns(&path)?;
        let manifest_file_path = self.interaction.prompt_manifest_file_path()?;
        let manifest_format = self.interaction.prompt_manifest_format()?;
        let manifest_version_field_path = self.interaction.prompt_manifest_version_field_path()?;

        let (root_config, _) = self.project_provider.load_configs(&project)?;
        write_additional_package(
            &self.writer,
            &project,
            &root_config,
            AdditionalPackageAddInput {
                name,
                path,
                influence,
                manifest_file_path,
                manifest_format,
                manifest_version_field_path,
            },
        )
    }
}

pub struct AdditionalPackageInteractiveRemoveOperation<P, W, I> {
    project_provider: P,
    writer: W,
    interaction: I,
}

impl<P, W, I> AdditionalPackageInteractiveRemoveOperation<P, W, I>
where
    P: ProjectProvider,
    W: AdditionalPackageConfigWriter,
    I: AdditionalPackageInteractionProvider,
{
    #[must_use]
    pub fn new(project_provider: P, writer: W, interaction: I) -> Self {
        Self {
            project_provider,
            writer,
            interaction,
        }
    }

    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered, configs cannot be loaded,
    /// the selection prompt fails, or the manifest cannot be written.
    pub fn execute(&self, start_path: &Path) -> Result<Vec<AdditionalPackageEvent>> {
        let project = self.project_provider.discover_project(start_path)?;
        require_workspace(&project)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;

        let packages: Vec<&AdditionalPackageDeclaration> =
            root_config.additional_packages().iter().collect();

        if packages.is_empty() {
            return Ok(vec![AdditionalPackageEvent::NoAdditionalPackages]);
        }

        let selection = self.interaction.select_package_to_remove(&packages)?;
        let MenuSelection::Selected(index) = selection else {
            return Ok(vec![]);
        };

        let name = packages[index].name().clone();

        if !self.interaction.confirm_removal(&name)? {
            return Ok(vec![]);
        }

        let (manifest_path, section) = resolve_manifest_and_section(&project);
        self.writer
            .remove_additional_package(&manifest_path, section, &name)?;

        Ok(vec![AdditionalPackageEvent::Removed { name }])
    }
}

pub struct AdditionalPackageInteractiveEditOperation<P, W, I> {
    project_provider: P,
    writer: W,
    interaction: I,
}

impl<P, W, I> AdditionalPackageInteractiveEditOperation<P, W, I>
where
    P: ProjectProvider,
    W: AdditionalPackageConfigWriter,
    I: AdditionalPackageInteractionProvider,
{
    #[must_use]
    pub fn new(project_provider: P, writer: W, interaction: I) -> Self {
        Self {
            project_provider,
            writer,
            interaction,
        }
    }

    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered, configs cannot be loaded,
    /// any interactive prompt fails, glob patterns are invalid, or the manifest cannot
    /// be written.
    pub fn execute(&self, start_path: &Path) -> Result<Vec<AdditionalPackageEvent>> {
        let project = self.project_provider.discover_project(start_path)?;
        require_workspace(&project)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;

        let packages: Vec<&AdditionalPackageDeclaration> =
            root_config.additional_packages().iter().collect();

        if packages.is_empty() {
            return Ok(vec![AdditionalPackageEvent::NoAdditionalPackages]);
        }

        let selection = self.interaction.select_package_to_edit(&packages)?;
        let MenuSelection::Selected(index) = selection else {
            return Ok(vec![]);
        };

        let name = packages[index].name().clone();
        let mut updates = AdditionalPackageUpdate {
            path: None,
            influence: None,
            manifest_file_path: None,
            manifest_format: None,
            manifest_version_field_path: None,
        };
        let mut changed_fields: Vec<String> = Vec::new();

        loop {
            let field_selection = self.interaction.select_field_to_edit()?;
            let MenuSelection::Selected(field) = field_selection else {
                break;
            };

            match field {
                AdditionalPackageField::Path => {
                    updates.path = Some(self.interaction.prompt_package_path()?);
                    changed_fields.push("path".to_string());
                }
                AdditionalPackageField::Influence => {
                    let current_path = updates
                        .path
                        .as_deref()
                        .unwrap_or_else(|| packages[index].path());
                    updates.influence =
                        Some(self.interaction.prompt_influence_patterns(current_path)?);
                    changed_fields.push("influence".to_string());
                }
                AdditionalPackageField::ManifestFilePath => {
                    updates.manifest_file_path =
                        Some(self.interaction.prompt_manifest_file_path()?);
                    changed_fields.push("manifest.file-path".to_string());
                }
                AdditionalPackageField::ManifestFormat => {
                    updates.manifest_format = Some(self.interaction.prompt_manifest_format()?);
                    changed_fields.push("manifest.format".to_string());
                }
                AdditionalPackageField::ManifestVersionFieldPath => {
                    updates.manifest_version_field_path =
                        Some(self.interaction.prompt_manifest_version_field_path()?);
                    changed_fields.push("manifest.version-field-path".to_string());
                }
            }
        }

        if changed_fields.is_empty() {
            return Ok(vec![]);
        }

        let (manifest_path, section) = resolve_manifest_and_section(&project);
        self.writer
            .update_additional_package(&manifest_path, section, &name, &updates)?;

        Ok(vec![AdditionalPackageEvent::Updated {
            name,
            field: changed_fields.join(", "),
        }])
    }
}

fn write_additional_package<W>(
    writer: &W,
    project: &changeset_project::CargoProject,
    root_config: &changeset_project::RootChangesetConfig,
    input: AdditionalPackageAddInput,
) -> Result<Vec<AdditionalPackageEvent>>
where
    W: AdditionalPackageConfigWriter,
{
    let existing = root_config.additional_packages();
    if existing.iter().any(|p| p.name() == &input.name) {
        return Err(OperationError::AdditionalPackageAlreadyExists { name: input.name });
    }

    if project.packages().iter().any(|p| p.name() == &input.name) {
        return Err(OperationError::AdditionalPackageAlreadyExists { name: input.name });
    }

    if !input.manifest_file_path.exists() {
        return Err(OperationError::AdditionalPackageManifestNotFound {
            name: input.name,
            path: input.manifest_file_path,
        });
    }

    validate_influence_patterns(&input.name, &input.influence)?;

    let declaration = AdditionalPackageDeclaration::new(
        input.name.clone(),
        input.path,
        input.influence,
        AdditionalPackageManifest::new(
            input.manifest_file_path,
            input.manifest_format,
            input.manifest_version_field_path,
        ),
    );

    let (manifest_path, section) = resolve_manifest_and_section(project);

    writer.add_additional_package(&manifest_path, section, &declaration)?;

    Ok(vec![AdditionalPackageEvent::Added { name: input.name }])
}

fn require_workspace(project: &changeset_project::CargoProject) -> Result<()> {
    if *project.kind() == ProjectKind::SinglePackage {
        return Err(OperationError::AdditionalPackagesRequireWorkspace);
    }
    Ok(())
}

fn resolve_manifest_and_section(
    project: &changeset_project::CargoProject,
) -> (PathBuf, MetadataSection) {
    let manifest_path = project.root().join(CARGO_MANIFEST_FILENAME);
    let section = match project.kind() {
        ProjectKind::VirtualWorkspace | ProjectKind::WorkspaceWithRoot => {
            MetadataSection::Workspace
        }
        ProjectKind::SinglePackage => {
            unreachable!("require_workspace() must be called before this function")
        }
    };
    (manifest_path, section)
}

fn validate_influence_patterns(name: &str, patterns: &[String]) -> Result<()> {
    for pattern in patterns {
        GlobBuilder::new(pattern).build().map_err(|source| {
            OperationError::AdditionalPackageInvalidGlob {
                name: name.to_string(),
                pattern: pattern.clone(),
                source,
            }
        })?;
    }
    Ok(())
}

fn describe_updated_fields(updates: &AdditionalPackageUpdate) -> String {
    let mut fields = Vec::new();
    if updates.path.is_some() {
        fields.push("path");
    }
    if updates.influence.is_some() {
        fields.push("influence");
    }
    if updates.manifest_file_path.is_some() {
        fields.push("manifest.file-path");
    }
    if updates.manifest_format.is_some() {
        fields.push("manifest.format");
    }
    if updates.manifest_version_field_path.is_some() {
        fields.push("manifest.version-field-path");
    }
    if fields.is_empty() {
        "none".to_string()
    } else {
        fields.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use changeset_core::{AdditionalPackageManifest, ManifestFormat, PackageInfo};
    use changeset_project::RootChangesetConfig;
    use tempfile::TempDir;

    use super::*;
    use crate::mocks::{MockAdditionalPackageConfigWriter, MockProjectProvider};

    fn make_decl(name: &str) -> AdditionalPackageDeclaration {
        AdditionalPackageDeclaration::new(
            name.to_string(),
            PathBuf::from(format!("charts/{name}")),
            vec![format!("charts/{name}/**")],
            AdditionalPackageManifest::new(
                PathBuf::from(format!("charts/{name}/Chart.yaml")),
                ManifestFormat::Yaml,
                "version".to_string(),
            ),
        )
    }

    fn make_project_with_packages(
        additional: Vec<AdditionalPackageDeclaration>,
    ) -> MockProjectProvider {
        let config = RootChangesetConfig::default().with_additional_packages(additional);
        MockProjectProvider::workspace(vec![("crate-a", "1.0.0")]).with_root_config(config)
    }

    fn make_temp_manifest_file() -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("create temp dir");
        let path = dir.path().join("Chart.yaml");
        std::fs::write(&path, "version: \"1.0.0\"\n").expect("write manifest");
        (dir, path)
    }

    #[test]
    fn direct_add_succeeds_with_valid_input() {
        let (_dir, manifest_file) = make_temp_manifest_file();
        let provider = make_project_with_packages(vec![]);
        let writer = MockAdditionalPackageConfigWriter::new();

        let op = AdditionalPackageDirectAddOperation::new(provider, writer);
        let input = AdditionalPackageAddInput {
            name: "my-chart".to_string(),
            path: PathBuf::from("charts/my-chart"),
            influence: vec!["charts/my-chart/**".to_string()],
            manifest_file_path: manifest_file,
            manifest_format: ManifestFormat::Yaml,
            manifest_version_field_path: "version".to_string(),
        };

        let events = op
            .execute(Path::new("/any"), input)
            .expect("should succeed");
        assert!(
            events
                .iter()
                .any(|e| matches!(e, AdditionalPackageEvent::Added { name } if name == "my-chart"))
        );
    }

    #[test]
    fn direct_add_rejects_duplicate_name() {
        let (_dir, manifest_file) = make_temp_manifest_file();
        let provider = make_project_with_packages(vec![make_decl("my-chart")]);
        let writer = MockAdditionalPackageConfigWriter::new();

        let op = AdditionalPackageDirectAddOperation::new(provider, writer);
        let input = AdditionalPackageAddInput {
            name: "my-chart".to_string(),
            path: PathBuf::from("charts/my-chart"),
            influence: vec![],
            manifest_file_path: manifest_file,
            manifest_format: ManifestFormat::Yaml,
            manifest_version_field_path: "version".to_string(),
        };

        let result = op.execute(Path::new("/any"), input);
        assert!(matches!(
            result,
            Err(OperationError::AdditionalPackageAlreadyExists { name }) if name == "my-chart"
        ));
    }

    #[test]
    fn direct_add_rejects_name_collision_with_rust_crate() {
        let (_dir, manifest_file) = make_temp_manifest_file();
        let provider = make_project_with_packages(vec![]);
        let writer = MockAdditionalPackageConfigWriter::new();

        let op = AdditionalPackageDirectAddOperation::new(provider, writer);
        let input = AdditionalPackageAddInput {
            name: "crate-a".to_string(),
            path: PathBuf::from("crate-a"),
            influence: vec![],
            manifest_file_path: manifest_file,
            manifest_format: ManifestFormat::Yaml,
            manifest_version_field_path: "version".to_string(),
        };

        let result = op.execute(Path::new("/any"), input);
        assert!(matches!(
            result,
            Err(OperationError::AdditionalPackageAlreadyExists { .. })
        ));
    }

    #[test]
    fn direct_add_validates_manifest_file_exists() {
        let provider = make_project_with_packages(vec![]);
        let writer = MockAdditionalPackageConfigWriter::new();

        let op = AdditionalPackageDirectAddOperation::new(provider, writer);
        let input = AdditionalPackageAddInput {
            name: "my-chart".to_string(),
            path: PathBuf::from("charts/my-chart"),
            influence: vec![],
            manifest_file_path: PathBuf::from("/nonexistent/Chart.yaml"),
            manifest_format: ManifestFormat::Yaml,
            manifest_version_field_path: "version".to_string(),
        };

        let result = op.execute(Path::new("/any"), input);
        assert!(matches!(
            result,
            Err(OperationError::AdditionalPackageManifestNotFound { .. })
        ));
    }

    #[test]
    fn direct_add_validates_glob_patterns() {
        let (_dir, manifest_file) = make_temp_manifest_file();
        let provider = make_project_with_packages(vec![]);
        let writer = MockAdditionalPackageConfigWriter::new();

        let op = AdditionalPackageDirectAddOperation::new(provider, writer);
        let input = AdditionalPackageAddInput {
            name: "my-chart".to_string(),
            path: PathBuf::from("charts/my-chart"),
            influence: vec!["[invalid".to_string()],
            manifest_file_path: manifest_file,
            manifest_format: ManifestFormat::Yaml,
            manifest_version_field_path: "version".to_string(),
        };

        let result = op.execute(Path::new("/any"), input);
        assert!(matches!(
            result,
            Err(OperationError::AdditionalPackageInvalidGlob { .. })
        ));
    }

    #[test]
    fn direct_remove_succeeds() {
        let provider = make_project_with_packages(vec![make_decl("my-chart")]);
        let writer = MockAdditionalPackageConfigWriter::new();

        let op = AdditionalPackageDirectRemoveOperation::new(provider, writer);
        let events = op
            .execute(Path::new("/any"), "my-chart")
            .expect("should succeed");

        assert!(
            events.iter().any(
                |e| matches!(e, AdditionalPackageEvent::Removed { name } if name == "my-chart")
            )
        );
    }

    #[test]
    fn direct_remove_nonexistent() {
        let provider = make_project_with_packages(vec![]);
        let writer = MockAdditionalPackageConfigWriter::new();

        let op = AdditionalPackageDirectRemoveOperation::new(provider, writer);
        let result = op.execute(Path::new("/any"), "nonexistent");

        assert!(matches!(
            result,
            Err(OperationError::AdditionalPackageNotFound { .. })
        ));
    }

    #[test]
    fn direct_edit_updates_fields() {
        let provider = make_project_with_packages(vec![make_decl("my-chart")]);
        let writer = MockAdditionalPackageConfigWriter::new();

        let op = AdditionalPackageDirectEditOperation::new(provider, writer);
        let input = AdditionalPackageEditInput {
            name: "my-chart".to_string(),
            updates: AdditionalPackageUpdate {
                path: Some(PathBuf::from("new/path")),
                influence: None,
                manifest_file_path: None,
                manifest_format: None,
                manifest_version_field_path: None,
            },
        };

        let events = op
            .execute(Path::new("/any"), input)
            .expect("should succeed");
        assert!(events.iter().any(
            |e| matches!(e, AdditionalPackageEvent::Updated { name, .. } if name == "my-chart")
        ));
    }

    #[test]
    fn direct_edit_nonexistent() {
        let provider = make_project_with_packages(vec![]);
        let writer = MockAdditionalPackageConfigWriter::new();

        let op = AdditionalPackageDirectEditOperation::new(provider, writer);
        let input = AdditionalPackageEditInput {
            name: "nonexistent".to_string(),
            updates: AdditionalPackageUpdate {
                path: Some(PathBuf::from("new/path")),
                influence: None,
                manifest_file_path: None,
                manifest_format: None,
                manifest_version_field_path: None,
            },
        };

        let result = op.execute(Path::new("/any"), input);
        assert!(matches!(
            result,
            Err(OperationError::AdditionalPackageNotFound { .. })
        ));
    }

    #[test]
    fn list_returns_all_packages() {
        let provider = make_project_with_packages(vec![make_decl("chart-a"), make_decl("chart-b")]);

        let op = AdditionalPackageListOperation::new(provider);
        let events = op.execute(Path::new("/any")).expect("should succeed");

        let listed = events.iter().find_map(|e| {
            if let AdditionalPackageEvent::Listed(s) = e {
                Some(s)
            } else {
                None
            }
        });
        let summaries = listed.expect("should have Listed event");
        assert_eq!(summaries.len(), 2);
    }

    #[test]
    fn list_empty_returns_no_additional_packages() {
        let provider = make_project_with_packages(vec![]);
        let op = AdditionalPackageListOperation::new(provider);
        let events = op.execute(Path::new("/any")).expect("should succeed");

        assert!(
            events
                .iter()
                .any(|e| matches!(e, AdditionalPackageEvent::NoAdditionalPackages))
        );
    }

    #[test]
    fn additional_packages_from_mock_provider() {
        let info = PackageInfo::new(
            "my-helm-chart".to_string(),
            "1.2.3".parse().expect("valid version"),
            PathBuf::from("/mock/charts/my-helm-chart"),
        );
        let provider = MockProjectProvider::workspace(vec![]).with_additional_packages(vec![info]);

        let project_provider: &dyn crate::traits::ProjectProvider = &provider;
        let project = project_provider
            .discover_project(Path::new("/any"))
            .expect("discover");
        let (root_config, _) = project_provider
            .load_configs(&project)
            .expect("load configs");

        assert_eq!(root_config.additional_packages().len(), 0);
    }

    #[test]
    fn add_rejects_single_package_project() {
        let (_dir, manifest_file) = make_temp_manifest_file();
        let provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let writer = MockAdditionalPackageConfigWriter::new();

        let op = AdditionalPackageDirectAddOperation::new(provider, writer);
        let input = AdditionalPackageAddInput {
            name: "my-chart".to_string(),
            path: PathBuf::from("charts/my-chart"),
            influence: vec![],
            manifest_file_path: manifest_file,
            manifest_format: ManifestFormat::Yaml,
            manifest_version_field_path: "version".to_string(),
        };

        let result = op.execute(Path::new("/any"), input);
        assert!(matches!(
            result,
            Err(OperationError::AdditionalPackagesRequireWorkspace)
        ));
    }

    #[test]
    fn remove_rejects_single_package_project() {
        let provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let writer = MockAdditionalPackageConfigWriter::new();

        let op = AdditionalPackageDirectRemoveOperation::new(provider, writer);
        let result = op.execute(Path::new("/any"), "my-chart");
        assert!(matches!(
            result,
            Err(OperationError::AdditionalPackagesRequireWorkspace)
        ));
    }

    #[test]
    fn edit_rejects_single_package_project() {
        let provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let writer = MockAdditionalPackageConfigWriter::new();

        let op = AdditionalPackageDirectEditOperation::new(provider, writer);
        let input = AdditionalPackageEditInput {
            name: "my-chart".to_string(),
            updates: AdditionalPackageUpdate {
                path: Some(PathBuf::from("new/path")),
                influence: None,
                manifest_file_path: None,
                manifest_format: None,
                manifest_version_field_path: None,
            },
        };

        let result = op.execute(Path::new("/any"), input);
        assert!(matches!(
            result,
            Err(OperationError::AdditionalPackagesRequireWorkspace)
        ));
    }

    #[test]
    fn list_rejects_single_package_project() {
        let provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let op = AdditionalPackageListOperation::new(provider);
        let result = op.execute(Path::new("/any"));
        assert!(matches!(
            result,
            Err(OperationError::AdditionalPackagesRequireWorkspace)
        ));
    }

    #[test]
    fn interactive_add_rejects_single_package_without_prompts() {
        let provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let writer = MockAdditionalPackageConfigWriter::new();
        let interaction = crate::mocks::PanickingAdditionalPackageInteractionProvider;

        let op = AdditionalPackageInteractiveAddOperation::new(provider, writer, interaction);
        let result = op.execute(Path::new("/any"));
        assert!(matches!(
            result,
            Err(OperationError::AdditionalPackagesRequireWorkspace)
        ));
    }

    #[test]
    fn interactive_remove_rejects_single_package_without_prompts() {
        let provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let writer = MockAdditionalPackageConfigWriter::new();
        let interaction = crate::mocks::PanickingAdditionalPackageInteractionProvider;

        let op = AdditionalPackageInteractiveRemoveOperation::new(provider, writer, interaction);
        let result = op.execute(Path::new("/any"));
        assert!(matches!(
            result,
            Err(OperationError::AdditionalPackagesRequireWorkspace)
        ));
    }

    #[test]
    fn interactive_edit_rejects_single_package_without_prompts() {
        let provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let writer = MockAdditionalPackageConfigWriter::new();
        let interaction = crate::mocks::PanickingAdditionalPackageInteractionProvider;

        let op = AdditionalPackageInteractiveEditOperation::new(provider, writer, interaction);
        let result = op.execute(Path::new("/any"));
        assert!(matches!(
            result,
            Err(OperationError::AdditionalPackagesRequireWorkspace)
        ));
    }
}
