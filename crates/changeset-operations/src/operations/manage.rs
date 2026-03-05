use std::path::Path;

use changeset_core::PrereleaseSpec;
use changeset_project::{CargoProject, GraduationState, PrereleaseState};
use changeset_version::{is_prerelease, is_zero_version};

use crate::Result;
use crate::error::OperationError;
use crate::traits::{
    GraduationAction, GraduationInteractionProvider, MenuSelection, PrereleaseAction,
    PrereleaseInteractionProvider, ProjectProvider, ReleaseStateIO,
};

pub struct PrereleaseManageOperation<P, S, I> {
    project_provider: P,
    release_state_io: S,
    interaction_provider: I,
}

// GraduationInteractionProvider is required because the prerelease menu
// includes a "Graduate" action that moves packages to the graduation queue.
impl<P, S, I> PrereleaseManageOperation<P, S, I>
where
    P: ProjectProvider,
    S: ReleaseStateIO,
    I: PrereleaseInteractionProvider + GraduationInteractionProvider,
{
    pub fn new(project_provider: P, release_state_io: S, interaction_provider: I) -> Self {
        Self {
            project_provider,
            release_state_io,
            interaction_provider,
        }
    }

    /// # Errors
    ///
    /// Returns an error if project discovery, state loading, or interaction fails.
    pub fn execute(&self, start_path: &Path) -> Result<Vec<PrereleaseEvent>> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;
        let changeset_dir = project.root().join(root_config.changeset_dir());

        let mut prerelease_state = self
            .release_state_io
            .load_prerelease_state(&changeset_dir)?
            .unwrap_or_default();

        let mut graduation_state = self
            .release_state_io
            .load_graduation_state(&changeset_dir)?
            .unwrap_or_default();

        let mut events = Vec::new();

        loop {
            events.push(PrereleaseEvent::DisplayState(
                prerelease_state
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            ));

            let action = self.interaction_provider.select_prerelease_action()?;

            match action {
                MenuSelection::Selected(PrereleaseAction::Add) => {
                    self.handle_add(&project, &changeset_dir, &mut prerelease_state, &mut events)?;
                }
                MenuSelection::Selected(PrereleaseAction::Remove) => {
                    self.handle_remove(&changeset_dir, &mut prerelease_state, &mut events)?;
                }
                MenuSelection::Selected(PrereleaseAction::Graduate) => {
                    self.handle_graduate(
                        &project,
                        &changeset_dir,
                        &mut prerelease_state,
                        &mut graduation_state,
                        &mut events,
                    )?;
                }
                MenuSelection::Selected(PrereleaseAction::Done) | MenuSelection::Cancelled => {
                    break;
                }
            }
        }

        Ok(events)
    }

    fn handle_add(
        &self,
        project: &CargoProject,
        changeset_dir: &Path,
        prerelease_state: &mut PrereleaseState,
        events: &mut Vec<PrereleaseEvent>,
    ) -> Result<()> {
        let available: Vec<_> = project
            .packages()
            .iter()
            .filter(|p| !prerelease_state.contains(&p.name))
            .collect();

        if available.is_empty() {
            events.push(PrereleaseEvent::AllPackagesInPrerelease);
            return Ok(());
        }

        let selection = self
            .interaction_provider
            .select_package_for_prerelease(&available)?;

        let MenuSelection::Selected(index) = selection else {
            return Ok(());
        };

        let crate_name = &available[index].name;
        let tag = self.interaction_provider.get_prerelease_tag()?;

        validate_prerelease_tag(&tag)?;

        prerelease_state.insert(crate_name.clone(), tag.clone());
        self.release_state_io
            .save_prerelease_state(changeset_dir, prerelease_state)?;
        events.push(PrereleaseEvent::Added {
            crate_name: crate_name.clone(),
            tag,
        });

        Ok(())
    }

    fn handle_remove(
        &self,
        changeset_dir: &Path,
        prerelease_state: &mut PrereleaseState,
        events: &mut Vec<PrereleaseEvent>,
    ) -> Result<()> {
        if prerelease_state.is_empty() {
            events.push(PrereleaseEvent::NoPrereleasePackages);
            return Ok(());
        }

        let mut items: Vec<_> = prerelease_state
            .iter()
            .map(|(name, tag)| (name.to_string(), tag.to_string()))
            .collect();
        items.sort_by(|a, b| a.0.cmp(&b.0));

        let refs: Vec<(&str, &str)> = items
            .iter()
            .map(|(n, t)| (n.as_str(), t.as_str()))
            .collect();
        let selection = self
            .interaction_provider
            .select_package_to_remove_prerelease(&refs)?;

        let MenuSelection::Selected(index) = selection else {
            return Ok(());
        };

        let crate_name = items[index].0.clone();
        let _ = prerelease_state.remove(&crate_name);
        self.release_state_io
            .save_prerelease_state(changeset_dir, prerelease_state)?;
        events.push(PrereleaseEvent::Removed { crate_name });

        Ok(())
    }

    fn handle_graduate(
        &self,
        project: &CargoProject,
        changeset_dir: &Path,
        prerelease_state: &mut PrereleaseState,
        graduation_state: &mut GraduationState,
        events: &mut Vec<PrereleaseEvent>,
    ) -> Result<()> {
        let eligible: Vec<_> = project
            .packages()
            .iter()
            .filter(|p| is_zero_version(&p.version) && !is_prerelease(&p.version))
            .collect();

        if eligible.is_empty() {
            events.push(PrereleaseEvent::NoEligibleForGraduation);
            return Ok(());
        }

        let selection = self
            .interaction_provider
            .select_package_for_graduation(&eligible)?;

        let MenuSelection::Selected(index) = selection else {
            return Ok(());
        };

        let crate_name = &eligible[index].name;

        if prerelease_state.remove(crate_name).is_some() {
            self.release_state_io
                .save_prerelease_state(changeset_dir, prerelease_state)?;
        }

        graduation_state.add(crate_name.clone());
        self.release_state_io
            .save_graduation_state(changeset_dir, graduation_state)?;
        events.push(PrereleaseEvent::MovedToGraduation {
            crate_name: crate_name.clone(),
        });

        Ok(())
    }
}

