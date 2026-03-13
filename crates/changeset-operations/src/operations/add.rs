use std::collections::HashMap;
use std::path::{Path, PathBuf};

use changeset_core::{BumpType, ChangeCategory, Changeset, PackageInfo, PackageRelease};
use indexmap::IndexSet;

use crate::Result;
use crate::error::OperationError;
use crate::traits::{
    BumpSelection, CategorySelection, ChangesetWriter, DependencyGraphProvider, DescriptionInput,
    InteractionProvider, PackageSelection, ProjectProvider,
};

pub struct AddInput {
    pub packages: Vec<String>,
    pub bump: Option<BumpType>,
    pub package_bumps: HashMap<String, BumpType>,
    pub category: ChangeCategory,
    pub description: Option<String>,
    pub exclude_dependents: bool,
}

impl Default for AddInput {
    fn default() -> Self {
        Self {
            packages: Vec::new(),
            bump: None,
            package_bumps: HashMap::new(),
            category: ChangeCategory::Changed,
            description: None,
            exclude_dependents: false,
        }
    }
}

#[derive(Debug)]
pub enum AddResult {
    Created {
        changeset: Changeset,
        file_path: PathBuf,
        uncovered_dependents: Vec<String>,
    },
    Cancelled,
    NoPackages,
}

pub struct AddOperation<P, W, I> {
    project_provider: P,
    changeset_writer: W,
    interaction_provider: I,
}

impl<P, W, I> AddOperation<P, W, I>
where
    P: ProjectProvider + DependencyGraphProvider,
    W: ChangesetWriter,
    I: InteractionProvider,
{
    pub fn new(project_provider: P, changeset_writer: W, interaction_provider: I) -> Self {
        Self {
            project_provider,
            changeset_writer,
            interaction_provider,
        }
    }

    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered, has no packages, or
    /// if the changeset cannot be written.
    pub fn execute(&self, start_path: &Path, input: AddInput) -> Result<AddResult> {
        let project = self.project_provider.discover_project(start_path)?;

        if project.packages().is_empty() {
            return Err(OperationError::EmptyProject(project.root().to_path_buf()));
        }

        let graph = if !input.exclude_dependents && project.packages().len() > 1 {
            Some(self.project_provider.build_dependency_graph(&project)?)
        } else {
            None
        };

        let display_labels: Option<Vec<String>> = graph.as_ref().map(|g| {
            project
                .packages()
                .iter()
                .map(|pkg| {
                    let dependents = g.transitive_dependents(&pkg.name);
                    if dependents.is_empty() {
                        format!("{} ({})", pkg.name, pkg.version)
                    } else {
                        let mut dep_list: Vec<&str> = dependents.into_iter().collect();
                        dep_list.sort_unstable();
                        format!(
                            "{} ({}) [depended on by: {}]",
                            pkg.name,
                            pkg.version,
                            dep_list.join(", ")
                        )
                    }
                })
                .collect()
        });

        let packages =
            match self.select_packages(project.packages(), &input, display_labels.as_deref())? {
                Some(packages) if packages.is_empty() => return Ok(AddResult::NoPackages),
                Some(packages) => packages,
                None => return Ok(AddResult::Cancelled),
            };

        let uncovered_dependents = if let Some(ref g) = graph {
            let selected_names: Vec<&str> = packages.iter().map(|p| p.name.as_str()).collect();
            let dependents = g.transitive_dependents_of_set(&selected_names);
            let mut result: Vec<String> = dependents.into_iter().map(String::from).collect();
            result.sort();
            result
        } else {
            Vec::new()
        };

        let Some(releases) = self.collect_releases(&packages, &input)? else {
            return Ok(AddResult::Cancelled);
        };

        let Some(category) = self.select_category(&input)? else {
            return Ok(AddResult::Cancelled);
        };

        let Some(desc) = self.get_description(&input)? else {
            return Ok(AddResult::Cancelled);
        };

        let description = desc.trim().to_string();
        if description.is_empty() {
            return Err(OperationError::EmptyDescription);
        }

        let changeset = Changeset {
            summary: description,
            releases,
            category,
            consumed_for_prerelease: None,
            graduate: false,
        };

        let (root_config, _) = self.project_provider.load_configs(&project)?;
        let changeset_dir = self
            .project_provider
            .ensure_changeset_dir(&project, &root_config)?;

        let filename = self
            .changeset_writer
            .write_changeset(&changeset_dir, &changeset)?;
        let file_path = changeset_dir.join(&filename);

        Ok(AddResult::Created {
            changeset,
            file_path,
            uncovered_dependents,
        })
    }

    fn select_packages(
        &self,
        available: &[PackageInfo],
        input: &AddInput,
        display_labels: Option<&[String]>,
    ) -> Result<Option<Vec<PackageInfo>>> {
        let explicit_packages = collect_explicit_packages(input);

        if !explicit_packages.is_empty() {
            let packages = resolve_explicit_packages(available, &explicit_packages)?;
            return Ok(Some(packages));
        }

        if available.len() == 1 {
            return Ok(Some(vec![available[0].clone()]));
        }

        match self
            .interaction_provider
            .select_packages(available, display_labels)?
        {
            PackageSelection::Selected(packages) => Ok(Some(packages)),
            PackageSelection::Cancelled => Ok(None),
        }
    }

    fn collect_releases(
        &self,
        packages: &[PackageInfo],
        input: &AddInput,
    ) -> Result<Option<Vec<PackageRelease>>> {
        let mut releases = Vec::with_capacity(packages.len());

        for package in packages {
            let bump_type = if let Some(bump) = input.package_bumps.get(&package.name) {
                *bump
            } else if let Some(bump) = input.bump {
                bump
            } else {
                match self.interaction_provider.select_bump_type(&package.name)? {
                    BumpSelection::Selected(bump) => bump,
                    BumpSelection::Cancelled => return Ok(None),
                }
            };

            releases.push(PackageRelease {
                name: package.name.clone(),
                bump_type,
            });
        }

        Ok(Some(releases))
    }

    fn select_category(&self, input: &AddInput) -> Result<Option<ChangeCategory>> {
        let has_explicit_input = input.description.is_some()
            || input.bump.is_some()
            || !input.packages.is_empty()
            || !input.package_bumps.is_empty();

        if input.category != ChangeCategory::default() || has_explicit_input {
            return Ok(Some(input.category));
        }

        match self.interaction_provider.select_category()? {
            CategorySelection::Selected(category) => Ok(Some(category)),
            CategorySelection::Cancelled => Ok(None),
        }
    }

    fn get_description(&self, input: &AddInput) -> Result<Option<String>> {
        if let Some(description) = &input.description {
            return Ok(Some(description.clone()));
        }

        match self.interaction_provider.get_description()? {
            DescriptionInput::Provided(description) => Ok(Some(description)),
            DescriptionInput::Cancelled => Ok(None),
        }
    }
}

