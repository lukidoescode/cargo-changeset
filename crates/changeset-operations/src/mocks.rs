use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use changeset_changelog::{RepositoryInfo, VersionRelease};
use changeset_core::{BumpType, ChangeCategory, Changeset, PackageInfo};
use changeset_git::{CommitInfo, FileChange, TagInfo};
use changeset_manifest::{InitConfig, MetadataSection};
use changeset_project::{
    CargoProject, GraduationState, PackageChangesetConfig, PrereleaseState, ProjectKind,
    RootChangesetConfig,
};
use semver::Version;

use crate::Result;
use crate::traits::{
    BumpSelection, CategorySelection, ChangelogSettingsInput, ChangelogWriteResult,
    ChangelogWriter, ChangesetReader, ChangesetWriter, DescriptionInput, GitCommitProvider,
    GitDiffProvider, GitSettingsInput, GitStagingProvider, GitStatusProvider, GitTagProvider,
    InheritedVersionChecker, InitInteractionProvider, InteractionProvider,
    ManifestDependencyWriter, ManifestMetadataWriter, ManifestVersionWriter, PackageSelection,
    ProjectContext, ProjectProvider, ReleaseStateIO, VersionSettingsInput, WorkspaceVersionManager,
};

macro_rules! impl_arc_delegation {
    (
        impl $trait_name:ident for Arc<$type:ty> {
            $(
                fn $method:ident(&self $(, $arg:ident: $arg_ty:ty)*) -> $ret:ty;
            )*
        }
    ) => {
        impl $trait_name for Arc<$type> {
            $(
                fn $method(&self $(, $arg: $arg_ty)*) -> $ret {
                    (**self).$method($($arg),*)
                }
            )*
        }
    };
}

pub struct MockProjectProvider {
    project: CargoProject,
    changeset_dir: PathBuf,
    root_config: RootChangesetConfig,
}

impl MockProjectProvider {
    #[must_use]
    pub fn new(project: CargoProject) -> Self {
        let changeset_dir = project.root.join(".changeset");
        Self {
            project,
            changeset_dir,
            root_config: RootChangesetConfig::default(),
        }
    }

    #[must_use]
    pub fn with_changeset_dir(mut self, dir: PathBuf) -> Self {
        if let Some(parent) = dir.parent() {
            self.project.root = parent.to_path_buf();
        }
        self.changeset_dir = dir;
        self
    }

    #[must_use]
    pub fn with_project_root(mut self, root: PathBuf) -> Self {
        self.project.root = root;
        self
    }

    #[must_use]
    pub fn with_root_config(mut self, config: RootChangesetConfig) -> Self {
        self.root_config = config;
        self
    }

    /// # Panics
    ///
    /// Panics if the version string is not valid semver.
    #[must_use]
    pub fn single_package(name: &str, version: &str) -> Self {
        let root = PathBuf::from("/mock/project");
        let project = CargoProject {
            root: root.clone(),
            kind: ProjectKind::SinglePackage,
            packages: vec![PackageInfo {
                name: name.to_string(),
                version: version.parse().expect("valid version"),
                path: root.clone(),
            }],
        };
        Self::new(project)
    }

    /// # Panics
    ///
    /// Panics if any version string is not valid semver.
    #[must_use]
    pub fn workspace(packages: Vec<(&str, &str)>) -> Self {
        let root = PathBuf::from("/mock/workspace");
        let pkg_infos: Vec<PackageInfo> = packages
            .into_iter()
            .map(|(name, version)| PackageInfo {
                name: name.to_string(),
                version: version.parse().expect("valid version"),
                path: root.join("crates").join(name),
            })
            .collect();

        let project = CargoProject {
            root,
            kind: ProjectKind::VirtualWorkspace,
            packages: pkg_infos,
        };
        Self::new(project)
    }
}

impl ProjectProvider for MockProjectProvider {
    fn discover_project(&self, _start_path: &Path) -> Result<CargoProject> {
        Ok(self.project.clone())
    }

    fn load_configs(
        &self,
        _project: &CargoProject,
    ) -> Result<(RootChangesetConfig, HashMap<String, PackageChangesetConfig>)> {
        Ok((self.root_config.clone(), HashMap::new()))
    }

    fn ensure_changeset_dir(
        &self,
        _project: &CargoProject,
        _config: &RootChangesetConfig,
    ) -> Result<PathBuf> {
        Ok(self.changeset_dir.clone())
    }
}

pub struct MockChangesetReader {
    changesets: Arc<Mutex<HashMap<PathBuf, Changeset>>>,
    listed_files: Vec<PathBuf>,
}