pub struct GraduationManageOperation<P, S, I> {
    project_provider: P,
    release_state_io: S,
    interaction_provider: I,
}

impl<P, S, I> GraduationManageOperation<P, S, I>
where
    P: ProjectProvider,
    S: ReleaseStateIO,
    I: GraduationInteractionProvider,
{
    pub fn new(project_provider: P, release_state_io: S, interaction_provider: I) -> Self {
        Self {
            project_provider,
            release_state_io,
            interaction_provider,
        }
    }

    /// # Errors
    ///
    /// Returns an error if project discovery, state loading, or interaction fails.
    pub fn execute(&self, start_path: &Path) -> Result<Vec<GraduationEvent>> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;
        let changeset_dir = project.root().join(root_config.changeset_dir());

        let mut state = self
            .release_state_io
            .load_graduation_state(&changeset_dir)?
            .unwrap_or_default();

        let mut events = Vec::new();

        loop {
            events.push(GraduationEvent::DisplayState(
                state.iter().map(str::to_string).collect(),
            ));

            let action = self.interaction_provider.select_graduation_action()?;

            match action {
                MenuSelection::Selected(GraduationAction::Add) => {
                    let eligible: Vec<_> = project
                        .packages()
                        .iter()
                        .filter(|p| {
                            is_zero_version(&p.version)
                                && !is_prerelease(&p.version)
                                && !state.contains(&p.name)
                        })
                        .collect();

                    if eligible.is_empty() {
                        events.push(GraduationEvent::NoEligibleForGraduation);
                        continue;
                    }

                    let selection = self
                        .interaction_provider
                        .select_package_for_graduation(&eligible)?;

                    let MenuSelection::Selected(index) = selection else {
                        continue;
                    };

                    let crate_name = &eligible[index].name;
                    state.add(crate_name.clone());
                    self.release_state_io
                        .save_graduation_state(&changeset_dir, &state)?;
                    events.push(GraduationEvent::Added {
                        crate_name: crate_name.clone(),
                    });
                }
                MenuSelection::Selected(GraduationAction::Remove) => {
                    if state.is_empty() {
                        events.push(GraduationEvent::NoGraduationPackages);
                        continue;
                    }

                    let mut items: Vec<String> = state.iter().map(str::to_string).collect();
                    items.sort();

                    let selection = self
                        .interaction_provider
                        .select_package_to_remove_graduation(&items)?;

                    let MenuSelection::Selected(index) = selection else {
                        continue;
                    };

                    let crate_name = &items[index];
                    let _ = state.remove(crate_name);
                    self.release_state_io
                        .save_graduation_state(&changeset_dir, &state)?;
                    events.push(GraduationEvent::Removed {
                        crate_name: crate_name.clone(),
                    });
                }
                MenuSelection::Selected(GraduationAction::Done) | MenuSelection::Cancelled => {
                    break;
                }
            }
        }

        Ok(events)
    }
}

