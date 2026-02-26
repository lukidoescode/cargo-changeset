use std::marker::PhantomData;
use std::path::Path;

use changeset_project::TagFormat;
use changeset_saga::SagaStep;
use tracing::debug;

use super::context::ReleaseSagaContext;
use super::saga_data::{DependencyUpdate, ManifestUpdate, ReleaseSagaData};
use super::{CommitResult, TagResult};
use crate::OperationError;
use crate::traits::{
    ChangelogWriter, ChangesetReader, ChangesetWriter, GitProvider, ManifestWriter, ReleaseStateIO,
};

pub struct WriteManifestVersionsStep<G, M, RW, S, C> {
    _marker: PhantomData<(G, M, RW, S, C)>,
}

impl<G, M, RW, S, C> WriteManifestVersionsStep<G, M, RW, S, C> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<G, M, RW, S, C> Default for WriteManifestVersionsStep<G, M, RW, S, C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<G, M, RW, S, C> SagaStep for WriteManifestVersionsStep<G, M, RW, S, C>
where
    G: GitProvider + Send + Sync,
    M: ManifestWriter + Send + Sync,
    RW: ChangesetReader + ChangesetWriter + Send + Sync,
    S: ReleaseStateIO + Send + Sync,
    C: ChangelogWriter + Send + Sync,
{
    type Input = ReleaseSagaData;
    type Output = ReleaseSagaData;
    type Context = ReleaseSagaContext<G, M, RW, S, C>;
    type Error = OperationError;

    fn name(&self) -> &'static str {
        "write_manifest_versions"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        mut input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        let mut manifest_updates = Vec::new();

        for release in &input.planned_releases {
            if let Some(pkg_path) = input.package_paths.get(&release.name) {
                let manifest_path = pkg_path.join("Cargo.toml");
                ctx.manifest_writer()
                    .write_version(&manifest_path, &release.new_version)?;
                ctx.manifest_writer()
                    .verify_version(&manifest_path, &release.new_version)?;

                let update = ManifestUpdate {
                    manifest_path,
                    old_version: release.current_version.clone(),
                    new_version: release.new_version.clone(),
                    written: true,
                };
                debug!(
                    manifest = %update.manifest_path.display(),
                    old = %update.old_version,
                    new = %update.new_version,
                    written = update.written,
                    "updated manifest version"
                );
                manifest_updates.push(update);
            }
        }

        input.manifest_updates = manifest_updates;
        Ok(input)
    }

    fn compensate(&self, ctx: &Self::Context, input: Self::Input) -> Result<(), Self::Error> {
        debug!(
            count = input.manifest_updates.len(),
            "rolling back manifest version updates"
        );
        for release in &input.planned_releases {
            if let Some(pkg_path) = input.package_paths.get(&release.name) {
                let manifest_path = pkg_path.join("Cargo.toml");
                ctx.manifest_writer()
                    .write_version(&manifest_path, &release.current_version)?;
            }
        }
        Ok(())
    }

    fn compensation_description(&self) -> String {
        "restore original package versions in Cargo.toml files".to_string()
    }
}

pub struct UpdateDependencyVersionsStep<G, M, RW, S, C> {
    _marker: PhantomData<(G, M, RW, S, C)>,
}

impl<G, M, RW, S, C> UpdateDependencyVersionsStep<G, M, RW, S, C> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<G, M, RW, S, C> Default for UpdateDependencyVersionsStep<G, M, RW, S, C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<G, M, RW, S, C> SagaStep for UpdateDependencyVersionsStep<G, M, RW, S, C>
where
    G: GitProvider + Send + Sync,
    M: ManifestWriter + Send + Sync,
    RW: ChangesetReader + ChangesetWriter + Send + Sync,
    S: ReleaseStateIO + Send + Sync,
    C: ChangelogWriter + Send + Sync,
{
    type Input = ReleaseSagaData;
    type Output = ReleaseSagaData;
    type Context = ReleaseSagaContext<G, M, RW, S, C>;
    type Error = OperationError;

    fn name(&self) -> &'static str {
        "update_dependency_versions"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        mut input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        let mut dependency_updates = Vec::new();

        let mut manifest_paths: Vec<_> = input
            .package_paths
            .values()
            .map(|p| p.join("Cargo.toml"))
            .collect();
        manifest_paths.push(input.root_manifest_path.clone());

        for release in &input.planned_releases {
            for manifest_path in &manifest_paths {
                let updated = ctx.manifest_writer().update_dependency_version(
                    manifest_path,
                    &release.name,
                    &release.new_version,
                )?;

                if updated {
                    let update = DependencyUpdate {
                        manifest_path: manifest_path.clone(),
                        dependency_name: release.name.clone(),
                        old_version: release.current_version.clone(),
                        new_version: release.new_version.clone(),
                    };
                    debug!(
                        manifest = %update.manifest_path.display(),
                        dependency = %update.dependency_name,
                        old = %update.old_version,
                        new = %update.new_version,
                        "updated dependency version"
                    );
                    dependency_updates.push(update);
                }
            }
        }

        input.dependency_updates = dependency_updates;
        Ok(input)
    }

    fn compensate(&self, ctx: &Self::Context, input: Self::Input) -> Result<(), Self::Error> {
        debug!(
            count = input.dependency_updates.len(),
            "rolling back dependency version updates"
        );
        let mut manifest_paths: Vec<_> = input
            .package_paths
            .values()
            .map(|p| p.join("Cargo.toml"))
            .collect();
        manifest_paths.push(input.root_manifest_path.clone());

        for release in &input.planned_releases {
            for manifest_path in &manifest_paths {
                ctx.manifest_writer().update_dependency_version(
                    manifest_path,
                    &release.name,
                    &release.current_version,
                )?;
            }
        }
        Ok(())
    }

    fn compensation_description(&self) -> String {
        "restore original dependency versions in Cargo.toml files".to_string()
    }
}

pub struct RemoveWorkspaceVersionStep<G, M, RW, S, C> {
    _marker: PhantomData<(G, M, RW, S, C)>,
}

impl<G, M, RW, S, C> RemoveWorkspaceVersionStep<G, M, RW, S, C> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<G, M, RW, S, C> Default for RemoveWorkspaceVersionStep<G, M, RW, S, C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<G, M, RW, S, C> SagaStep for RemoveWorkspaceVersionStep<G, M, RW, S, C>
where
    G: GitProvider + Send + Sync,
    M: ManifestWriter + Send + Sync,
    RW: ChangesetReader + ChangesetWriter + Send + Sync,
    S: ReleaseStateIO + Send + Sync,
    C: ChangelogWriter + Send + Sync,
{
    type Input = ReleaseSagaData;
    type Output = ReleaseSagaData;
    type Context = ReleaseSagaContext<G, M, RW, S, C>;
    type Error = OperationError;

    fn name(&self) -> &'static str {
        "remove_workspace_version"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        mut input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        if !input.inherited_packages.is_empty() {
            input.original_workspace_version = ctx
                .manifest_writer()
                .read_workspace_version(&input.root_manifest_path)?;
            ctx.manifest_writer()
                .remove_workspace_version(&input.root_manifest_path)?;
            input.workspace_version_removed = true;
        }
        Ok(input)
    }

    fn compensate(&self, ctx: &Self::Context, input: Self::Input) -> Result<(), Self::Error> {
        if !input.inherited_packages.is_empty() {
            if let Some(version) = &input.original_workspace_version {
                ctx.manifest_writer()
                    .write_workspace_version(&input.root_manifest_path, version)?;
            } else if let Some(release) = input.planned_releases.first() {
                ctx.manifest_writer()
                    .write_workspace_version(&input.root_manifest_path, &release.current_version)?;
            }
        }
        Ok(())
    }

    fn compensation_description(&self) -> String {
        "restore workspace package version".to_string()
    }
}