impl MockChangesetReader {
    #[must_use]
    pub fn new() -> Self {
        Self {
            changesets: Arc::new(Mutex::new(HashMap::new())),
            listed_files: Vec::new(),
        }
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn with_changeset(mut self, path: PathBuf, changeset: Changeset) -> Self {
        self.listed_files.push(path.clone());
        self.changesets
            .lock()
            .expect("lock poisoned")
            .insert(path, changeset);
        self
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn with_changesets(mut self, changesets: Vec<(PathBuf, Changeset)>) -> Self {
        {
            let mut locked = self.changesets.lock().expect("lock poisoned");
            for (path, changeset) in changesets {
                self.listed_files.push(path.clone());
                locked.insert(path, changeset);
            }
        }
        self
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn with_consumed_changeset(
        mut self,
        path: PathBuf,
        mut changeset: Changeset,
        version: String,
    ) -> Self {
        changeset.consumed_for_prerelease = Some(version);
        self.listed_files.push(path.clone());
        self.changesets
            .lock()
            .expect("lock poisoned")
            .insert(path, changeset);
        self
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn get_consumed_status(&self, path: &Path) -> Option<String> {
        self.changesets
            .lock()
            .expect("lock poisoned")
            .get(path)
            .and_then(|c| c.consumed_for_prerelease.clone())
    }
}

impl Default for MockChangesetReader {
    fn default() -> Self {
        Self::new()
    }
}

impl ChangesetReader for MockChangesetReader {
    fn read_changeset(&self, path: &Path) -> Result<Changeset> {
        self.changesets
            .lock()
            .expect("lock poisoned")
            .get(path)
            .cloned()
            .ok_or_else(|| crate::OperationError::ChangesetFileRead {
                path: path.to_path_buf(),
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "mock file not found"),
            })
    }

    fn list_changesets(&self, _changeset_dir: &Path) -> Result<Vec<PathBuf>> {
        let changesets = self.changesets.lock().expect("lock poisoned");
        Ok(self
            .listed_files
            .iter()
            .filter(|p| {
                changesets
                    .get(*p)
                    .is_some_and(|c| c.consumed_for_prerelease.is_none())
            })
            .cloned()
            .collect())
    }

    fn list_consumed_changesets(&self, _changeset_dir: &Path) -> Result<Vec<PathBuf>> {
        let changesets = self.changesets.lock().expect("lock poisoned");
        Ok(self
            .listed_files
            .iter()
            .filter(|p| {
                changesets
                    .get(*p)
                    .is_some_and(|c| c.consumed_for_prerelease.is_some())
            })
            .cloned()
            .collect())
    }
}

impl ChangesetWriter for MockChangesetReader {
    fn write_changeset(&self, _changeset_dir: &Path, _changeset: &Changeset) -> Result<String> {
        Ok("mock-changeset.md".to_string())
    }

    fn restore_changeset(&self, path: &Path, changeset: &Changeset) -> Result<()> {
        self.changesets
            .lock()
            .expect("lock poisoned")
            .insert(path.to_path_buf(), changeset.clone());
        Ok(())
    }

    fn filename_exists(&self, _changeset_dir: &Path, _filename: &str) -> bool {
        false
    }

    fn mark_consumed_for_prerelease(
        &self,
        _changeset_dir: &Path,
        paths: &[&Path],
        version: &Version,
    ) -> Result<()> {
        let mut changesets = self.changesets.lock().expect("lock poisoned");
        for path in paths {
            if let Some(changeset) = changesets.get_mut(*path) {
                changeset.consumed_for_prerelease = Some(version.to_string());
            }
        }
        Ok(())
    }

    fn clear_consumed_for_prerelease(&self, _changeset_dir: &Path, paths: &[&Path]) -> Result<()> {
        let mut changesets = self.changesets.lock().expect("lock poisoned");
        for path in paths {
            if let Some(changeset) = changesets.get_mut(*path) {
                changeset.consumed_for_prerelease = None;
            }
        }
        Ok(())
    }
}

impl_arc_delegation! {
    impl ChangesetReader for Arc<MockChangesetReader> {
        fn read_changeset(&self, path: &Path) -> Result<Changeset>;
        fn list_changesets(&self, changeset_dir: &Path) -> Result<Vec<PathBuf>>;
        fn list_consumed_changesets(&self, changeset_dir: &Path) -> Result<Vec<PathBuf>>;
    }
}

impl_arc_delegation! {
    impl ChangesetWriter for Arc<MockChangesetReader> {
        fn write_changeset(&self, changeset_dir: &Path, changeset: &Changeset) -> Result<String>;
        fn restore_changeset(&self, path: &Path, changeset: &Changeset) -> Result<()>;
        fn filename_exists(&self, changeset_dir: &Path, filename: &str) -> bool;
        fn mark_consumed_for_prerelease(&self, changeset_dir: &Path, paths: &[&Path], version: &Version) -> Result<()>;
        fn clear_consumed_for_prerelease(&self, changeset_dir: &Path, paths: &[&Path]) -> Result<()>;
    }
}

pub struct MockChangesetWriter {
    written: Mutex<Vec<(PathBuf, Changeset)>>,
    filename: String,
}

impl MockChangesetWriter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            written: Mutex::new(Vec::new()),
            filename: "mock-changeset.md".to_string(),
        }
    }