pub struct PrereleaseDirectOperation<P, S> {
    project_provider: P,
    release_state_io: S,
}

impl<P, S> PrereleaseDirectOperation<P, S>
where
    P: ProjectProvider,
    S: ReleaseStateIO,
{
    pub fn new(project_provider: P, release_state_io: S) -> Self {
        Self {
            project_provider,
            release_state_io,
        }
    }

    /// # Errors
    ///
    /// Returns an error if project discovery, state loading, validation, or persistence fails.
    pub fn execute(
        &self,
        start_path: &Path,
        input: &PrereleaseDirectInput,
    ) -> Result<Vec<PrereleaseEvent>> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;
        let changeset_dir = project.root().join(root_config.changeset_dir());

        let mut prerelease_state = self
            .release_state_io
            .load_prerelease_state(&changeset_dir)?
            .unwrap_or_default();

        let mut graduation_state = self
            .release_state_io
            .load_graduation_state(&changeset_dir)?
            .unwrap_or_default();

        let mut events = Vec::new();
        let mut modified_prerelease = false;
        let mut modified_graduation = false;

        for entry in &input.add {
            let (crate_name, tag) = parse_prerelease_entry(entry)?;
            validate_package_exists(&project, &crate_name)?;
            validate_prerelease_tag(&tag)?;

            prerelease_state.insert(crate_name.clone(), tag.clone());
            modified_prerelease = true;
            events.push(PrereleaseEvent::Added { crate_name, tag });
        }

        for crate_name in &input.remove {
            if prerelease_state.remove(crate_name).is_some() {
                modified_prerelease = true;
                events.push(PrereleaseEvent::Removed {
                    crate_name: crate_name.clone(),
                });
            }
        }

        for crate_name in &input.graduate {
            validate_package_exists(&project, crate_name)?;
            validate_can_graduate(&project, crate_name)?;

            if prerelease_state.remove(crate_name).is_some() {
                modified_prerelease = true;
            }

            graduation_state.add(crate_name.clone());
            modified_graduation = true;
            events.push(PrereleaseEvent::MovedToGraduation {
                crate_name: crate_name.clone(),
            });
        }

        if modified_prerelease {
            self.release_state_io
                .save_prerelease_state(&changeset_dir, &prerelease_state)?;
        }
        if modified_graduation {
            self.release_state_io
                .save_graduation_state(&changeset_dir, &graduation_state)?;
        }

        if input.list {
            events.push(PrereleaseEvent::DisplayState(
                prerelease_state
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            ));
        }

        Ok(events)
    }
}

pub struct PrereleaseDirectInput {
    add: Vec<String>,
    remove: Vec<String>,
    graduate: Vec<String>,
    list: bool,
}

impl PrereleaseDirectInput {
    #[must_use]
    pub fn new(add: Vec<String>, remove: Vec<String>, graduate: Vec<String>, list: bool) -> Self {
        Self {
            add,
            remove,
            graduate,
            list,
        }
    }
}

pub struct GraduationDirectOperation<P, S> {
    project_provider: P,
    release_state_io: S,
}