fn collect_explicit_packages(input: &AddInput) -> Vec<String> {
    let mut packages: IndexSet<String> = input.packages.iter().cloned().collect();

    for name in input.package_bumps.keys() {
        packages.insert(name.clone());
    }

    packages.into_iter().collect()
}

fn resolve_explicit_packages(
    packages: &[PackageInfo],
    package_names: &[String],
) -> Result<Vec<PackageInfo>> {
    let unique_names: IndexSet<&String> = package_names.iter().collect();
    let mut selected = Vec::with_capacity(unique_names.len());

    for name in unique_names {
        let package = packages.iter().find(|p| p.name == *name).ok_or_else(|| {
            let available = packages
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            OperationError::UnknownPackage {
                name: name.clone(),
                available,
            }
        })?;
        selected.push(package.clone());
    }

    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_explicit_packages_from_packages_list() {
        let input = AddInput {
            packages: vec!["a".to_string(), "b".to_string()],
            ..Default::default()
        };

        let packages = collect_explicit_packages(&input);

        assert_eq!(packages.len(), 2);
        assert!(packages.contains(&"a".to_string()));
        assert!(packages.contains(&"b".to_string()));
    }

    #[test]
    fn collect_explicit_packages_from_package_bumps() {
        let mut package_bumps = HashMap::new();
        package_bumps.insert("a".to_string(), BumpType::Major);
        package_bumps.insert("b".to_string(), BumpType::Minor);

        let input = AddInput {
            package_bumps,
            ..Default::default()
        };

        let packages = collect_explicit_packages(&input);

        assert_eq!(packages.len(), 2);
        assert!(packages.contains(&"a".to_string()));
        assert!(packages.contains(&"b".to_string()));
    }

    #[test]
    fn collect_explicit_packages_merges_and_deduplicates() {
        let mut package_bumps = HashMap::new();
        package_bumps.insert("a".to_string(), BumpType::Major);
        package_bumps.insert("b".to_string(), BumpType::Minor);

        let input = AddInput {
            packages: vec!["a".to_string(), "c".to_string()],
            package_bumps,
            ..Default::default()
        };

        let packages = collect_explicit_packages(&input);

        assert_eq!(packages.len(), 3);
        assert!(packages.contains(&"a".to_string()));
        assert!(packages.contains(&"b".to_string()));
        assert!(packages.contains(&"c".to_string()));
    }

    #[test]
    fn collect_explicit_packages_empty() {
        let input = AddInput::default();

        let packages = collect_explicit_packages(&input);

        assert!(packages.is_empty());
    }
}