    #[must_use]
    pub fn with_filename(mut self, filename: &str) -> Self {
        self.filename = filename.to_string();
        self
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn written_changesets(&self) -> Vec<(PathBuf, Changeset)> {
        self.written.lock().expect("lock poisoned").clone()
    }
}

impl Default for MockChangesetWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl ChangesetWriter for MockChangesetWriter {
    fn write_changeset(&self, changeset_dir: &Path, changeset: &Changeset) -> Result<String> {
        self.written
            .lock()
            .expect("lock poisoned")
            .push((changeset_dir.to_path_buf(), changeset.clone()));
        Ok(self.filename.clone())
    }

    fn restore_changeset(&self, path: &Path, changeset: &Changeset) -> Result<()> {
        self.written
            .lock()
            .expect("lock poisoned")
            .push((path.to_path_buf(), changeset.clone()));
        Ok(())
    }

    fn filename_exists(&self, _changeset_dir: &Path, _filename: &str) -> bool {
        false
    }

    fn mark_consumed_for_prerelease(
        &self,
        _changeset_dir: &Path,
        _paths: &[&Path],
        _version: &Version,
    ) -> Result<()> {
        Ok(())
    }

    fn clear_consumed_for_prerelease(&self, _changeset_dir: &Path, _paths: &[&Path]) -> Result<()> {
        Ok(())
    }
}

struct MockGitState {
    staged_files: Vec<PathBuf>,
    commits: Vec<String>,
    tags_created: Vec<(String, String)>,
    deleted_files: Vec<PathBuf>,
    deleted_tags: Vec<String>,
    reset_count: usize,
    fail_on_commit: bool,
    fail_on_create_tag: bool,
    fail_on_create_tag_nth: Option<usize>,
    fail_on_stage_files: bool,
}

pub struct MockGitProvider {
    changed_files: Vec<FileChange>,
    clean: bool,
    branch: String,
    remote_url: Option<String>,
    state: Mutex<MockGitState>,
}

impl MockGitProvider {
    #[must_use]
    pub fn new() -> Self {
        Self {
            changed_files: Vec::new(),
            clean: true,
            branch: "main".to_string(),
            remote_url: None,
            state: Mutex::new(MockGitState {
                staged_files: Vec::new(),
                commits: Vec::new(),
                tags_created: Vec::new(),
                deleted_files: Vec::new(),
                deleted_tags: Vec::new(),
                reset_count: 0,
                fail_on_commit: false,
                fail_on_create_tag: false,
                fail_on_create_tag_nth: None,
                fail_on_stage_files: false,
            }),
        }
    }

    #[must_use]
    pub fn with_changed_files(mut self, files: Vec<FileChange>) -> Self {
        self.changed_files = files;
        self
    }

    #[must_use]
    pub fn with_branch(mut self, branch: &str) -> Self {
        self.branch = branch.to_string();
        self
    }

    #[must_use]
    pub fn is_clean(mut self, clean: bool) -> Self {
        self.clean = clean;
        self
    }