pub struct MarkChangesetsConsumedStep<G, M, RW, S, C> {
    _marker: PhantomData<(G, M, RW, S, C)>,
}

impl<G, M, RW, S, C> MarkChangesetsConsumedStep<G, M, RW, S, C> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<G, M, RW, S, C> Default for MarkChangesetsConsumedStep<G, M, RW, S, C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<G, M, RW, S, C> SagaStep for MarkChangesetsConsumedStep<G, M, RW, S, C>
where
    G: GitProvider + Send + Sync,
    M: ManifestWriter + Send + Sync,
    RW: ChangesetReader + ChangesetWriter + Send + Sync,
    S: ReleaseStateIO + Send + Sync,
    C: ChangelogWriter + Send + Sync,
{
    type Input = ReleaseSagaData;
    type Output = ReleaseSagaData;
    type Context = ReleaseSagaContext<G, M, RW, S, C>;
    type Error = OperationError;

    fn name(&self) -> &'static str {
        "mark_changesets_consumed"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        mut input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        if input.is_prerelease_release && !input.changeset_files.is_empty() {
            if let Some(first_release) = input.planned_releases.first() {
                let paths_refs: Vec<&Path> = input
                    .changeset_files
                    .iter()
                    .map(|f| f.path.as_path())
                    .collect();
                ctx.changeset_rw().mark_consumed_for_prerelease(
                    &input.changeset_dir,
                    &paths_refs,
                    &first_release.new_version,
                )?;
                input.changesets_consumed = true;
            }
        }
        Ok(input)
    }

    fn compensate(&self, ctx: &Self::Context, input: Self::Input) -> Result<(), Self::Error> {
        // Check the same conditions as execute() to determine if we would have marked
        // changesets as consumed. We cannot rely on input.changesets_consumed because
        // compensate receives the original input, not the modified output.
        if input.is_prerelease_release && !input.changeset_files.is_empty() {
            let files_to_clear: Vec<&Path> = input
                .changeset_files
                .iter()
                .filter(|f| f.original_consumed_status.is_none())
                .map(|f| f.path.as_path())
                .collect();

            if !files_to_clear.is_empty() {
                ctx.changeset_rw()
                    .clear_consumed_for_prerelease(&input.changeset_dir, &files_to_clear)?;
            }
        }
        Ok(())
    }

    fn compensation_description(&self) -> String {
        "restore original changeset consumed status".to_string()
    }
}

pub struct ClearChangesetsConsumedStep<G, M, RW, S, C> {
    _marker: PhantomData<(G, M, RW, S, C)>,
}

impl<G, M, RW, S, C> ClearChangesetsConsumedStep<G, M, RW, S, C> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<G, M, RW, S, C> Default for ClearChangesetsConsumedStep<G, M, RW, S, C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<G, M, RW, S, C> SagaStep for ClearChangesetsConsumedStep<G, M, RW, S, C>
where
    G: GitProvider + Send + Sync,
    M: ManifestWriter + Send + Sync,
    RW: ChangesetReader + ChangesetWriter + Send + Sync,
    S: ReleaseStateIO + Send + Sync,
    C: ChangelogWriter + Send + Sync,
{
    type Input = ReleaseSagaData;
    type Output = ReleaseSagaData;
    type Context = ReleaseSagaContext<G, M, RW, S, C>;
    type Error = OperationError;

    fn name(&self) -> &'static str {
        "clear_changesets_consumed"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        mut input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        if input.is_graduating {
            let consumed_paths = ctx
                .changeset_rw()
                .list_consumed_changesets(&input.changeset_dir)?;

            if !consumed_paths.is_empty() {
                let mut consumed_files = Vec::new();
                for path in &consumed_paths {
                    if let Ok(changeset) = ctx.changeset_rw().read_changeset(path) {
                        consumed_files.push(super::steps::ChangesetFileState {
                            path: path.clone(),
                            original_consumed_status: changeset.consumed_for_prerelease.clone(),
                            backup: Some(changeset),
                        });
                    }
                }

                let paths_refs: Vec<&Path> = consumed_paths.iter().map(AsRef::as_ref).collect();
                ctx.changeset_rw()
                    .clear_consumed_for_prerelease(&input.changeset_dir, &paths_refs)?;
                input.consumed_cleared = true;
                input.consumed_files_cleared = consumed_files;
            }
        }
        Ok(input)
    }

    fn compensate(&self, ctx: &Self::Context, input: Self::Input) -> Result<(), Self::Error> {
        for file_state in &input.consumed_files_cleared {
            if let Some(original_version) = &file_state.original_consumed_status {
                let version: semver::Version =
                    original_version
                        .parse()
                        .map_err(|_| OperationError::VersionParse {
                            version: original_version.clone(),
                            context: "compensation restore consumed status".to_string(),
                        })?;
                ctx.changeset_rw().mark_consumed_for_prerelease(
                    &input.changeset_dir,
                    &[file_state.path.as_path()],
                    &version,
                )?;
            }
        }
        Ok(())
    }

    fn compensation_description(&self) -> String {
        "restore consumed changeset status".to_string()
    }
}

pub struct DeleteChangesetFilesStep<G, M, RW, S, C> {
    _marker: PhantomData<(G, M, RW, S, C)>,
}