#[cfg(test)]
mod operation_tests {
    use super::*;
    use crate::mocks::{
        MockChangesetWriter, MockInteractionProvider, MockProjectProvider, make_package,
    };

    #[test]
    fn creates_changeset_for_single_package_project() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let writer = MockChangesetWriter::new().with_filename("test-changeset.md");
        let interaction = MockInteractionProvider::all_cancelled();

        let operation = AddOperation::new(project_provider, writer, interaction);

        let input = AddInput {
            packages: vec!["my-crate".to_string()],
            bump: Some(BumpType::Patch),
            description: Some("Fix a bug".to_string()),
            ..Default::default()
        };

        let result = operation
            .execute(Path::new("/any"), input)
            .expect("AddOperation failed with valid single-package input");

        match result {
            AddResult::Created {
                changeset,
                file_path,
                ..
            } => {
                assert_eq!(changeset.summary, "Fix a bug");
                assert_eq!(changeset.releases.len(), 1);
                assert_eq!(changeset.releases[0].name, "my-crate");
                assert_eq!(changeset.releases[0].bump_type, BumpType::Patch);
                assert!(file_path.ends_with("test-changeset.md"));
            }
            _ => panic!("Expected AddResult::Created"),
        }
    }

    #[test]
    fn creates_changeset_with_multiple_packages() {
        let project_provider =
            MockProjectProvider::workspace(vec![("crate-a", "1.0.0"), ("crate-b", "2.0.0")]);
        let writer = MockChangesetWriter::new();
        let interaction = MockInteractionProvider::all_cancelled();

        let operation = AddOperation::new(project_provider, writer, interaction);

        let mut package_bumps = HashMap::new();
        package_bumps.insert("crate-a".to_string(), BumpType::Major);
        package_bumps.insert("crate-b".to_string(), BumpType::Minor);

        let input = AddInput {
            package_bumps,
            description: Some("Breaking change".to_string()),
            ..Default::default()
        };

        let result = operation
            .execute(Path::new("/any"), input)
            .expect("AddOperation failed with valid multi-package input");

        match result {
            AddResult::Created { changeset, .. } => {
                assert_eq!(changeset.releases.len(), 2);
                let names: Vec<_> = changeset.releases.iter().map(|r| r.name.as_str()).collect();
                assert!(names.contains(&"crate-a"));
                assert!(names.contains(&"crate-b"));
            }
            _ => panic!("Expected AddResult::Created"),
        }
    }

    #[test]
    fn returns_cancelled_when_package_selection_cancelled() {
        let project_provider = MockProjectProvider::workspace(vec![("a", "1.0.0"), ("b", "1.0.0")]);
        let writer = MockChangesetWriter::new();
        let interaction = MockInteractionProvider::all_cancelled();

        let operation = AddOperation::new(project_provider, writer, interaction);

        let result = operation
            .execute(Path::new("/any"), AddInput::default())
            .expect("AddOperation should not fail when interaction is cancelled");

        assert!(matches!(result, AddResult::Cancelled));
    }

    #[test]
    fn returns_cancelled_when_bump_selection_cancelled() {
        let packages = vec![make_package("my-crate", "1.0.0")];
        let project_provider = MockProjectProvider::workspace(vec![("my-crate", "1.0.0")]);
        let writer = MockChangesetWriter::new();
        let interaction = MockInteractionProvider {
            package_selection: crate::traits::PackageSelection::Selected(packages),
            bump_selections: std::sync::Mutex::new(vec![]),
            category_selection: crate::traits::CategorySelection::Selected(ChangeCategory::Changed),
            description: crate::traits::DescriptionInput::Provided("test".to_string()),
        };

        let operation = AddOperation::new(project_provider, writer, interaction);

        let result = operation
            .execute(Path::new("/any"), AddInput::default())
            .expect("AddOperation should not fail when bump selection is cancelled");

        assert!(matches!(result, AddResult::Cancelled));
    }

    #[test]
    fn returns_error_for_unknown_package() {
        let project_provider = MockProjectProvider::single_package("real-crate", "1.0.0");
        let writer = MockChangesetWriter::new();
        let interaction = MockInteractionProvider::all_cancelled();

        let operation = AddOperation::new(project_provider, writer, interaction);

        let input = AddInput {
            packages: vec!["unknown-crate".to_string()],
            bump: Some(BumpType::Patch),
            description: Some("Test".to_string()),
            ..Default::default()
        };

        let result = operation.execute(Path::new("/any"), input);

        assert!(result.is_err());
        let err = result.expect_err("AddOperation should fail for unknown package");
        assert!(matches!(err, crate::OperationError::UnknownPackage { .. }));
    }

    #[test]
    fn returns_error_for_empty_description() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let writer = MockChangesetWriter::new();
        let interaction = MockInteractionProvider::all_cancelled();

        let operation = AddOperation::new(project_provider, writer, interaction);

        let input = AddInput {
            packages: vec!["my-crate".to_string()],
            bump: Some(BumpType::Patch),
            description: Some("   ".to_string()),
            ..Default::default()
        };

        let result = operation.execute(Path::new("/any"), input);

        assert!(result.is_err());
        let err = result.expect_err("AddOperation should fail for empty description");
        assert!(matches!(err, crate::OperationError::EmptyDescription));
    }

    #[test]
    fn uses_interactive_selection_for_workspace_without_explicit_packages() {
        let packages = vec![
            make_package("crate-a", "1.0.0"),
            make_package("crate-b", "2.0.0"),
        ];
        let project_provider =
            MockProjectProvider::workspace(vec![("crate-a", "1.0.0"), ("crate-b", "2.0.0")]);
        let writer = MockChangesetWriter::new();
        let interaction =
            MockInteractionProvider::with_selections(packages, BumpType::Minor, "Interactive desc")
                .with_bump_sequence(vec![BumpType::Minor, BumpType::Minor]);

        let operation = AddOperation::new(project_provider, writer, interaction);

        let result = operation
            .execute(Path::new("/any"), AddInput::default())
            .expect("AddOperation failed with interactive workspace selection");

        match result {
            AddResult::Created { changeset, .. } => {
                assert_eq!(changeset.summary, "Interactive desc");
                assert_eq!(changeset.releases.len(), 2);
            }
            _ => panic!("Expected AddResult::Created"),
        }
    }

    #[test]
    fn auto_selects_single_package_without_interaction() {
        let project_provider = MockProjectProvider::single_package("solo-crate", "1.0.0");
        let writer = MockChangesetWriter::new();
        let interaction = MockInteractionProvider::with_selections(
            vec![make_package("solo-crate", "1.0.0")],
            BumpType::Patch,
            "Auto-selected",
        );

        let operation = AddOperation::new(project_provider, writer, interaction);

        let input = AddInput {
            bump: Some(BumpType::Patch),
            description: Some("Non-interactive description".to_string()),
            ..Default::default()
        };

        let result = operation
            .execute(Path::new("/any"), input)
            .expect("AddOperation failed for single-package auto-selection");

        match result {
            AddResult::Created { changeset, .. } => {
                assert_eq!(changeset.releases.len(), 1);
                assert_eq!(changeset.releases[0].name, "solo-crate");
                assert_eq!(changeset.summary, "Non-interactive description");
            }
            _ => panic!("Expected AddResult::Created"),
        }
    }

    #[test]
    fn respects_explicit_category() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let writer = MockChangesetWriter::new();
        let interaction = MockInteractionProvider::all_cancelled();

        let operation = AddOperation::new(project_provider, writer, interaction);

        let input = AddInput {
            packages: vec!["my-crate".to_string()],
            bump: Some(BumpType::Minor),
            category: ChangeCategory::Fixed,
            description: Some("Bug fix".to_string()),
            ..Default::default()
        };

        let result = operation
            .execute(Path::new("/any"), input)
            .expect("AddOperation failed with explicit category");

        match result {
            AddResult::Created { changeset, .. } => {
                assert_eq!(changeset.category, ChangeCategory::Fixed);
            }
            _ => panic!("Expected AddResult::Created"),
        }
    }

    #[test]
    fn creates_changeset_file_in_project() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let writer = MockChangesetWriter::new().with_filename("my-changeset.md");
        let interaction = MockInteractionProvider::all_cancelled();

        let operation = AddOperation::new(project_provider, writer, interaction);

        let input = AddInput {
            packages: vec!["my-crate".to_string()],
            bump: Some(BumpType::Patch),
            description: Some("Test description".to_string()),
            ..Default::default()
        };

        let result = operation
            .execute(Path::new("/any"), input)
            .expect("AddOperation failed to create changeset file");

        match result {
            AddResult::Created { file_path, .. } => {
                assert!(file_path.to_string_lossy().contains("my-changeset.md"));
            }
            _ => panic!("Expected AddResult::Created"),
        }
    }

    #[test]
    fn none_bump_without_description_returns_empty_description_error() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let writer = MockChangesetWriter::new();
        let interaction = MockInteractionProvider::all_cancelled();

        let operation = AddOperation::new(project_provider, writer, interaction);

        let input = AddInput {
            packages: vec!["my-crate".to_string()],
            bump: Some(BumpType::None),
            description: Some("   ".to_string()),
            ..Default::default()
        };

        let result = operation.execute(Path::new("/any"), input);

        assert!(result.is_err());
        let err = result.expect_err("AddOperation should fail for none bump without description");
        assert!(matches!(err, crate::OperationError::EmptyDescription));
    }

    #[test]
    fn none_bump_with_explicit_description_creates_changeset() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let writer = MockChangesetWriter::new();
        let interaction = MockInteractionProvider::all_cancelled();

        let operation = AddOperation::new(project_provider, writer, interaction);

        let input = AddInput {
            packages: vec!["my-crate".to_string()],
            bump: Some(BumpType::None),
            description: Some("Internal refactoring".to_string()),
            ..Default::default()
        };

        let result = operation
            .execute(Path::new("/any"), input)
            .expect("AddOperation should succeed for none bump with description");

        match result {
            AddResult::Created { changeset, .. } => {
                assert_eq!(changeset.summary, "Internal refactoring");
                assert_eq!(changeset.releases[0].bump_type, BumpType::None);
            }
            _ => panic!("Expected AddResult::Created"),
        }
    }

    #[test]
    fn none_bump_interactive_description_creates_changeset() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let writer = MockChangesetWriter::new();
        let interaction = MockInteractionProvider {
            package_selection: crate::traits::PackageSelection::Cancelled,
            bump_selections: std::sync::Mutex::new(vec![]),
            category_selection: crate::traits::CategorySelection::Selected(ChangeCategory::Changed),
            description: crate::traits::DescriptionInput::Provided(
                "Interactive reason".to_string(),
            ),
        };

        let operation = AddOperation::new(project_provider, writer, interaction);

        let input = AddInput {
            bump: Some(BumpType::None),
            ..Default::default()
        };

        let result = operation
            .execute(Path::new("/any"), input)
            .expect("AddOperation should succeed with interactive description for none bump");

        match result {
            AddResult::Created { changeset, .. } => {
                assert_eq!(changeset.summary, "Interactive reason");
                assert_eq!(changeset.releases[0].bump_type, BumpType::None);
                assert_eq!(changeset.category, ChangeCategory::Changed);
            }
            _ => panic!("Expected AddResult::Created"),
        }
    }

    #[test]
    fn mixed_none_and_patch_bumps_creates_changeset() {
        let project_provider =
            MockProjectProvider::workspace(vec![("crate-a", "1.0.0"), ("crate-b", "2.0.0")]);
        let writer = MockChangesetWriter::new();
        let interaction = MockInteractionProvider::all_cancelled();

        let operation = AddOperation::new(project_provider, writer, interaction);

        let mut package_bumps = HashMap::new();
        package_bumps.insert("crate-a".to_string(), BumpType::None);
        package_bumps.insert("crate-b".to_string(), BumpType::Patch);

        let input = AddInput {
            package_bumps,
            description: Some("Update crate-b with internal crate-a changes".to_string()),
            ..Default::default()
        };

        let result = operation
            .execute(Path::new("/any"), input)
            .expect("AddOperation should succeed for mixed none/patch bumps with description");

        match result {
            AddResult::Created { changeset, .. } => {
                assert_eq!(
                    changeset.summary,
                    "Update crate-b with internal crate-a changes"
                );
                assert_eq!(changeset.releases.len(), 2);
                let none_release = changeset
                    .releases
                    .iter()
                    .find(|r| r.name == "crate-a")
                    .expect("crate-a should be in releases");
                assert_eq!(none_release.bump_type, BumpType::None);
                let patch_release = changeset
                    .releases
                    .iter()
                    .find(|r| r.name == "crate-b")
                    .expect("crate-b should be in releases");
                assert_eq!(patch_release.bump_type, BumpType::Patch);
            }
            _ => panic!("Expected AddResult::Created"),
        }
    }

    #[test]
    fn exclude_dependents_skips_dependency_computation() {
        let project_provider =
            MockProjectProvider::workspace(vec![("core", "1.0.0"), ("app", "1.0.0")])
                .with_dependency_edges(vec![("app", "core")]);
        let writer = MockChangesetWriter::new();
        let interaction = MockInteractionProvider::all_cancelled();

        let operation = AddOperation::new(project_provider, writer, interaction);

        let input = AddInput {
            packages: vec!["core".to_string()],
            bump: Some(BumpType::Patch),
            description: Some("Fix in core".to_string()),
            exclude_dependents: true,
            ..Default::default()
        };

        let result = operation
            .execute(Path::new("/any"), input)
            .expect("AddOperation should succeed with exclude_dependents");

        match result {
            AddResult::Created {
                uncovered_dependents,
                ..
            } => {
                assert!(uncovered_dependents.is_empty());
            }
            _ => panic!("Expected AddResult::Created"),
        }
    }

    #[test]
    fn non_interactive_with_dependents_returns_uncovered() {
        let project_provider = MockProjectProvider::workspace(vec![
            ("core", "1.0.0"),
            ("lib", "1.0.0"),
            ("app", "1.0.0"),
        ])
        .with_dependency_edges(vec![("lib", "core"), ("app", "lib")]);
        let writer = MockChangesetWriter::new();
        let interaction = MockInteractionProvider::all_cancelled();

        let operation = AddOperation::new(project_provider, writer, interaction);

        let input = AddInput {
            packages: vec!["core".to_string()],
            bump: Some(BumpType::Patch),
            description: Some("Fix in core".to_string()),
            ..Default::default()
        };

        let result = operation
            .execute(Path::new("/any"), input)
            .expect("AddOperation should succeed and report uncovered dependents");

        match result {
            AddResult::Created {
                uncovered_dependents,
                ..
            } => {
                assert!(uncovered_dependents.contains(&"lib".to_string()));
                assert!(uncovered_dependents.contains(&"app".to_string()));
                assert_eq!(uncovered_dependents.len(), 2);
            }
            _ => panic!("Expected AddResult::Created"),
        }
    }

    #[test]
    fn single_package_workspace_skips_dependency_computation() {
        let project_provider = MockProjectProvider::single_package("solo", "1.0.0");
        let writer = MockChangesetWriter::new();
        let interaction = MockInteractionProvider::all_cancelled();

        let operation = AddOperation::new(project_provider, writer, interaction);

        let input = AddInput {
            packages: vec!["solo".to_string()],
            bump: Some(BumpType::Minor),
            description: Some("New feature".to_string()),
            ..Default::default()
        };

        let result = operation
            .execute(Path::new("/any"), input)
            .expect("AddOperation should succeed for single-package project");

        match result {
            AddResult::Created {
                uncovered_dependents,
                ..
            } => {
                assert!(uncovered_dependents.is_empty());
            }
            _ => panic!("Expected AddResult::Created"),
        }
    }
}