impl<P, S> GraduationDirectOperation<P, S>
where
    P: ProjectProvider,
    S: ReleaseStateIO,
{
    pub fn new(project_provider: P, release_state_io: S) -> Self {
        Self {
            project_provider,
            release_state_io,
        }
    }

    /// # Errors
    ///
    /// Returns an error if project discovery, state loading, validation, or persistence fails.
    pub fn execute(
        &self,
        start_path: &Path,
        input: &GraduationDirectInput,
    ) -> Result<Vec<GraduationEvent>> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, _) = self.project_provider.load_configs(&project)?;
        let changeset_dir = project.root().join(root_config.changeset_dir());

        let mut state = self
            .release_state_io
            .load_graduation_state(&changeset_dir)?
            .unwrap_or_default();

        let mut events = Vec::new();
        let mut modified = false;

        for crate_name in &input.add {
            validate_package_exists(&project, crate_name)?;
            validate_can_graduate(&project, crate_name)?;

            state.add(crate_name.clone());
            modified = true;
            events.push(GraduationEvent::Added {
                crate_name: crate_name.clone(),
            });
        }

        for crate_name in &input.remove {
            if state.remove(crate_name) {
                modified = true;
                events.push(GraduationEvent::Removed {
                    crate_name: crate_name.clone(),
                });
            }
        }

        if modified {
            self.release_state_io
                .save_graduation_state(&changeset_dir, &state)?;
        }

        if input.list {
            events.push(GraduationEvent::DisplayState(
                state.iter().map(str::to_string).collect(),
            ));
        }

        Ok(events)
    }
}

pub struct GraduationDirectInput {
    add: Vec<String>,
    remove: Vec<String>,
    list: bool,
}