impl<G, M, RW, S, C> DeleteChangesetFilesStep<G, M, RW, S, C> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<G, M, RW, S, C> Default for DeleteChangesetFilesStep<G, M, RW, S, C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<G, M, RW, S, C> SagaStep for DeleteChangesetFilesStep<G, M, RW, S, C>
where
    G: GitProvider + Send + Sync,
    M: ManifestWriter + Send + Sync,
    RW: ChangesetReader + ChangesetWriter + Send + Sync,
    S: ReleaseStateIO + Send + Sync,
    C: ChangelogWriter + Send + Sync,
{
    type Input = ReleaseSagaData;
    type Output = ReleaseSagaData;
    type Context = ReleaseSagaContext<G, M, RW, S, C>;
    type Error = OperationError;

    fn name(&self) -> &'static str {
        "delete_changeset_files"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        mut input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        let should_delete = input.should_delete_changesets
            && !input.is_prerelease_release
            && !input.is_prerelease_graduation;

        if should_delete && !input.changeset_files.is_empty() {
            for file_state in &mut input.changeset_files {
                file_state.backup = ctx.changeset_rw().read_changeset(&file_state.path).ok();
            }

            let paths_refs: Vec<&Path> = input
                .changeset_files
                .iter()
                .map(|f| f.path.as_path())
                .collect();
            ctx.git_provider()
                .delete_files(ctx.project_root(), &paths_refs)?;
            input.changesets_deleted = input
                .changeset_files
                .iter()
                .map(|f| f.path.clone())
                .collect();
        }
        Ok(input)
    }

    fn compensate(&self, ctx: &Self::Context, input: Self::Input) -> Result<(), Self::Error> {
        for file_state in &input.changeset_files {
            if let Some(changeset) = &file_state.backup {
                ctx.changeset_rw()
                    .restore_changeset(&file_state.path, changeset)?;
            }
        }
        Ok(())
    }

    fn compensation_description(&self) -> String {
        "restore deleted changeset files".to_string()
    }
}

pub struct StageFilesStep<G, M, RW, S, C> {
    _marker: PhantomData<(G, M, RW, S, C)>,
}

impl<G, M, RW, S, C> StageFilesStep<G, M, RW, S, C> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<G, M, RW, S, C> Default for StageFilesStep<G, M, RW, S, C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<G, M, RW, S, C> SagaStep for StageFilesStep<G, M, RW, S, C>
where
    G: GitProvider + Send + Sync,
    M: ManifestWriter + Send + Sync,
    RW: ChangesetReader + ChangesetWriter + Send + Sync,
    S: ReleaseStateIO + Send + Sync,
    C: ChangelogWriter + Send + Sync,
{
    type Input = ReleaseSagaData;
    type Output = ReleaseSagaData;
    type Context = ReleaseSagaContext<G, M, RW, S, C>;
    type Error = OperationError;

    fn name(&self) -> &'static str {
        "stage_files"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        mut input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        if !input.should_commit {
            return Ok(input);
        }

        let mut files = Vec::new();

        for update in &input.manifest_updates {
            files.push(update.manifest_path.clone());
        }

        if input.workspace_version_removed {
            files.push(input.root_manifest_path.clone());
        }

        for update in &input.changelog_updates {
            files.push(update.path.clone());
        }

        for update in &input.dependency_updates {
            files.push(update.manifest_path.clone());
        }

        if !input.changesets_deleted.is_empty() {
            files.extend(input.changesets_deleted.iter().cloned());
        }

        files.sort();
        files.dedup();

        if !files.is_empty() {
            let paths_refs: Vec<&Path> = files.iter().map(AsRef::as_ref).collect();
            ctx.git_provider()
                .stage_files(ctx.project_root(), &paths_refs)?;
            input.staged_files = files;
            input.files_were_staged = true;
        }

        Ok(input)
    }

    fn compensate(&self, _ctx: &Self::Context, _input: Self::Input) -> Result<(), Self::Error> {
        Ok(())
    }

    fn compensation_description(&self) -> String {
        "no action needed (file contents restored by other compensations)".to_string()
    }
}

pub struct CreateCommitStep<G, M, RW, S, C> {
    commit_title_template: String,
    include_changes_in_body: bool,
    _marker: PhantomData<(G, M, RW, S, C)>,
}

impl<G, M, RW, S, C> CreateCommitStep<G, M, RW, S, C> {
    #[must_use]
    pub fn new(commit_title_template: String, include_changes_in_body: bool) -> Self {
        Self {
            commit_title_template,
            include_changes_in_body,
            _marker: PhantomData,
        }
    }

    fn build_commit_message(&self, planned_releases: &[crate::types::PackageVersion]) -> String {
        let version_list: Vec<String> = planned_releases
            .iter()
            .map(|r| format!("{}@v{}", r.name, r.new_version))
            .collect();
        let new_version = version_list.join(", ");

        let title = self
            .commit_title_template
            .replace("{new-version}", &new_version);

        if !self.include_changes_in_body {
            return title;
        }

        let body: Vec<String> = planned_releases
            .iter()
            .map(|r| format!("- {} {} -> {}", r.name, r.current_version, r.new_version))
            .collect();

        format!("{}\n\n{}", title, body.join("\n"))
    }
}

impl<G, M, RW, S, C> SagaStep for CreateCommitStep<G, M, RW, S, C>
where
    G: GitProvider + Send + Sync,
    M: ManifestWriter + Send + Sync,
    RW: ChangesetReader + ChangesetWriter + Send + Sync,
    S: ReleaseStateIO + Send + Sync,
    C: ChangelogWriter + Send + Sync,
{
    type Input = ReleaseSagaData;
    type Output = ReleaseSagaData;
    type Context = ReleaseSagaContext<G, M, RW, S, C>;
    type Error = OperationError;

    fn name(&self) -> &'static str {
        "create_commit"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        mut input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        if !input.should_commit || !input.files_were_staged {
            return Ok(input);
        }

        let message = self.build_commit_message(&input.planned_releases);
        let commit_info = ctx.git_provider().commit(ctx.project_root(), &message)?;

        input.commit_result = Some(CommitResult {
            sha: commit_info.sha,
            message: commit_info.message,
        });

        Ok(input)
    }

    fn compensate(&self, ctx: &Self::Context, input: Self::Input) -> Result<(), Self::Error> {
        if input.should_commit {
            ctx.git_provider().reset_to_parent(ctx.project_root())?;
        }
        Ok(())
    }

    fn compensation_description(&self) -> String {
        "reset to parent commit".to_string()
    }
}

pub struct CreateTagsStep<G, M, RW, S, C> {
    tag_format: TagFormat,
    use_crate_prefix: bool,
    _marker: PhantomData<(G, M, RW, S, C)>,
}

impl<G, M, RW, S, C> CreateTagsStep<G, M, RW, S, C> {
    #[must_use]
    pub fn new(tag_format: TagFormat, use_crate_prefix: bool) -> Self {
        Self {
            tag_format,
            use_crate_prefix,
            _marker: PhantomData,
        }
    }
}