    #[must_use]
    pub fn with_remote_url(mut self, url: &str) -> Self {
        self.remote_url = Some(url.to_string());
        self
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn staged_files(&self) -> Vec<PathBuf> {
        self.state
            .lock()
            .expect("lock poisoned")
            .staged_files
            .clone()
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn commits(&self) -> Vec<String> {
        self.state.lock().expect("lock poisoned").commits.clone()
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn tags_created(&self) -> Vec<(String, String)> {
        self.state
            .lock()
            .expect("lock poisoned")
            .tags_created
            .clone()
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn deleted_files(&self) -> Vec<PathBuf> {
        self.state
            .lock()
            .expect("lock poisoned")
            .deleted_files
            .clone()
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn deleted_tags(&self) -> Vec<String> {
        self.state
            .lock()
            .expect("lock poisoned")
            .deleted_tags
            .clone()
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn reset_count(&self) -> usize {
        self.state.lock().expect("lock poisoned").reset_count
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn set_fail_on_commit(&self, fail: bool) {
        self.state.lock().expect("lock poisoned").fail_on_commit = fail;
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn set_fail_on_create_tag(&self, fail: bool) {
        self.state.lock().expect("lock poisoned").fail_on_create_tag = fail;
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn set_fail_on_create_tag_nth(&self, n: usize) {
        self.state
            .lock()
            .expect("lock poisoned")
            .fail_on_create_tag_nth = Some(n);
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn set_fail_on_stage_files(&self, fail: bool) {
        self.state
            .lock()
            .expect("lock poisoned")
            .fail_on_stage_files = fail;
    }
}

impl Default for MockGitProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GitDiffProvider for MockGitProvider {
    fn changed_files(
        &self,
        _project_root: &Path,
        _base: &str,
        _head: &str,
    ) -> Result<Vec<FileChange>> {
        Ok(self.changed_files.clone())
    }
}

impl GitStatusProvider for MockGitProvider {
    fn is_working_tree_clean(&self, _project_root: &Path) -> Result<bool> {
        Ok(self.clean)
    }

    fn current_branch(&self, _project_root: &Path) -> Result<String> {
        Ok(self.branch.clone())
    }

    fn remote_url(&self, _project_root: &Path) -> Result<Option<String>> {
        Ok(self.remote_url.clone())
    }
}

impl GitStagingProvider for MockGitProvider {
    fn stage_files(&self, _project_root: &Path, paths: &[&Path]) -> Result<()> {
        let mut state = self.state.lock().expect("lock poisoned");
        if state.fail_on_stage_files {
            return Err(crate::OperationError::Io(std::io::Error::other(
                "mock stage files failure",
            )));
        }
        state
            .staged_files
            .extend(paths.iter().map(|p| p.to_path_buf()));
        Ok(())
    }

    fn delete_files(&self, _project_root: &Path, paths: &[&Path]) -> Result<()> {
        self.state
            .lock()
            .expect("lock poisoned")
            .deleted_files
            .extend(paths.iter().map(|p| p.to_path_buf()));
        Ok(())
    }
}

impl GitCommitProvider for MockGitProvider {
    fn commit(&self, _project_root: &Path, message: &str) -> Result<CommitInfo> {
        let mut state = self.state.lock().expect("lock poisoned");
        if state.fail_on_commit {
            return Err(crate::OperationError::Io(std::io::Error::other(
                "mock commit failure",
            )));
        }
        state.commits.push(message.to_string());
        Ok(CommitInfo {
            sha: "abc123def456".to_string(),
            message: message.to_string(),
        })
    }

    fn reset_to_parent(&self, _project_root: &Path) -> Result<()> {
        self.state.lock().expect("lock poisoned").reset_count += 1;
        Ok(())
    }
}

impl GitTagProvider for MockGitProvider {
    fn create_tag(&self, _project_root: &Path, tag_name: &str, message: &str) -> Result<TagInfo> {
        let mut state = self.state.lock().expect("lock poisoned");
        if state.fail_on_create_tag {
            return Err(crate::OperationError::Io(std::io::Error::other(
                "mock create tag failure",
            )));
        }

        let current_count = state.tags_created.len();
        if let Some(n) = state.fail_on_create_tag_nth {
            if current_count == n {
                return Err(crate::OperationError::Io(std::io::Error::other(
                    "mock create tag failure (nth)",
                )));
            }
        }

        state
            .tags_created
            .push((tag_name.to_string(), message.to_string()));
        Ok(TagInfo {
            name: tag_name.to_string(),
            target_sha: "abc123def456".to_string(),
        })
    }

    fn delete_tag(&self, _project_root: &Path, tag_name: &str) -> Result<bool> {
        self.state
            .lock()
            .expect("lock poisoned")
            .deleted_tags
            .push(tag_name.to_string());
        Ok(true)
    }
}

impl_arc_delegation! {
    impl GitDiffProvider for Arc<MockGitProvider> {
        fn changed_files(&self, project_root: &Path, base: &str, head: &str) -> Result<Vec<FileChange>>;
    }
}

impl_arc_delegation! {
    impl GitStatusProvider for Arc<MockGitProvider> {
        fn is_working_tree_clean(&self, project_root: &Path) -> Result<bool>;
        fn current_branch(&self, project_root: &Path) -> Result<String>;
        fn remote_url(&self, project_root: &Path) -> Result<Option<String>>;
    }
}

impl_arc_delegation! {
    impl GitStagingProvider for Arc<MockGitProvider> {
        fn stage_files(&self, project_root: &Path, paths: &[&Path]) -> Result<()>;
        fn delete_files(&self, project_root: &Path, paths: &[&Path]) -> Result<()>;
    }
}

impl_arc_delegation! {
    impl GitCommitProvider for Arc<MockGitProvider> {
        fn commit(&self, project_root: &Path, message: &str) -> Result<CommitInfo>;
        fn reset_to_parent(&self, project_root: &Path) -> Result<()>;
    }
}

impl_arc_delegation! {
    impl GitTagProvider for Arc<MockGitProvider> {
        fn create_tag(&self, project_root: &Path, tag_name: &str, message: &str) -> Result<TagInfo>;
        fn delete_tag(&self, project_root: &Path, tag_name: &str) -> Result<bool>;
    }
}

pub struct MockInteractionProvider {
    pub package_selection: PackageSelection,
    pub bump_selections: Mutex<Vec<BumpType>>,
    pub category_selection: CategorySelection,
    pub description: DescriptionInput,
}

impl MockInteractionProvider {
    #[must_use]
    pub fn all_cancelled() -> Self {
        Self {
            package_selection: PackageSelection::Cancelled,
            bump_selections: Mutex::new(Vec::new()),
            category_selection: CategorySelection::Cancelled,
            description: DescriptionInput::Cancelled,
        }
    }

    #[must_use]
    pub fn with_selections(packages: Vec<PackageInfo>, bump: BumpType, description: &str) -> Self {
        Self {
            package_selection: PackageSelection::Selected(packages),
            bump_selections: Mutex::new(vec![bump]),
            category_selection: CategorySelection::Selected(ChangeCategory::Changed),
            description: DescriptionInput::Provided(description.to_string()),
        }
    }

    #[must_use]
    pub fn with_bump_sequence(self, bumps: Vec<BumpType>) -> Self {
        Self {
            bump_selections: Mutex::new(bumps),
            ..self
        }
    }

    #[must_use]
    pub fn with_category(self, category: ChangeCategory) -> Self {
        Self {
            category_selection: CategorySelection::Selected(category),
            ..self
        }
    }
}

impl InteractionProvider for MockInteractionProvider {
    fn select_packages(&self, _available: &[PackageInfo]) -> Result<PackageSelection> {
        Ok(self.package_selection.clone())
    }

    fn select_bump_type(&self, _package_name: &str) -> Result<BumpSelection> {
        let mut selections = self.bump_selections.lock().expect("lock poisoned");
        if selections.is_empty() {
            return Ok(BumpSelection::Cancelled);
        }
        let bump = selections.remove(0);
        Ok(BumpSelection::Selected(bump))
    }

    fn select_category(&self) -> Result<CategorySelection> {
        Ok(self.category_selection.clone())
    }

    fn get_description(&self) -> Result<DescriptionInput> {
        Ok(self.description.clone())
    }
}

/// # Panics
///
/// Panics if the version string is not valid semver.
#[must_use]
pub fn make_package(name: &str, version: &str) -> PackageInfo {
    PackageInfo {
        name: name.to_string(),
        version: version.parse().expect("valid version"),
        path: PathBuf::from(format!("/mock/crates/{name}")),
    }
}

#[must_use]
pub fn make_changeset(package_name: &str, bump: BumpType, summary: &str) -> Changeset {
    Changeset {
        summary: summary.to_string(),
        releases: vec![changeset_core::PackageRelease {
            name: package_name.to_string(),
            bump_type: bump,
        }],
        category: ChangeCategory::Changed,
        consumed_for_prerelease: None,
        graduate: false,
    }
}

struct MockManifestState {
    written_versions: Vec<(PathBuf, Version)>,
    dependency_version_updates: Vec<(PathBuf, String, Version)>,
    dependency_update_returns_true: bool,
    removed_workspace_version: bool,
    workspace_version: Option<Version>,
    written_metadata: Vec<(PathBuf, MetadataSection, InitConfig)>,
}

pub struct MockManifestWriter {
    state: Mutex<MockManifestState>,
    inherited_paths: HashSet<PathBuf>,
}

impl MockManifestWriter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Mutex::new(MockManifestState {
                written_versions: Vec::new(),
                dependency_version_updates: Vec::new(),
                dependency_update_returns_true: false,
                removed_workspace_version: false,
                workspace_version: None,
                written_metadata: Vec::new(),
            }),
            inherited_paths: HashSet::new(),
        }
    }

    #[must_use]
    pub fn with_inherited(mut self, paths: Vec<PathBuf>) -> Self {
        self.inherited_paths = paths.into_iter().collect();
        self
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn with_dependency_updates_returning_true(self) -> Self {
        self.state
            .lock()
            .expect("lock poisoned")
            .dependency_update_returns_true = true;
        self
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn dependency_version_updates(&self) -> Vec<(PathBuf, String, Version)> {
        self.state
            .lock()
            .expect("lock poisoned")
            .dependency_version_updates
            .clone()
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn with_workspace_version(self, version: Version) -> Self {
        self.state.lock().expect("lock poisoned").workspace_version = Some(version);
        self
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn written_versions(&self) -> Vec<(PathBuf, Version)> {
        self.state
            .lock()
            .expect("lock poisoned")
            .written_versions
            .clone()
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn workspace_version_removed(&self) -> bool {
        self.state
            .lock()
            .expect("lock poisoned")
            .removed_workspace_version
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn get_workspace_version(&self) -> Option<Version> {
        self.state
            .lock()
            .expect("lock poisoned")
            .workspace_version
            .clone()
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn written_metadata(&self) -> Vec<(PathBuf, MetadataSection, InitConfig)> {
        self.state
            .lock()
            .expect("lock poisoned")
            .written_metadata
            .clone()
    }
}

impl Default for MockManifestWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl InheritedVersionChecker for MockManifestWriter {
    fn has_inherited_version(&self, manifest_path: &Path) -> Result<bool> {
        Ok(self.inherited_paths.contains(manifest_path))
    }
}

impl ManifestVersionWriter for MockManifestWriter {
    fn write_version(&self, manifest_path: &Path, new_version: &Version) -> Result<()> {
        self.state
            .lock()
            .expect("lock poisoned")
            .written_versions
            .push((manifest_path.to_path_buf(), new_version.clone()));
        Ok(())
    }

    fn verify_version(&self, _manifest_path: &Path, _expected: &Version) -> Result<()> {
        Ok(())
    }
}

impl ManifestDependencyWriter for MockManifestWriter {
    fn update_dependency_version(
        &self,
        manifest_path: &Path,
        dependency_name: &str,
        new_version: &Version,
    ) -> Result<bool> {
        let mut state = self.state.lock().expect("lock poisoned");
        let returns_true = state.dependency_update_returns_true;
        if returns_true {
            state.dependency_version_updates.push((
                manifest_path.to_path_buf(),
                dependency_name.to_string(),
                new_version.clone(),
            ));
        }
        Ok(returns_true)
    }
}

impl WorkspaceVersionManager for MockManifestWriter {
    fn read_workspace_version(&self, _manifest_path: &Path) -> Result<Option<Version>> {
        Ok(self
            .state
            .lock()
            .expect("lock poisoned")
            .workspace_version
            .clone())
    }

    fn remove_workspace_version(&self, _manifest_path: &Path) -> Result<()> {
        let mut state = self.state.lock().expect("lock poisoned");
        state.removed_workspace_version = true;
        state.workspace_version = None;
        Ok(())
    }

    fn write_workspace_version(&self, _manifest_path: &Path, version: &Version) -> Result<()> {
        let mut state = self.state.lock().expect("lock poisoned");
        state.workspace_version = Some(version.clone());
        state.removed_workspace_version = false;
        Ok(())
    }
}

impl ManifestMetadataWriter for MockManifestWriter {
    fn write_metadata(
        &self,
        manifest_path: &Path,
        section: MetadataSection,
        config: &InitConfig,
    ) -> Result<()> {
        self.state
            .lock()
            .expect("lock poisoned")
            .written_metadata
            .push((manifest_path.to_path_buf(), section, config.clone()));
        Ok(())
    }
}

impl_arc_delegation! {
    impl InheritedVersionChecker for Arc<MockManifestWriter> {
        fn has_inherited_version(&self, manifest_path: &Path) -> Result<bool>;
    }
}

impl_arc_delegation! {
    impl ManifestVersionWriter for Arc<MockManifestWriter> {
        fn write_version(&self, manifest_path: &Path, new_version: &Version) -> Result<()>;
        fn verify_version(&self, manifest_path: &Path, expected: &Version) -> Result<()>;
    }
}

impl_arc_delegation! {
    impl ManifestDependencyWriter for Arc<MockManifestWriter> {
        fn update_dependency_version(&self, manifest_path: &Path, dependency_name: &str, new_version: &Version) -> Result<bool>;
    }
}

impl_arc_delegation! {
    impl WorkspaceVersionManager for Arc<MockManifestWriter> {
        fn read_workspace_version(&self, manifest_path: &Path) -> Result<Option<Version>>;
        fn remove_workspace_version(&self, manifest_path: &Path) -> Result<()>;
        fn write_workspace_version(&self, manifest_path: &Path, version: &Version) -> Result<()>;
    }
}

impl_arc_delegation! {
    impl ManifestMetadataWriter for Arc<MockManifestWriter> {
        fn write_metadata(&self, manifest_path: &Path, section: MetadataSection, config: &InitConfig) -> Result<()>;
    }
}

pub struct MockChangelogWriter {
    written: Mutex<Vec<(PathBuf, VersionRelease)>>,
    existing_changelogs: HashSet<PathBuf>,
}

impl MockChangelogWriter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            written: Mutex::new(Vec::new()),
            existing_changelogs: HashSet::new(),
        }
    }

    #[must_use]
    pub fn with_existing_changelog(mut self, path: PathBuf) -> Self {
        self.existing_changelogs.insert(path);
        self
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn written_releases(&self) -> Vec<(PathBuf, VersionRelease)> {
        self.written.lock().expect("lock poisoned").clone()
    }
}

impl Default for MockChangelogWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for MockChangelogWriter {
    fn clone(&self) -> Self {
        Self {
            written: Mutex::new(self.written.lock().expect("lock poisoned").clone()),
            existing_changelogs: self.existing_changelogs.clone(),
        }
    }
}

impl ChangelogWriter for MockChangelogWriter {
    fn write_release(
        &self,
        changelog_path: &Path,
        release: &VersionRelease,
        _repo_info: Option<&RepositoryInfo>,
        _previous_version: Option<&str>,
    ) -> Result<ChangelogWriteResult> {
        let created = !self.existing_changelogs.contains(changelog_path);

        self.written
            .lock()
            .expect("lock poisoned")
            .push((changelog_path.to_path_buf(), release.clone()));

        Ok(ChangelogWriteResult {
            path: changelog_path.to_path_buf(),
            created,
        })
    }

    fn changelog_exists(&self, path: &Path) -> bool {
        self.existing_changelogs.contains(path)
    }

    fn restore_changelog(&self, _path: &Path, _content: &str) -> Result<()> {
        Ok(())
    }

    fn delete_changelog(&self, _path: &Path) -> Result<()> {
        Ok(())
    }
}

impl_arc_delegation! {
    impl ChangelogWriter for Arc<MockChangelogWriter> {
        fn write_release(&self, changelog_path: &Path, release: &VersionRelease, repo_info: Option<&RepositoryInfo>, previous_version: Option<&str>) -> Result<ChangelogWriteResult>;
        fn changelog_exists(&self, path: &Path) -> bool;
        fn restore_changelog(&self, path: &Path, content: &str) -> Result<()>;
        fn delete_changelog(&self, path: &Path) -> Result<()>;
    }
}

pub struct MockInheritedVersionChecker {
    inherited_paths: HashSet<PathBuf>,
}

impl MockInheritedVersionChecker {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inherited_paths: HashSet::new(),
        }
    }

    #[must_use]
    pub fn with_inherited(mut self, paths: Vec<PathBuf>) -> Self {
        self.inherited_paths = paths.into_iter().collect();
        self
    }
}

impl Default for MockInheritedVersionChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl InheritedVersionChecker for MockInheritedVersionChecker {
    fn has_inherited_version(&self, manifest_path: &Path) -> Result<bool> {
        Ok(self.inherited_paths.contains(manifest_path))
    }
}

pub struct FailingInheritedVersionChecker;

impl InheritedVersionChecker for FailingInheritedVersionChecker {
    fn has_inherited_version(&self, manifest_path: &Path) -> Result<bool> {
        Err(crate::OperationError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("mock read error for {}", manifest_path.display()),
        )))
    }
}

pub struct MockReleaseStateIO {
    prerelease_state: RwLock<Option<PrereleaseState>>,
    graduation_state: RwLock<Option<GraduationState>>,
}

impl MockReleaseStateIO {
    #[must_use]
    pub fn new() -> Self {
        Self {
            prerelease_state: RwLock::new(None),
            graduation_state: RwLock::new(None),
        }
    }

    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn with_prerelease_state(self, state: PrereleaseState) -> Self {
        *self.prerelease_state.write().expect("lock poisoned") = Some(state);
        self
    }

    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn with_graduation_state(self, state: GraduationState) -> Self {
        *self.graduation_state.write().expect("lock poisoned") = Some(state);
        self
    }

    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn get_graduation_state(&self) -> Option<GraduationState> {
        self.graduation_state.read().expect("lock poisoned").clone()
    }

    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn get_prerelease_state(&self) -> Option<PrereleaseState> {
        self.prerelease_state.read().expect("lock poisoned").clone()
    }
}

impl Default for MockReleaseStateIO {
    fn default() -> Self {
        Self::new()
    }
}

impl ReleaseStateIO for MockReleaseStateIO {
    fn load_prerelease_state(&self, _changeset_dir: &Path) -> Result<Option<PrereleaseState>> {
        Ok(self.prerelease_state.read().expect("lock poisoned").clone())
    }

    fn save_prerelease_state(&self, _changeset_dir: &Path, state: &PrereleaseState) -> Result<()> {
        *self.prerelease_state.write().expect("lock poisoned") = if state.is_empty() {
            None
        } else {
            Some(state.clone())
        };
        Ok(())
    }

    fn load_graduation_state(&self, _changeset_dir: &Path) -> Result<Option<GraduationState>> {
        Ok(self.graduation_state.read().expect("lock poisoned").clone())
    }

    fn save_graduation_state(&self, _changeset_dir: &Path, state: &GraduationState) -> Result<()> {
        *self.graduation_state.write().expect("lock poisoned") = if state.is_empty() {
            None
        } else {
            Some(state.clone())
        };
        Ok(())
    }
}

impl_arc_delegation! {
    impl ReleaseStateIO for Arc<MockReleaseStateIO> {
        fn load_prerelease_state(&self, changeset_dir: &Path) -> Result<Option<PrereleaseState>>;
        fn save_prerelease_state(&self, changeset_dir: &Path, state: &PrereleaseState) -> Result<()>;
        fn load_graduation_state(&self, changeset_dir: &Path) -> Result<Option<GraduationState>>;
        fn save_graduation_state(&self, changeset_dir: &Path, state: &GraduationState) -> Result<()>;
    }
}

#[allow(clippy::struct_field_names, clippy::option_option)]
pub struct MockInitInteractionProvider {
    git_settings: Mutex<Option<Option<GitSettingsInput>>>,
    changelog_settings: Mutex<Option<Option<ChangelogSettingsInput>>>,
    version_settings: Mutex<Option<Option<VersionSettingsInput>>>,
}

impl MockInitInteractionProvider {
    #[must_use]
    pub fn new() -> Self {
        Self {
            git_settings: Mutex::new(None),
            changelog_settings: Mutex::new(None),
            version_settings: Mutex::new(None),
        }
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn with_git_settings(self, settings: Option<GitSettingsInput>) -> Self {
        *self.git_settings.lock().expect("lock poisoned") = Some(settings);
        self
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn with_changelog_settings(self, settings: Option<ChangelogSettingsInput>) -> Self {
        *self.changelog_settings.lock().expect("lock poisoned") = Some(settings);
        self
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn with_version_settings(self, settings: Option<VersionSettingsInput>) -> Self {
        *self.version_settings.lock().expect("lock poisoned") = Some(settings);
        self
    }

    #[must_use]
    pub fn all_skipped() -> Self {
        Self::new()
            .with_git_settings(None)
            .with_changelog_settings(None)
            .with_version_settings(None)
    }

    #[must_use]
    pub fn all_defaults() -> Self {
        Self::new()
            .with_git_settings(Some(GitSettingsInput::default()))
            .with_changelog_settings(Some(ChangelogSettingsInput::default()))
            .with_version_settings(Some(VersionSettingsInput::default()))
    }
}

impl Default for MockInitInteractionProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl InitInteractionProvider for MockInitInteractionProvider {
    fn configure_git_settings(&self, _context: ProjectContext) -> Result<Option<GitSettingsInput>> {
        Ok(self
            .git_settings
            .lock()
            .expect("lock poisoned")
            .clone()
            .flatten())
    }

    fn configure_changelog_settings(
        &self,
        _context: ProjectContext,
    ) -> Result<Option<ChangelogSettingsInput>> {
        Ok(self
            .changelog_settings
            .lock()
            .expect("lock poisoned")
            .clone()
            .flatten())
    }

    fn configure_version_settings(&self) -> Result<Option<VersionSettingsInput>> {
        Ok(self
            .version_settings
            .lock()
            .expect("lock poisoned")
            .clone()
            .flatten())
    }
}

impl_arc_delegation! {
    impl InitInteractionProvider for Arc<MockInitInteractionProvider> {
        fn configure_git_settings(&self, context: ProjectContext) -> Result<Option<GitSettingsInput>>;
        fn configure_changelog_settings(&self, context: ProjectContext) -> Result<Option<ChangelogSettingsInput>>;
        fn configure_version_settings(&self) -> Result<Option<VersionSettingsInput>>;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_changeset(name: &str) -> Changeset {
        make_changeset(name, BumpType::Patch, &format!("Fix {name}"))
    }

    #[test]
    fn mock_list_changesets_filters_consumed() {
        let changeset_dir = PathBuf::from("/mock/.changeset");
        let unconsumed_path = changeset_dir.join("unconsumed.md");
        let consumed_path = changeset_dir.join("consumed.md");

        let reader = MockChangesetReader::new()
            .with_changeset(unconsumed_path.clone(), make_test_changeset("pkg-a"))
            .with_consumed_changeset(
                consumed_path.clone(),
                make_test_changeset("pkg-b"),
                "1.0.0-pre.1".to_string(),
            );

        let listed = reader
            .list_changesets(&changeset_dir)
            .expect("list_changesets should succeed");

        assert_eq!(listed.len(), 1);
        assert!(listed.contains(&unconsumed_path));
        assert!(!listed.contains(&consumed_path));
    }

    #[test]
    fn mock_list_consumed_changesets_returns_consumed() {
        let changeset_dir = PathBuf::from("/mock/.changeset");
        let unconsumed_path = changeset_dir.join("unconsumed.md");
        let consumed_path = changeset_dir.join("consumed.md");

        let reader = MockChangesetReader::new()
            .with_changeset(unconsumed_path.clone(), make_test_changeset("pkg-a"))
            .with_consumed_changeset(
                consumed_path.clone(),
                make_test_changeset("pkg-b"),
                "1.0.0-pre.1".to_string(),
            );

        let consumed = reader
            .list_consumed_changesets(&changeset_dir)
            .expect("list_consumed_changesets should succeed");

        assert_eq!(consumed.len(), 1);
        assert!(consumed.contains(&consumed_path));
        assert!(!consumed.contains(&unconsumed_path));
    }

    #[test]
    fn mock_mark_consumed_updates_state() {
        let changeset_dir = PathBuf::from("/mock/.changeset");
        let path = changeset_dir.join("changeset.md");

        let reader =
            MockChangesetReader::new().with_changeset(path.clone(), make_test_changeset("pkg-a"));

        assert!(reader.get_consumed_status(&path).is_none());

        let version: Version = "2.0.0-pre.1".parse().expect("valid version");
        reader
            .mark_consumed_for_prerelease(&changeset_dir, &[path.as_path()], &version)
            .expect("mark_consumed should succeed");

        assert_eq!(
            reader.get_consumed_status(&path),
            Some("2.0.0-pre.1".to_string())
        );

        let listed = reader
            .list_changesets(&changeset_dir)
            .expect("list_changesets should succeed");
        assert!(listed.is_empty());

        let consumed = reader
            .list_consumed_changesets(&changeset_dir)
            .expect("list_consumed_changesets should succeed");
        assert_eq!(consumed.len(), 1);
        assert!(consumed.contains(&path));
    }

    #[test]
    fn mock_clear_consumed_updates_state() {
        let changeset_dir = PathBuf::from("/mock/.changeset");
        let path = changeset_dir.join("changeset.md");

        let reader = MockChangesetReader::new().with_consumed_changeset(
            path.clone(),
            make_test_changeset("pkg-a"),
            "1.0.0-pre.1".to_string(),
        );

        assert_eq!(
            reader.get_consumed_status(&path),
            Some("1.0.0-pre.1".to_string())
        );

        reader
            .clear_consumed_for_prerelease(&changeset_dir, &[path.as_path()])
            .expect("clear_consumed should succeed");

        assert!(reader.get_consumed_status(&path).is_none());

        let consumed = reader
            .list_consumed_changesets(&changeset_dir)
            .expect("list_consumed_changesets should succeed");
        assert!(consumed.is_empty());

        let listed = reader
            .list_changesets(&changeset_dir)
            .expect("list_changesets should succeed");
        assert_eq!(listed.len(), 1);
        assert!(listed.contains(&path));
    }
}
