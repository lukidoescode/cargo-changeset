use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use changeset_changelog::{RepositoryInfo, VersionRelease};
use changeset_core::{BumpType, ChangeCategory, Changeset, PackageInfo};
use changeset_git::{CommitInfo, FileChange, TagInfo};
use changeset_project::{CargoProject, PackageChangesetConfig, ProjectKind, RootChangesetConfig};
use semver::Version;

use crate::Result;
use crate::traits::{
    BumpSelection, CategorySelection, ChangelogWriteResult, ChangelogWriter, ChangesetReader,
    ChangesetWriter, DescriptionInput, GitProvider, InheritedVersionChecker, InteractionProvider,
    ManifestWriter, PackageSelection, ProjectProvider,
};

pub struct MockProjectProvider {
    project: CargoProject,
    changeset_dir: PathBuf,
}

impl MockProjectProvider {
    #[must_use]
    pub fn new(project: CargoProject) -> Self {
        let changeset_dir = project.root.join(".changeset");
        Self {
            project,
            changeset_dir,
        }
    }

    #[must_use]
    pub fn with_changeset_dir(mut self, dir: PathBuf) -> Self {
        self.changeset_dir = dir;
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
        Ok((RootChangesetConfig::default(), HashMap::new()))
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
    changesets: HashMap<PathBuf, Changeset>,
    listed_files: Vec<PathBuf>,
}

impl MockChangesetReader {
    #[must_use]
    pub fn new() -> Self {
        Self {
            changesets: HashMap::new(),
            listed_files: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_changeset(mut self, path: PathBuf, changeset: Changeset) -> Self {
        self.listed_files.push(path.clone());
        self.changesets.insert(path, changeset);
        self
    }

    #[must_use]
    pub fn with_changesets(mut self, changesets: Vec<(PathBuf, Changeset)>) -> Self {
        for (path, changeset) in changesets {
            self.listed_files.push(path.clone());
            self.changesets.insert(path, changeset);
        }
        self
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
            .get(path)
            .cloned()
            .ok_or_else(|| crate::OperationError::ChangesetFileRead {
                path: path.to_path_buf(),
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "mock file not found"),
            })
    }

    fn list_changesets(&self, _changeset_dir: &Path) -> Result<Vec<PathBuf>> {
        Ok(self.listed_files.clone())
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

    fn filename_exists(&self, _changeset_dir: &Path, _filename: &str) -> bool {
        false
    }
}

pub struct MockGitProvider {
    changed_files: Vec<FileChange>,
    clean: bool,
    branch: String,
    remote_url: Option<String>,
}

impl MockGitProvider {
    #[must_use]
    pub fn new() -> Self {
        Self {
            changed_files: Vec::new(),
            clean: true,
            branch: "main".to_string(),
            remote_url: None,
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
}

impl Default for MockGitProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GitProvider for MockGitProvider {
    fn changed_files(
        &self,
        _project_root: &Path,
        _base: &str,
        _head: &str,
    ) -> Result<Vec<FileChange>> {
        Ok(self.changed_files.clone())
    }

    fn is_working_tree_clean(&self, _project_root: &Path) -> Result<bool> {
        Ok(self.clean)
    }

    fn current_branch(&self, _project_root: &Path) -> Result<String> {
        Ok(self.branch.clone())
    }

    fn stage_files(&self, _project_root: &Path, _paths: &[&Path]) -> Result<()> {
        Ok(())
    }

    fn commit(&self, _project_root: &Path, message: &str) -> Result<CommitInfo> {
        Ok(CommitInfo {
            sha: "abc123def456".to_string(),
            message: message.to_string(),
        })
    }

    fn create_tag(&self, _project_root: &Path, tag_name: &str, _message: &str) -> Result<TagInfo> {
        Ok(TagInfo {
            name: tag_name.to_string(),
            target_sha: "abc123def456".to_string(),
        })
    }

    fn remote_url(&self, _project_root: &Path) -> Result<Option<String>> {
        Ok(self.remote_url.clone())
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
    }
}

pub struct MockManifestWriter {
    written_versions: Mutex<Vec<(PathBuf, Version)>>,
    inherited_paths: HashSet<PathBuf>,
    removed_workspace_version: Mutex<bool>,
}

impl MockManifestWriter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            written_versions: Mutex::new(Vec::new()),
            inherited_paths: HashSet::new(),
            removed_workspace_version: Mutex::new(false),
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
    pub fn written_versions(&self) -> Vec<(PathBuf, Version)> {
        self.written_versions.lock().expect("lock poisoned").clone()
    }

    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn workspace_version_removed(&self) -> bool {
        *self
            .removed_workspace_version
            .lock()
            .expect("lock poisoned")
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

impl ManifestWriter for MockManifestWriter {
    fn write_version(&self, manifest_path: &Path, new_version: &Version) -> Result<()> {
        self.written_versions
            .lock()
            .expect("lock poisoned")
            .push((manifest_path.to_path_buf(), new_version.clone()));
        Ok(())
    }

    fn remove_workspace_version(&self, _manifest_path: &Path) -> Result<()> {
        *self
            .removed_workspace_version
            .lock()
            .expect("lock poisoned") = true;
        Ok(())
    }

    fn verify_version(&self, _manifest_path: &Path, _expected: &Version) -> Result<()> {
        Ok(())
    }
}

impl InheritedVersionChecker for Arc<MockManifestWriter> {
    fn has_inherited_version(&self, manifest_path: &Path) -> Result<bool> {
        (**self).has_inherited_version(manifest_path)
    }
}

impl ManifestWriter for Arc<MockManifestWriter> {
    fn write_version(&self, manifest_path: &Path, new_version: &Version) -> Result<()> {
        (**self).write_version(manifest_path, new_version)
    }

    fn remove_workspace_version(&self, manifest_path: &Path) -> Result<()> {
        (**self).remove_workspace_version(manifest_path)
    }

    fn verify_version(&self, manifest_path: &Path, expected: &Version) -> Result<()> {
        (**self).verify_version(manifest_path, expected)
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