impl<G, M, RW, S, C> SagaStep for CreateTagsStep<G, M, RW, S, C>
where
    G: GitProvider + Send + Sync,
    M: ManifestWriter + Send + Sync,
    RW: ChangesetReader + ChangesetWriter + Send + Sync,
    S: ReleaseStateIO + Send + Sync,
    C: ChangelogWriter + Send + Sync,
{
    type Input = ReleaseSagaData;
    type Output = ReleaseSagaData;
    type Context = ReleaseSagaContext<G, M, RW, S, C>;
    type Error = OperationError;

    fn name(&self) -> &'static str {
        "create_tags"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        mut input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        if !input.should_create_tags || input.commit_result.is_none() {
            return Ok(input);
        }

        let use_prefix = self.use_crate_prefix || self.tag_format == TagFormat::CratePrefixed;

        let mut tags = Vec::new();
        let mut created_tag_names: Vec<String> = Vec::new();

        for release in &input.planned_releases {
            let tag_name = if use_prefix {
                format!("{}@v{}", release.name, release.new_version)
            } else {
                format!("v{}", release.new_version)
            };

            let tag_message = format!("Release {} v{}", release.name, release.new_version);

            match ctx
                .git_provider()
                .create_tag(ctx.project_root(), &tag_name, &tag_message)
            {
                Ok(tag_info) => {
                    created_tag_names.push(tag_name);
                    tags.push(TagResult {
                        name: tag_info.name,
                        target_sha: tag_info.target_sha,
                    });
                }
                Err(e) => {
                    for created_tag in &created_tag_names {
                        let _ = ctx
                            .git_provider()
                            .delete_tag(ctx.project_root(), created_tag);
                    }
                    return Err(e);
                }
            }
        }

        input.tags_created = tags;
        Ok(input)
    }

    fn compensate(&self, ctx: &Self::Context, input: Self::Input) -> Result<(), Self::Error> {
        if !input.should_create_tags {
            return Ok(());
        }

        let use_prefix = self.use_crate_prefix || self.tag_format == TagFormat::CratePrefixed;

        let mut failed_tags = Vec::new();
        for release in &input.planned_releases {
            let tag_name = if use_prefix {
                format!("{}@v{}", release.name, release.new_version)
            } else {
                format!("v{}", release.new_version)
            };
            if ctx
                .git_provider()
                .delete_tag(ctx.project_root(), &tag_name)
                .is_err()
            {
                failed_tags.push(tag_name);
            }
        }

        if failed_tags.is_empty() {
            Ok(())
        } else {
            Err(OperationError::TagDeletionFailed { failed_tags })
        }
    }

    fn compensation_description(&self) -> String {
        "delete the created tags".to_string()
    }
}

pub struct UpdateReleaseStateStep<G, M, RW, S, C> {
    _marker: PhantomData<(G, M, RW, S, C)>,
}

impl<G, M, RW, S, C> UpdateReleaseStateStep<G, M, RW, S, C> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<G, M, RW, S, C> Default for UpdateReleaseStateStep<G, M, RW, S, C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<G, M, RW, S, C> SagaStep for UpdateReleaseStateStep<G, M, RW, S, C>
where
    G: GitProvider + Send + Sync,
    M: ManifestWriter + Send + Sync,
    RW: ChangesetReader + ChangesetWriter + Send + Sync,
    S: ReleaseStateIO + Send + Sync,
    C: ChangelogWriter + Send + Sync,
{
    type Input = ReleaseSagaData;
    type Output = ReleaseSagaData;
    type Context = ReleaseSagaContext<G, M, RW, S, C>;
    type Error = OperationError;

    fn name(&self) -> &'static str {
        "update_release_state"
    }

    fn execute(
        &self,
        ctx: &Self::Context,
        input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        if let Some(update) = &input.prerelease_state_update {
            ctx.release_state_io()
                .save_prerelease_state(&input.changeset_dir, &update.new_state)?;
        }

        if let Some(update) = &input.graduation_state_update {
            ctx.release_state_io()
                .save_graduation_state(&input.changeset_dir, &update.new_state)?;
        }

        Ok(input)
    }

    fn compensate(&self, ctx: &Self::Context, input: Self::Input) -> Result<(), Self::Error> {
        if let Some(update) = &input.prerelease_state_update {
            if let Some(original) = &update.original {
                ctx.release_state_io()
                    .save_prerelease_state(&input.changeset_dir, original)?;
            }
        }

        if let Some(update) = &input.graduation_state_update {
            if let Some(original) = &update.original {
                ctx.release_state_io()
                    .save_graduation_state(&input.changeset_dir, original)?;
            }
        }

        Ok(())
    }

    fn compensation_description(&self) -> String {
        "restore original release state files".to_string()
    }
}

pub struct RestoreChangelogsStep<G, M, RW, S, C> {
    _marker: PhantomData<(G, M, RW, S, C)>,
}