impl GraduationDirectInput {
    #[must_use]
    pub fn new(add: Vec<String>, remove: Vec<String>, list: bool) -> Self {
        Self { add, remove, list }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrereleaseEvent {
    DisplayState(Vec<(String, String)>),
    Added { crate_name: String, tag: String },
    Removed { crate_name: String },
    MovedToGraduation { crate_name: String },
    AllPackagesInPrerelease,
    NoPrereleasePackages,
    NoEligibleForGraduation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraduationEvent {
    DisplayState(Vec<String>),
    Added { crate_name: String },
    Removed { crate_name: String },
    NoEligibleForGraduation,
    NoGraduationPackages,
}

fn parse_prerelease_entry(input: &str) -> Result<(String, String)> {
    let Some((crate_name, tag)) = input.split_once(':') else {
        return Err(OperationError::InvalidPrereleaseFormat {
            input: input.to_string(),
        });
    };

    if crate_name.is_empty() || tag.is_empty() {
        return Err(OperationError::InvalidPrereleaseFormat {
            input: input.to_string(),
        });
    }

    Ok((crate_name.to_string(), tag.to_string()))
}

fn validate_prerelease_tag(tag: &str) -> Result<()> {
    tag.parse::<PrereleaseSpec>()
        .map_err(|source| OperationError::InvalidPrereleaseTag {
            tag: tag.to_string(),
            source,
        })?;
    Ok(())
}

fn validate_package_exists(project: &CargoProject, name: &str) -> Result<()> {
    if !project.packages().iter().any(|p| p.name == name) {
        return Err(OperationError::PackageNotFound {
            name: name.to_string(),
        });
    }
    Ok(())
}

fn validate_can_graduate(project: &CargoProject, name: &str) -> Result<()> {
    let package = project
        .packages()
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| OperationError::PackageNotFound {
            name: name.to_string(),
        })?;

    if is_prerelease(&package.version) {
        return Err(OperationError::CannotGraduatePrerelease {
            package: name.to_string(),
            version: package.version.to_string(),
        });
    }

    if !is_zero_version(&package.version) {
        return Err(OperationError::CannotGraduateStable {
            package: name.to_string(),
            version: package.version.to_string(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use changeset_core::PackageInfo;
    use changeset_project::{CargoProject, ProjectKind};
    use std::path::PathBuf;

    fn make_project(packages: Vec<(&str, &str)>) -> CargoProject {
        CargoProject::new(
            PathBuf::from("/mock/project"),
            ProjectKind::VirtualWorkspace,
            packages
                .into_iter()
                .map(|(name, version)| PackageInfo {
                    name: name.to_string(),
                    version: version.parse().expect("valid version"),
                    path: PathBuf::from(format!("/mock/project/crates/{name}")),
                })
                .collect(),
        )
    }

    mod parse_prerelease_entry_tests {
        use super::*;

        #[test]
        fn parses_valid_format() {
            let result = parse_prerelease_entry("my-crate:alpha");

            assert!(result.is_ok());
            let (name, tag) = result.expect("should parse");
            assert_eq!(name, "my-crate");
            assert_eq!(tag, "alpha");
        }

        #[test]
        fn parses_custom_tag() {
            let result = parse_prerelease_entry("crate-name:nightly");

            assert!(result.is_ok());
            let (name, tag) = result.expect("should parse");
            assert_eq!(name, "crate-name");
            assert_eq!(tag, "nightly");
        }

        #[test]
        fn rejects_missing_colon() {
            let result = parse_prerelease_entry("no-colon-here");

            assert!(result.is_err());
            assert!(matches!(
                result.expect_err("should fail"),
                OperationError::InvalidPrereleaseFormat { .. }
            ));
        }

        #[test]
        fn rejects_empty_crate_name() {
            let result = parse_prerelease_entry(":alpha");

            assert!(result.is_err());
            assert!(matches!(
                result.expect_err("should fail"),
                OperationError::InvalidPrereleaseFormat { .. }
            ));
        }

        #[test]
        fn rejects_empty_tag() {
            let result = parse_prerelease_entry("crate-name:");

            assert!(result.is_err());
            assert!(matches!(
                result.expect_err("should fail"),
                OperationError::InvalidPrereleaseFormat { .. }
            ));
        }

        #[test]
        fn handles_multiple_colons() {
            let result = parse_prerelease_entry("crate:tag:extra");

            assert!(result.is_ok());
            let (name, tag) = result.expect("should parse");
            assert_eq!(name, "crate");
            assert_eq!(tag, "tag:extra");
        }
    }

    mod validate_prerelease_tag_tests {
        use super::*;

        #[test]
        fn accepts_alpha() {
            assert!(validate_prerelease_tag("alpha").is_ok());
        }

        #[test]
        fn accepts_beta() {
            assert!(validate_prerelease_tag("beta").is_ok());
        }

        #[test]
        fn accepts_rc() {
            assert!(validate_prerelease_tag("rc").is_ok());
        }

        #[test]
        fn accepts_custom_alphanumeric() {
            assert!(validate_prerelease_tag("nightly").is_ok());
            assert!(validate_prerelease_tag("dev123").is_ok());
        }

        #[test]
        fn accepts_hyphenated_tags() {
            assert!(validate_prerelease_tag("pre-release").is_ok());
        }

        #[test]
        fn rejects_empty() {
            let result = validate_prerelease_tag("");

            assert!(result.is_err());
        }

        #[test]
        fn rejects_invalid_characters() {
            let result = validate_prerelease_tag("alpha.1");

            assert!(result.is_err());
        }

        #[test]
        fn rejects_spaces() {
            let result = validate_prerelease_tag("alpha 1");

            assert!(result.is_err());
            assert!(matches!(
                result.expect_err("should fail"),
                OperationError::InvalidPrereleaseTag { .. }
            ));
        }

        #[test]
        fn rejects_underscores() {
            let result = validate_prerelease_tag("alpha_1");

            assert!(result.is_err());
        }
    }

    mod validate_package_exists_tests {
        use super::*;

        #[test]
        fn succeeds_for_existing_package() {
            let project = make_project(vec![("crate-a", "1.0.0"), ("crate-b", "2.0.0")]);

            let result = validate_package_exists(&project, "crate-a");

            assert!(result.is_ok());
        }

        #[test]
        fn fails_for_unknown_package() {
            let project = make_project(vec![("crate-a", "1.0.0")]);

            let result = validate_package_exists(&project, "nonexistent");

            assert!(result.is_err());
            let err = result.expect_err("should fail");
            assert!(matches!(err, OperationError::PackageNotFound { .. }));
            assert!(err.to_string().contains("nonexistent"));
        }

        #[test]
        fn fails_for_empty_project() {
            let project = make_project(vec![]);

            let result = validate_package_exists(&project, "any-crate");

            assert!(result.is_err());
            assert!(matches!(
                result.expect_err("should fail"),
                OperationError::PackageNotFound { .. }
            ));
        }
    }

    mod validate_can_graduate_tests {
        use super::*;

        #[test]
        fn succeeds_for_zero_stable_version() {
            let project = make_project(vec![("crate-a", "0.5.0")]);

            let result = validate_can_graduate(&project, "crate-a");

            assert!(result.is_ok());
        }

        #[test]
        fn fails_for_prerelease_version() {
            let project = make_project(vec![("crate-a", "0.5.0-alpha.1")]);

            let result = validate_can_graduate(&project, "crate-a");

            assert!(result.is_err());
            let err = result.expect_err("should fail");
            assert!(matches!(
                err,
                OperationError::CannotGraduatePrerelease { .. }
            ));
            assert!(err.to_string().contains("crate-a"));
            assert!(err.to_string().contains("prerelease"));
        }

        #[test]
        fn fails_for_stable_version_1_0_0() {
            let project = make_project(vec![("crate-a", "1.0.0")]);

            let result = validate_can_graduate(&project, "crate-a");

            assert!(result.is_err());
            let err = result.expect_err("should fail");
            assert!(matches!(err, OperationError::CannotGraduateStable { .. }));
            assert!(err.to_string().contains("stable"));
        }

        #[test]
        fn fails_for_stable_version_above_1() {
            let project = make_project(vec![("crate-a", "2.5.3")]);

            let result = validate_can_graduate(&project, "crate-a");

            assert!(result.is_err());
            assert!(matches!(
                result.expect_err("should fail"),
                OperationError::CannotGraduateStable { .. }
            ));
        }

        #[test]
        fn fails_for_unknown_package() {
            let project = make_project(vec![("crate-a", "0.5.0")]);

            let result = validate_can_graduate(&project, "nonexistent");

            assert!(result.is_err());
            assert!(matches!(
                result.expect_err("should fail"),
                OperationError::PackageNotFound { .. }
            ));
        }

        #[test]
        fn fails_for_zero_prerelease_version() {
            let project = make_project(vec![("crate-a", "0.1.0-beta.1")]);

            let result = validate_can_graduate(&project, "crate-a");

            assert!(result.is_err());
            assert!(matches!(
                result.expect_err("should fail"),
                OperationError::CannotGraduatePrerelease { .. }
            ));
        }
    }

    mod prerelease_operation {
        use super::*;
        use crate::mocks::{
            MockManageInteractionProvider, MockProjectProvider, MockReleaseStateIO,
        };
        use std::sync::Arc;

        #[test]
        fn exits_on_done_action() {
            let project_provider = MockProjectProvider::workspace(vec![("crate-a", "0.1.0")]);
            let release_state_io = Arc::new(MockReleaseStateIO::new());
            let interaction = Arc::new(
                MockManageInteractionProvider::new()
                    .with_prerelease_actions(vec![MenuSelection::Selected(PrereleaseAction::Done)]),
            );

            let operation = PrereleaseManageOperation::new(
                project_provider,
                Arc::clone(&release_state_io),
                Arc::clone(&interaction),
            );

            let events = operation
                .execute(std::path::Path::new("/any"))
                .expect("should succeed");

            assert!(
                events
                    .iter()
                    .any(|e| matches!(e, PrereleaseEvent::DisplayState(_)))
            );
        }

        #[test]
        fn exits_on_cancelled() {
            let project_provider = MockProjectProvider::workspace(vec![("crate-a", "0.1.0")]);
            let release_state_io = Arc::new(MockReleaseStateIO::new());
            let interaction = Arc::new(
                MockManageInteractionProvider::new()
                    .with_prerelease_actions(vec![MenuSelection::Cancelled]),
            );

            let operation = PrereleaseManageOperation::new(
                project_provider,
                Arc::clone(&release_state_io),
                Arc::clone(&interaction),
            );

            let events = operation
                .execute(std::path::Path::new("/any"))
                .expect("should succeed");

            assert_eq!(events.len(), 1);
        }

        #[test]
        fn adds_package_to_prerelease() {
            let project_provider = MockProjectProvider::workspace(vec![("crate-a", "0.1.0")]);
            let release_state_io = Arc::new(MockReleaseStateIO::new());
            let interaction = Arc::new(
                MockManageInteractionProvider::new()
                    .with_prerelease_actions(vec![
                        MenuSelection::Selected(PrereleaseAction::Add),
                        MenuSelection::Selected(PrereleaseAction::Done),
                    ])
                    .with_package_selections(vec![MenuSelection::Selected(0)])
                    .with_prerelease_tags(vec!["alpha".to_string()]),
            );

            let operation = PrereleaseManageOperation::new(
                project_provider,
                Arc::clone(&release_state_io),
                Arc::clone(&interaction),
            );

            let events = operation
                .execute(std::path::Path::new("/any"))
                .expect("should succeed");

            assert!(events.iter().any(|e| matches!(
                e,
                PrereleaseEvent::Added {
                    crate_name,
                    tag,
                } if crate_name == "crate-a" && tag == "alpha"
            )));
        }

        #[test]
        fn reports_all_packages_in_prerelease() {
            let project_provider = MockProjectProvider::workspace(vec![("crate-a", "0.1.0")]);
            let release_state_io = Arc::new(MockReleaseStateIO::new().with_prerelease_state({
                let mut state = changeset_project::PrereleaseState::default();
                state.insert("crate-a".to_string(), "alpha".to_string());
                state
            }));
            let interaction = Arc::new(
                MockManageInteractionProvider::new().with_prerelease_actions(vec![
                    MenuSelection::Selected(PrereleaseAction::Add),
                    MenuSelection::Selected(PrereleaseAction::Done),
                ]),
            );

            let operation = PrereleaseManageOperation::new(
                project_provider,
                Arc::clone(&release_state_io),
                Arc::clone(&interaction),
            );

            let events = operation
                .execute(std::path::Path::new("/any"))
                .expect("should succeed");

            assert!(
                events
                    .iter()
                    .any(|e| matches!(e, PrereleaseEvent::AllPackagesInPrerelease))
            );
        }
    }

    mod graduation_operation {
        use super::*;
        use crate::mocks::{
            MockManageInteractionProvider, MockProjectProvider, MockReleaseStateIO,
        };
        use std::sync::Arc;

        #[test]
        fn exits_on_done_action() {
            let project_provider = MockProjectProvider::workspace(vec![("crate-a", "0.1.0")]);
            let release_state_io = Arc::new(MockReleaseStateIO::new());
            let interaction = Arc::new(
                MockManageInteractionProvider::new()
                    .with_graduation_actions(vec![MenuSelection::Selected(GraduationAction::Done)]),
            );

            let operation = GraduationManageOperation::new(
                project_provider,
                Arc::clone(&release_state_io),
                Arc::clone(&interaction),
            );

            let events = operation
                .execute(std::path::Path::new("/any"))
                .expect("should succeed");

            assert!(
                events
                    .iter()
                    .any(|e| matches!(e, GraduationEvent::DisplayState(_)))
            );
        }

        #[test]
        fn adds_package_to_graduation() {
            let project_provider = MockProjectProvider::workspace(vec![("crate-a", "0.1.0")]);
            let release_state_io = Arc::new(MockReleaseStateIO::new());
            let interaction = Arc::new(
                MockManageInteractionProvider::new()
                    .with_graduation_actions(vec![
                        MenuSelection::Selected(GraduationAction::Add),
                        MenuSelection::Selected(GraduationAction::Done),
                    ])
                    .with_graduation_selections(vec![MenuSelection::Selected(0)]),
            );

            let operation = GraduationManageOperation::new(
                project_provider,
                Arc::clone(&release_state_io),
                Arc::clone(&interaction),
            );

            let events = operation
                .execute(std::path::Path::new("/any"))
                .expect("should succeed");

            assert!(events.iter().any(|e| matches!(
                e,
                GraduationEvent::Added { crate_name } if crate_name == "crate-a"
            )));
        }

        #[test]
        fn reports_no_graduation_packages_on_remove() {
            let project_provider = MockProjectProvider::workspace(vec![("crate-a", "0.1.0")]);
            let release_state_io = Arc::new(MockReleaseStateIO::new());
            let interaction = Arc::new(
                MockManageInteractionProvider::new().with_graduation_actions(vec![
                    MenuSelection::Selected(GraduationAction::Remove),
                    MenuSelection::Selected(GraduationAction::Done),
                ]),
            );

            let operation = GraduationManageOperation::new(
                project_provider,
                Arc::clone(&release_state_io),
                Arc::clone(&interaction),
            );

            let events = operation
                .execute(std::path::Path::new("/any"))
                .expect("should succeed");

            assert!(
                events
                    .iter()
                    .any(|e| matches!(e, GraduationEvent::NoGraduationPackages))
            );
        }
    }
}