impl<G, M, RW, S, C> RestoreChangelogsStep<G, M, RW, S, C> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<G, M, RW, S, C> Default for RestoreChangelogsStep<G, M, RW, S, C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<G, M, RW, S, C> SagaStep for RestoreChangelogsStep<G, M, RW, S, C>
where
    G: GitProvider + Send + Sync,
    M: ManifestWriter + Send + Sync,
    RW: ChangesetReader + ChangesetWriter + Send + Sync,
    S: ReleaseStateIO + Send + Sync,
    C: ChangelogWriter + Send + Sync,
{
    type Input = ReleaseSagaData;
    type Output = ReleaseSagaData;
    type Context = ReleaseSagaContext<G, M, RW, S, C>;
    type Error = OperationError;

    fn name(&self) -> &'static str {
        "restore_changelogs"
    }

    fn execute(
        &self,
        _ctx: &Self::Context,
        input: Self::Input,
    ) -> Result<Self::Output, Self::Error> {
        Ok(input)
    }

    fn compensate(&self, ctx: &Self::Context, input: Self::Input) -> Result<(), Self::Error> {
        for backup in &input.changelog_backups {
            if backup.file_existed {
                if let Some(content) = &backup.original_content {
                    ctx.changelog_writer()
                        .restore_changelog(&backup.path, content)?;
                }
            } else {
                ctx.changelog_writer().delete_changelog(&backup.path)?;
            }
        }
        Ok(())
    }

    fn compensation_description(&self) -> String {
        "restore original changelog files".to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use changeset_core::BumpType;
    use changeset_saga::SagaStep;
    use indexmap::IndexMap;

    use super::*;
    use crate::mocks::{
        MockChangelogWriter, MockChangesetReader, MockGitProvider, MockManifestWriter,
        MockReleaseStateIO,
    };
    use crate::operations::release::saga_data::SagaReleaseOptions;
    use crate::types::PackageVersion;

    type TestContext = ReleaseSagaContext<
        MockGitProvider,
        MockManifestWriter,
        MockChangesetReader,
        MockReleaseStateIO,
        MockChangelogWriter,
    >;

    fn make_test_context(
        git_provider: Arc<MockGitProvider>,
        manifest_writer: Arc<MockManifestWriter>,
        changeset_rw: Arc<MockChangesetReader>,
        release_state_io: Arc<MockReleaseStateIO>,
    ) -> TestContext {
        ReleaseSagaContext::new(
            PathBuf::from("/mock/project"),
            git_provider,
            manifest_writer,
            changeset_rw,
            release_state_io,
            Arc::new(MockChangelogWriter::new()),
        )
    }

    fn make_test_release(name: &str, current: &str, new: &str) -> PackageVersion {
        PackageVersion {
            name: name.to_string(),
            current_version: current.parse().expect("valid version"),
            new_version: new.parse().expect("valid version"),
            bump_type: BumpType::Patch,
        }
    }

    fn make_test_data() -> ReleaseSagaData {
        let mut package_paths = IndexMap::new();
        package_paths.insert(
            "pkg-a".to_string(),
            PathBuf::from("/mock/project/crates/pkg-a"),
        );

        ReleaseSagaData::new(
            PathBuf::from("/mock/project/.changeset"),
            PathBuf::from("/mock/project/Cargo.toml"),
            vec![make_test_release("pkg-a", "1.0.0", "1.0.1")],
            package_paths,
            Vec::new(),
            Vec::new(),
        )
        .with_options(SagaReleaseOptions {
            is_prerelease_release: false,
            is_graduating: false,
            is_prerelease_graduation: false,
            should_commit: true,
            should_create_tags: true,
            should_delete_changesets: true,
        })
    }

    #[test]
    fn write_manifest_versions_updates_manifests() -> anyhow::Result<()> {
        let manifest_writer = Arc::new(MockManifestWriter::new());
        let ctx = make_test_context(
            Arc::new(MockGitProvider::new()),
            Arc::clone(&manifest_writer),
            Arc::new(MockChangesetReader::new()),
            Arc::new(MockReleaseStateIO::new()),
        );

        let step: WriteManifestVersionsStep<
            MockGitProvider,
            MockManifestWriter,
            MockChangesetReader,
            MockReleaseStateIO,
            MockChangelogWriter,
        > = WriteManifestVersionsStep::new();
        let input = make_test_data();

        let result = SagaStep::execute(&step, &ctx, input)?;

        assert_eq!(result.manifest_updates.len(), 1);
        assert!(result.manifest_updates[0].written);
        assert_eq!(manifest_writer.written_versions().len(), 1);

        Ok(())
    }

    #[test]
    fn write_manifest_versions_compensate_restores_versions() -> anyhow::Result<()> {
        let manifest_writer = Arc::new(MockManifestWriter::new());
        let ctx = make_test_context(
            Arc::new(MockGitProvider::new()),
            Arc::clone(&manifest_writer),
            Arc::new(MockChangesetReader::new()),
            Arc::new(MockReleaseStateIO::new()),
        );

        let step: WriteManifestVersionsStep<
            MockGitProvider,
            MockManifestWriter,
            MockChangesetReader,
            MockReleaseStateIO,
            MockChangelogWriter,
        > = WriteManifestVersionsStep::new();
        let mut input = make_test_data();
        input.manifest_updates.push(ManifestUpdate {
            manifest_path: PathBuf::from("/mock/project/crates/pkg-a/Cargo.toml"),
            old_version: "1.0.0".parse()?,
            new_version: "1.0.1".parse()?,
            written: true,
        });

        SagaStep::compensate(&step, &ctx, input)?;

        let written = manifest_writer.written_versions();
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].1.to_string(), "1.0.0");

        Ok(())
    }

    #[test]
    fn update_dependency_versions_records_updates() -> anyhow::Result<()> {
        let manifest_writer =
            Arc::new(MockManifestWriter::new().with_dependency_updates_returning_true());
        let ctx = make_test_context(
            Arc::new(MockGitProvider::new()),
            Arc::clone(&manifest_writer),
            Arc::new(MockChangesetReader::new()),
            Arc::new(MockReleaseStateIO::new()),
        );

        let step: UpdateDependencyVersionsStep<
            MockGitProvider,
            MockManifestWriter,
            MockChangesetReader,
            MockReleaseStateIO,
            MockChangelogWriter,
        > = UpdateDependencyVersionsStep::new();
        let input = make_test_data();

        let result = SagaStep::execute(&step, &ctx, input)?;

        assert!(
            !result.dependency_updates.is_empty(),
            "dependency updates should be recorded"
        );
        let updates = manifest_writer.dependency_version_updates();
        assert!(
            !updates.is_empty(),
            "mock should record update_dependency_version calls"
        );
        let (_, dep_name, version) = &updates[0];
        assert_eq!(dep_name, "pkg-a");
        assert_eq!(version.to_string(), "1.0.1");

        Ok(())
    }

    #[test]
    fn update_dependency_versions_compensate_restores_versions() -> anyhow::Result<()> {
        let manifest_writer =
            Arc::new(MockManifestWriter::new().with_dependency_updates_returning_true());
        let ctx = make_test_context(
            Arc::new(MockGitProvider::new()),
            Arc::clone(&manifest_writer),
            Arc::new(MockChangesetReader::new()),
            Arc::new(MockReleaseStateIO::new()),
        );

        let step: UpdateDependencyVersionsStep<
            MockGitProvider,
            MockManifestWriter,
            MockChangesetReader,
            MockReleaseStateIO,
            MockChangelogWriter,
        > = UpdateDependencyVersionsStep::new();
        let input = make_test_data();

        SagaStep::compensate(&step, &ctx, input)?;

        let updates = manifest_writer.dependency_version_updates();
        assert!(
            !updates.is_empty(),
            "compensate should call update_dependency_version"
        );
        let (_, dep_name, version) = &updates[0];
        assert_eq!(dep_name, "pkg-a");
        assert_eq!(
            version.to_string(),
            "1.0.0",
            "compensate should restore the current (old) version"
        );

        Ok(())
    }

    #[test]
    fn stage_files_includes_dependency_update_files() -> anyhow::Result<()> {
        let git_provider = Arc::new(MockGitProvider::new());
        let ctx = make_test_context(
            Arc::clone(&git_provider),
            Arc::new(MockManifestWriter::new()),
            Arc::new(MockChangesetReader::new()),
            Arc::new(MockReleaseStateIO::new()),
        );

        let step: StageFilesStep<
            MockGitProvider,
            MockManifestWriter,
            MockChangesetReader,
            MockReleaseStateIO,
            MockChangelogWriter,
        > = StageFilesStep::new();
        let mut input = make_test_data();
        input.dependency_updates.push(DependencyUpdate {
            manifest_path: PathBuf::from("/mock/project/crates/pkg-b/Cargo.toml"),
            dependency_name: "pkg-a".to_string(),
            old_version: "1.0.0".parse()?,
            new_version: "1.0.1".parse()?,
        });

        let result = SagaStep::execute(&step, &ctx, input)?;

        assert!(result.files_were_staged);
        assert!(
            result
                .staged_files
                .contains(&PathBuf::from("/mock/project/crates/pkg-b/Cargo.toml")),
            "dependency update manifest should be staged"
        );

        Ok(())
    }

    #[test]
    fn stage_files_deduplicates_manifest_and_dependency_updates() -> anyhow::Result<()> {
        let git_provider = Arc::new(MockGitProvider::new());
        let ctx = make_test_context(
            Arc::clone(&git_provider),
            Arc::new(MockManifestWriter::new()),
            Arc::new(MockChangesetReader::new()),
            Arc::new(MockReleaseStateIO::new()),
        );

        let step: StageFilesStep<
            MockGitProvider,
            MockManifestWriter,
            MockChangesetReader,
            MockReleaseStateIO,
            MockChangelogWriter,
        > = StageFilesStep::new();
        let mut input = make_test_data();
        let shared_path = PathBuf::from("/mock/project/crates/pkg-a/Cargo.toml");
        input.manifest_updates.push(ManifestUpdate {
            manifest_path: shared_path.clone(),
            old_version: "1.0.0".parse()?,
            new_version: "1.0.1".parse()?,
            written: true,
        });
        input.dependency_updates.push(DependencyUpdate {
            manifest_path: shared_path.clone(),
            dependency_name: "pkg-a".to_string(),
            old_version: "1.0.0".parse()?,
            new_version: "1.0.1".parse()?,
        });

        let result = SagaStep::execute(&step, &ctx, input)?;

        let count = result
            .staged_files
            .iter()
            .filter(|p| **p == shared_path)
            .count();
        assert_eq!(count, 1, "duplicate paths should be deduplicated");

        Ok(())
    }

    #[test]
    fn stage_files_stages_manifest_and_changelog_files() -> anyhow::Result<()> {
        let git_provider = Arc::new(MockGitProvider::new());
        let ctx = make_test_context(
            Arc::clone(&git_provider),
            Arc::new(MockManifestWriter::new()),
            Arc::new(MockChangesetReader::new()),
            Arc::new(MockReleaseStateIO::new()),
        );

        let step: StageFilesStep<
            MockGitProvider,
            MockManifestWriter,
            MockChangesetReader,
            MockReleaseStateIO,
            MockChangelogWriter,
        > = StageFilesStep::new();
        let mut input = make_test_data();
        input.manifest_updates.push(ManifestUpdate {
            manifest_path: PathBuf::from("/mock/project/crates/pkg-a/Cargo.toml"),
            old_version: "1.0.0".parse()?,
            new_version: "1.0.1".parse()?,
            written: true,
        });

        let result = SagaStep::execute(&step, &ctx, input)?;

        assert!(result.files_were_staged);
        assert!(!result.staged_files.is_empty());
        assert!(!git_provider.staged_files().is_empty());

        Ok(())
    }

    #[test]
    fn create_commit_creates_commit_when_files_staged() -> anyhow::Result<()> {
        let git_provider = Arc::new(MockGitProvider::new());
        let ctx = make_test_context(
            Arc::clone(&git_provider),
            Arc::new(MockManifestWriter::new()),
            Arc::new(MockChangesetReader::new()),
            Arc::new(MockReleaseStateIO::new()),
        );

        let step: CreateCommitStep<
            MockGitProvider,
            MockManifestWriter,
            MockChangesetReader,
            MockReleaseStateIO,
            MockChangelogWriter,
        > = CreateCommitStep::new("Release {new-version}".to_string(), false);
        let mut input = make_test_data();
        input.files_were_staged = true;

        let result = SagaStep::execute(&step, &ctx, input)?;

        assert!(result.commit_result.is_some());
        assert_eq!(git_provider.commits().len(), 1);

        Ok(())
    }

    #[test]
    fn create_commit_compensate_resets_to_parent() -> anyhow::Result<()> {
        let git_provider = Arc::new(MockGitProvider::new());
        let ctx = make_test_context(
            Arc::clone(&git_provider),
            Arc::new(MockManifestWriter::new()),
            Arc::new(MockChangesetReader::new()),
            Arc::new(MockReleaseStateIO::new()),
        );

        let step: CreateCommitStep<
            MockGitProvider,
            MockManifestWriter,
            MockChangesetReader,
            MockReleaseStateIO,
            MockChangelogWriter,
        > = CreateCommitStep::new("Release {new-version}".to_string(), false);
        let mut input = make_test_data();
        input.commit_result = Some(CommitResult {
            sha: "abc123".to_string(),
            message: "Release".to_string(),
        });

        SagaStep::compensate(&step, &ctx, input)?;

        assert_eq!(git_provider.reset_count(), 1);

        Ok(())
    }

    #[test]
    fn create_tags_creates_tags_when_commit_exists() -> anyhow::Result<()> {
        let git_provider = Arc::new(MockGitProvider::new());
        let ctx = make_test_context(
            Arc::clone(&git_provider),
            Arc::new(MockManifestWriter::new()),
            Arc::new(MockChangesetReader::new()),
            Arc::new(MockReleaseStateIO::new()),
        );

        let step: CreateTagsStep<
            MockGitProvider,
            MockManifestWriter,
            MockChangesetReader,
            MockReleaseStateIO,
            MockChangelogWriter,
        > = CreateTagsStep::new(TagFormat::VersionOnly, false);
        let mut input = make_test_data();
        input.commit_result = Some(CommitResult {
            sha: "abc123".to_string(),
            message: "Release".to_string(),
        });

        let result = SagaStep::execute(&step, &ctx, input)?;

        assert_eq!(result.tags_created.len(), 1);
        assert_eq!(git_provider.tags_created().len(), 1);

        Ok(())
    }

    #[test]
    fn create_tags_compensate_deletes_tags() -> anyhow::Result<()> {
        let git_provider = Arc::new(MockGitProvider::new());
        let ctx = make_test_context(
            Arc::clone(&git_provider),
            Arc::new(MockManifestWriter::new()),
            Arc::new(MockChangesetReader::new()),
            Arc::new(MockReleaseStateIO::new()),
        );

        let step: CreateTagsStep<
            MockGitProvider,
            MockManifestWriter,
            MockChangesetReader,
            MockReleaseStateIO,
            MockChangelogWriter,
        > = CreateTagsStep::new(TagFormat::VersionOnly, false);
        let mut input = make_test_data();
        input.tags_created = vec![TagResult {
            name: "v1.0.1".to_string(),
            target_sha: "abc123".to_string(),
        }];

        SagaStep::compensate(&step, &ctx, input)?;

        assert_eq!(git_provider.deleted_tags().len(), 1);
        assert_eq!(git_provider.deleted_tags()[0], "v1.0.1");

        Ok(())
    }

    #[test]
    fn create_tags_partial_failure_deletes_first_tag_when_second_fails() {
        let git_provider = Arc::new(MockGitProvider::new());
        git_provider.set_fail_on_create_tag_nth(1);

        let ctx = make_test_context(
            Arc::clone(&git_provider),
            Arc::new(MockManifestWriter::new()),
            Arc::new(MockChangesetReader::new()),
            Arc::new(MockReleaseStateIO::new()),
        );

        let step: CreateTagsStep<
            MockGitProvider,
            MockManifestWriter,
            MockChangesetReader,
            MockReleaseStateIO,
            MockChangelogWriter,
        > = CreateTagsStep::new(TagFormat::CratePrefixed, true);

        let mut package_paths = IndexMap::new();
        package_paths.insert(
            "pkg-a".to_string(),
            PathBuf::from("/mock/project/crates/pkg-a"),
        );
        package_paths.insert(
            "pkg-b".to_string(),
            PathBuf::from("/mock/project/crates/pkg-b"),
        );

        let mut input = ReleaseSagaData::new(
            PathBuf::from("/mock/project/.changeset"),
            PathBuf::from("/mock/project/Cargo.toml"),
            vec![
                make_test_release("pkg-a", "1.0.0", "1.0.1"),
                make_test_release("pkg-b", "2.0.0", "2.0.1"),
            ],
            package_paths,
            Vec::new(),
            Vec::new(),
        )
        .with_options(SagaReleaseOptions {
            is_prerelease_release: false,
            is_graduating: false,
            is_prerelease_graduation: false,
            should_commit: true,
            should_create_tags: true,
            should_delete_changesets: true,
        });
        input.commit_result = Some(CommitResult {
            sha: "abc123".to_string(),
            message: "Release".to_string(),
        });

        let result = SagaStep::execute(&step, &ctx, input);
        assert!(result.is_err(), "should fail on second tag creation");

        assert_eq!(
            git_provider.tags_created().len(),
            1,
            "first tag should have been created before failure"
        );
        assert_eq!(
            git_provider.deleted_tags().len(),
            1,
            "first tag should be deleted during cleanup"
        );
        assert_eq!(
            git_provider.deleted_tags()[0],
            "pkg-a@v1.0.1",
            "deleted tag should be the first tag that was created"
        );
    }

    #[test]
    fn create_tags_partial_failure_deletes_multiple_tags_when_third_fails() {
        let git_provider = Arc::new(MockGitProvider::new());
        git_provider.set_fail_on_create_tag_nth(2);

        let ctx = make_test_context(
            Arc::clone(&git_provider),
            Arc::new(MockManifestWriter::new()),
            Arc::new(MockChangesetReader::new()),
            Arc::new(MockReleaseStateIO::new()),
        );

        let step: CreateTagsStep<
            MockGitProvider,
            MockManifestWriter,
            MockChangesetReader,
            MockReleaseStateIO,
            MockChangelogWriter,
        > = CreateTagsStep::new(TagFormat::CratePrefixed, true);

        let mut package_paths = IndexMap::new();
        package_paths.insert(
            "pkg-a".to_string(),
            PathBuf::from("/mock/project/crates/pkg-a"),
        );
        package_paths.insert(
            "pkg-b".to_string(),
            PathBuf::from("/mock/project/crates/pkg-b"),
        );
        package_paths.insert(
            "pkg-c".to_string(),
            PathBuf::from("/mock/project/crates/pkg-c"),
        );

        let mut input = ReleaseSagaData::new(
            PathBuf::from("/mock/project/.changeset"),
            PathBuf::from("/mock/project/Cargo.toml"),
            vec![
                make_test_release("pkg-a", "1.0.0", "1.0.1"),
                make_test_release("pkg-b", "2.0.0", "2.0.1"),
                make_test_release("pkg-c", "3.0.0", "3.0.1"),
            ],
            package_paths,
            Vec::new(),
            Vec::new(),
        )
        .with_options(SagaReleaseOptions {
            is_prerelease_release: false,
            is_graduating: false,
            is_prerelease_graduation: false,
            should_commit: true,
            should_create_tags: true,
            should_delete_changesets: true,
        });
        input.commit_result = Some(CommitResult {
            sha: "abc123".to_string(),
            message: "Release".to_string(),
        });

        let result = SagaStep::execute(&step, &ctx, input);
        assert!(result.is_err(), "should fail on third tag creation");

        assert_eq!(
            git_provider.tags_created().len(),
            2,
            "first two tags should have been created before failure"
        );
        assert_eq!(
            git_provider.deleted_tags().len(),
            2,
            "both successful tags should be deleted during cleanup"
        );

        let deleted = git_provider.deleted_tags();
        assert!(
            deleted.contains(&"pkg-a@v1.0.1".to_string()),
            "first tag should be in deleted list"
        );
        assert!(
            deleted.contains(&"pkg-b@v2.0.1".to_string()),
            "second tag should be in deleted list"
        );
    }

    #[allow(clippy::items_after_statements)]
    mod rollback_integration {
        use changeset_core::{ChangeCategory, Changeset, PackageRelease};
        use changeset_saga::SagaBuilder;

        use crate::operations::release::steps::ChangesetFileState;

        use super::*;

        fn make_test_changeset(name: &str) -> Changeset {
            Changeset {
                summary: format!("Fix {name}"),
                releases: vec![PackageRelease {
                    name: name.to_string(),
                    bump_type: BumpType::Patch,
                }],
                category: ChangeCategory::Fixed,
                consumed_for_prerelease: None,
                graduate: false,
            }
        }

        #[test]
        fn rollback_restores_manifests_when_commit_fails() {
            let git_provider = Arc::new(MockGitProvider::new());
            let manifest_writer = Arc::new(MockManifestWriter::new());
            let changeset_rw = Arc::new(MockChangesetReader::new().with_changeset(
                PathBuf::from("/mock/project/.changeset/changesets/fix.md"),
                make_test_changeset("pkg-a"),
            ));

            git_provider.set_fail_on_commit(true);

            let ctx = make_test_context(
                Arc::clone(&git_provider),
                Arc::clone(&manifest_writer),
                Arc::clone(&changeset_rw),
                Arc::new(MockReleaseStateIO::new()),
            );

            type WriteManifests = WriteManifestVersionsStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;
            type Stage = StageFilesStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;
            type Commit = CreateCommitStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;

            let saga = SagaBuilder::new()
                .first_step(WriteManifests::new())
                .then(Stage::new())
                .then(Commit::new("Release {new-version}".to_string(), false))
                .build();

            let input = make_test_data();

            let result = saga.execute(&ctx, input);
            assert!(result.is_err(), "saga should fail on commit");

            let written_versions = manifest_writer.written_versions();
            assert!(
                written_versions.len() >= 2,
                "should have written new version then restored old version"
            );

            let last_write = written_versions.last().expect("should have writes");
            assert_eq!(
                last_write.1.to_string(),
                "1.0.0",
                "last write should restore original version"
            );
        }

        #[test]
        fn rollback_resets_commit_when_tag_creation_fails() {
            let git_provider = Arc::new(MockGitProvider::new());
            let manifest_writer = Arc::new(MockManifestWriter::new());
            let changeset_rw = Arc::new(MockChangesetReader::new().with_changeset(
                PathBuf::from("/mock/project/.changeset/changesets/fix.md"),
                make_test_changeset("pkg-a"),
            ));

            git_provider.set_fail_on_create_tag(true);

            let ctx = make_test_context(
                Arc::clone(&git_provider),
                Arc::clone(&manifest_writer),
                Arc::clone(&changeset_rw),
                Arc::new(MockReleaseStateIO::new()),
            );

            type WriteManifests = WriteManifestVersionsStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;
            type Stage = StageFilesStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;
            type Commit = CreateCommitStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;
            type Tags = CreateTagsStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;

            let saga = SagaBuilder::new()
                .first_step(WriteManifests::new())
                .then(Stage::new())
                .then(Commit::new("Release {new-version}".to_string(), false))
                .then(Tags::new(TagFormat::VersionOnly, false))
                .build();

            let input = make_test_data();

            let result = saga.execute(&ctx, input);
            assert!(result.is_err(), "saga should fail on tag creation");

            assert_eq!(git_provider.reset_count(), 1, "should have reset commit");
        }

        #[test]
        fn rollback_restores_deleted_changesets_on_failure() {
            let git_provider = Arc::new(MockGitProvider::new());
            let manifest_writer = Arc::new(MockManifestWriter::new());
            let changeset_path = PathBuf::from("/mock/project/.changeset/changesets/fix.md");
            let original_changeset = make_test_changeset("pkg-a");

            let changeset_rw = Arc::new(
                MockChangesetReader::new()
                    .with_changeset(changeset_path.clone(), original_changeset.clone()),
            );

            git_provider.set_fail_on_commit(true);

            let ctx = make_test_context(
                Arc::clone(&git_provider),
                Arc::clone(&manifest_writer),
                Arc::clone(&changeset_rw),
                Arc::new(MockReleaseStateIO::new()),
            );

            type WriteManifests = WriteManifestVersionsStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;
            type DeleteChangesets = DeleteChangesetFilesStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;
            type Stage = StageFilesStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;
            type Commit = CreateCommitStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;

            let saga = SagaBuilder::new()
                .first_step(WriteManifests::new())
                .then(DeleteChangesets::new())
                .then(Stage::new())
                .then(Commit::new("Release {new-version}".to_string(), false))
                .build();

            let input = make_test_data();

            let result = saga.execute(&ctx, input);
            assert!(result.is_err(), "saga should fail on commit");

            let restored = changeset_rw.read_changeset(&changeset_path);
            assert!(
                restored.is_ok(),
                "changeset file should be restored after rollback"
            );
            let restored = restored.expect("changeset should exist");
            assert_eq!(restored.summary, original_changeset.summary);
            assert_eq!(restored.category, original_changeset.category);
        }

        #[test]
        fn rollback_restores_workspace_version_on_failure() {
            let git_provider = Arc::new(MockGitProvider::new());
            let manifest_writer = Arc::new(
                MockManifestWriter::new()
                    .with_workspace_version("1.0.0".parse().expect("valid version")),
            );
            let changeset_rw = Arc::new(MockChangesetReader::new());

            git_provider.set_fail_on_stage_files(true);

            let ctx = make_test_context(
                Arc::clone(&git_provider),
                Arc::clone(&manifest_writer),
                Arc::clone(&changeset_rw),
                Arc::new(MockReleaseStateIO::new()),
            );

            type RemoveWorkspace = RemoveWorkspaceVersionStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;
            type Stage = StageFilesStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;

            let saga = SagaBuilder::new()
                .first_step(RemoveWorkspace::new())
                .then(Stage::new())
                .build();

            let mut input = make_test_data();
            input.inherited_packages = vec!["pkg-a".to_string()];
            input.original_workspace_version = Some("1.0.0".parse().expect("valid version"));

            let result = saga.execute(&ctx, input);
            assert!(result.is_err(), "saga should fail on stage");

            assert_eq!(
                manifest_writer.get_workspace_version(),
                Some("1.0.0".parse().expect("valid version")),
                "workspace version should be restored"
            );
            assert!(
                !manifest_writer.workspace_version_removed(),
                "workspace version should be marked as not removed"
            );
        }

        #[test]
        fn rollback_restores_consumed_status_on_graduation_failure() {
            let git_provider = Arc::new(MockGitProvider::new());
            let manifest_writer = Arc::new(MockManifestWriter::new());
            let changeset_path = PathBuf::from("/mock/project/.changeset/changesets/fix.md");

            let mut consumed_changeset = make_test_changeset("pkg-a");
            consumed_changeset.consumed_for_prerelease = Some("1.0.1-alpha.1".to_string());

            let changeset_rw = Arc::new(MockChangesetReader::new().with_consumed_changeset(
                changeset_path.clone(),
                consumed_changeset.clone(),
                "1.0.1-alpha.1".to_string(),
            ));

            git_provider.set_fail_on_commit(true);

            let ctx = make_test_context(
                Arc::clone(&git_provider),
                Arc::clone(&manifest_writer),
                Arc::clone(&changeset_rw),
                Arc::new(MockReleaseStateIO::new()),
            );

            type WriteManifests = WriteManifestVersionsStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;
            type ClearConsumed = ClearChangesetsConsumedStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;
            type Stage = StageFilesStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;
            type Commit = CreateCommitStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;

            let saga = SagaBuilder::new()
                .first_step(WriteManifests::new())
                .then(ClearConsumed::new())
                .then(Stage::new())
                .then(Commit::new("Release {new-version}".to_string(), false))
                .build();

            let mut input = make_test_data();
            input.is_graduating = true;
            input.consumed_files_cleared = vec![ChangesetFileState {
                path: changeset_path.clone(),
                original_consumed_status: Some("1.0.1-alpha.1".to_string()),
                backup: None,
            }];

            let result = saga.execute(&ctx, input);
            assert!(result.is_err(), "saga should fail on commit");

            let status = changeset_rw.get_consumed_status(&changeset_path);
            assert_eq!(
                status,
                Some("1.0.1-alpha.1".to_string()),
                "consumed status should be restored after rollback"
            );
        }

        #[test]
        fn rollback_restores_prerelease_consumed_marking_on_failure() {
            let git_provider = Arc::new(MockGitProvider::new());
            let manifest_writer = Arc::new(MockManifestWriter::new());
            let changeset_path = PathBuf::from("/mock/project/.changeset/changesets/fix.md");

            let changeset_rw = Arc::new(
                MockChangesetReader::new()
                    .with_changeset(changeset_path.clone(), make_test_changeset("pkg-a")),
            );

            git_provider.set_fail_on_commit(true);

            let ctx = make_test_context(
                Arc::clone(&git_provider),
                Arc::clone(&manifest_writer),
                Arc::clone(&changeset_rw),
                Arc::new(MockReleaseStateIO::new()),
            );

            type WriteManifests = WriteManifestVersionsStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;
            type MarkConsumed = MarkChangesetsConsumedStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;
            type Stage = StageFilesStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;
            type Commit = CreateCommitStep<
                MockGitProvider,
                MockManifestWriter,
                MockChangesetReader,
                MockReleaseStateIO,
                MockChangelogWriter,
            >;

            let saga = SagaBuilder::new()
                .first_step(WriteManifests::new())
                .then(MarkConsumed::new())
                .then(Stage::new())
                .then(Commit::new("Release {new-version}".to_string(), false))
                .build();

            let mut input = make_test_data();
            input.is_prerelease_release = true;
            input.changeset_files = vec![ChangesetFileState {
                path: changeset_path.clone(),
                original_consumed_status: None,
                backup: None,
            }];

            let result = saga.execute(&ctx, input);
            assert!(result.is_err(), "saga should fail on commit");

            let status = changeset_rw.get_consumed_status(&changeset_path);
            assert!(
                status.is_none(),
                "consumed status should be cleared after rollback (was not consumed before)"
            );
        }
    }
}
