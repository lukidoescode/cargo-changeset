use std::path::{Path, PathBuf};

use changeset_git::{FileChange, FileStatus};
use changeset_project::map_files_to_packages;

use crate::Result;
use crate::traits::{ChangesetReader, GitProvider, ProjectProvider};
use crate::verification::rules::{CoverageRule, DeletedChangesetsRule};
use crate::verification::{VerificationContext, VerificationEngine, VerificationResult};

pub struct VerifyInput {
    pub base: String,
    pub head: Option<String>,
    pub allow_deleted_changesets: bool,
}

#[derive(Debug)]
pub enum VerifyOutcome {
    Success(VerificationResult),
    NoChanges,
    NoPackagesAffected {
        project_file_count: usize,
        ignored_file_count: usize,
    },
    Failed(VerificationResult),
}

pub struct VerifyOperation<P, G, R> {
    project_provider: P,
    git_provider: G,
    changeset_reader: R,
}

impl<P, G, R> VerifyOperation<P, G, R>
where
    P: ProjectProvider,
    G: GitProvider,
    R: ChangesetReader,
{
    pub fn new(project_provider: P, git_provider: G, changeset_reader: R) -> Self {
        Self {
            project_provider,
            git_provider,
            changeset_reader,
        }
    }

    /// # Errors
    ///
    /// Returns an error if the project cannot be discovered, git operations fail,
    /// or changeset files cannot be read.
    pub fn execute(&self, start_path: &Path, input: &VerifyInput) -> Result<VerifyOutcome> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, package_configs) = self.project_provider.load_configs(&project)?;
        let changeset_dir = root_config.changeset_dir();

        let head_ref = input.head.as_deref().unwrap_or("HEAD");
        let changed_files =
            self.git_provider
                .changed_files(&project.root, &input.base, head_ref)?;

        let (changeset_changes, code_changes): (Vec<_>, Vec<_>) = changed_files
            .into_iter()
            .partition(|change| change.path.starts_with(changeset_dir));

        let deleted_changesets = extract_deleted_changesets(&changeset_changes, changeset_dir);
        let changeset_files = extract_active_changesets(&changeset_changes);

        let changed_paths: Vec<PathBuf> =
            code_changes.into_iter().map(|change| change.path).collect();

        let has_deleted_changesets = !deleted_changesets.is_empty();
        let has_code_changes = !changed_paths.is_empty();

        if !has_code_changes && !has_deleted_changesets {
            return Ok(VerifyOutcome::NoChanges);
        }

        let mapping = if has_code_changes {
            Some(map_files_to_packages(
                &project,
                &changed_paths,
                &root_config,
                &package_configs,
            ))
        } else {
            None
        };

        let affected_packages = mapping.as_ref().map_or(
            Vec::new(),
            changeset_project::FileMapping::affected_packages,
        );

        if affected_packages.is_empty() && !has_deleted_changesets {
            let (project_file_count, ignored_file_count) = mapping
                .as_ref()
                .map_or((0, 0), |m| (m.project_files.len(), m.ignored_files.len()));
            return Ok(VerifyOutcome::NoPackagesAffected {
                project_file_count,
                ignored_file_count,
            });
        }

        let context = build_context(mapping.as_ref(), changeset_files, deleted_changesets);

        let deleted_rule = DeletedChangesetsRule::new(input.allow_deleted_changesets);
        let coverage_rule = CoverageRule::new(&self.changeset_reader);

        let mut engine = VerificationEngine::new();
        engine.add_rule(&deleted_rule);
        engine.add_rule(&coverage_rule);

        let result = engine.verify(&context)?;

        if result.is_success() {
            Ok(VerifyOutcome::Success(result))
        } else {
            Ok(VerifyOutcome::Failed(result))
        }
    }
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "md")
}

fn extract_deleted_changesets(changes: &[FileChange], changeset_dir: &Path) -> Vec<PathBuf> {
    changes
        .iter()
        .filter_map(|change| match change.status {
            FileStatus::Deleted if is_markdown_file(&change.path) => Some(change.path.clone()),
            FileStatus::Renamed => change
                .old_path
                .as_ref()
                .filter(|old| old.starts_with(changeset_dir) && is_markdown_file(old))
                .cloned(),
            _ => None,
        })
        .collect()
}

fn extract_active_changesets(changes: &[FileChange]) -> Vec<PathBuf> {
    changes
        .iter()
        .filter(|change| {
            is_markdown_file(&change.path)
                && matches!(
                    change.status,
                    FileStatus::Added | FileStatus::Modified | FileStatus::Renamed
                )
        })
        .map(|change| change.path.clone())
        .collect()
}

fn build_context(
    mapping: Option<&changeset_project::FileMapping>,
    changeset_files: Vec<PathBuf>,
    deleted_changesets: Vec<PathBuf>,
) -> VerificationContext {
    match mapping {
        Some(m) => VerificationContext {
            affected_packages: m.affected_packages().into_iter().cloned().collect(),
            changeset_files,
            deleted_changesets,
            project_files: m.project_files.clone(),
            ignored_files: m.ignored_files.clone(),
        },
        None => VerificationContext {
            affected_packages: Vec::new(),
            changeset_files,
            deleted_changesets,
            project_files: Vec::new(),
            ignored_files: Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mocks::{MockChangesetReader, MockGitProvider, MockProjectProvider};
    use changeset_core::BumpType;
    use changeset_git::FileStatus;

    #[test]
    fn returns_no_changes_when_no_files_changed() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let git_provider = MockGitProvider::new();
        let changeset_reader = MockChangesetReader::new();

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInput {
            base: "main".to_string(),
            head: None,
            allow_deleted_changesets: false,
        };

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("VerifyOperation failed when no files changed");

        assert!(matches!(result, VerifyOutcome::NoChanges));
    }

    #[test]
    fn returns_success_when_changeset_covers_affected_package() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");

        let git_provider = MockGitProvider::new().with_changed_files(vec![
            FileChange {
                path: PathBuf::from(".changeset/changesets/test.md"),
                status: FileStatus::Added,
                old_path: None,
            },
            FileChange {
                path: PathBuf::from("src/lib.rs"),
                status: FileStatus::Modified,
                old_path: None,
            },
        ]);

        let changeset = crate::mocks::make_changeset("my-crate", BumpType::Patch, "Fix bug");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/test.md"), changeset);

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInput {
            base: "main".to_string(),
            head: None,
            allow_deleted_changesets: false,
        };

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("VerifyOperation failed when changeset covers affected package");

        match result {
            VerifyOutcome::Success(verification_result) => {
                assert!(verification_result.uncovered_packages.is_empty());
                assert!(verification_result.covered_packages.contains("my-crate"));
            }
            other => panic!("Expected VerifyOutcome::Success, got {other:?}"),
        }
    }

    #[test]
    fn returns_failed_when_package_not_covered() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");

        let git_provider = MockGitProvider::new().with_changed_files(vec![FileChange {
            path: PathBuf::from("src/lib.rs"),
            status: FileStatus::Modified,
            old_path: None,
        }]);

        let changeset_reader = MockChangesetReader::new();

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInput {
            base: "main".to_string(),
            head: None,
            allow_deleted_changesets: false,
        };

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("VerifyOperation failed unexpectedly when package not covered");

        match result {
            VerifyOutcome::Failed(verification_result) => {
                assert!(!verification_result.uncovered_packages.is_empty());
            }
            other => panic!("Expected VerifyOutcome::Failed, got {other:?}"),
        }
    }

    #[test]
    fn extract_deleted_changesets_identifies_deleted_md_files() {
        let changes = vec![
            FileChange {
                path: PathBuf::from(".changeset/changesets/old.md"),
                status: FileStatus::Deleted,
                old_path: None,
            },
            FileChange {
                path: PathBuf::from("src/main.rs"),
                status: FileStatus::Deleted,
                old_path: None,
            },
        ];

        let deleted = extract_deleted_changesets(&changes, Path::new(".changeset"));

        assert_eq!(deleted.len(), 1);
        assert_eq!(deleted[0], PathBuf::from(".changeset/changesets/old.md"));
    }

    #[test]
    fn extract_active_changesets_identifies_added_and_modified() {
        let changes = vec![
            FileChange {
                path: PathBuf::from(".changeset/changesets/new.md"),
                status: FileStatus::Added,
                old_path: None,
            },
            FileChange {
                path: PathBuf::from(".changeset/changesets/updated.md"),
                status: FileStatus::Modified,
                old_path: None,
            },
            FileChange {
                path: PathBuf::from(".changeset/changesets/deleted.md"),
                status: FileStatus::Deleted,
                old_path: None,
            },
        ];

        let active = extract_active_changesets(&changes);

        assert_eq!(active.len(), 2);
        assert!(active.contains(&PathBuf::from(".changeset/changesets/new.md")));
        assert!(active.contains(&PathBuf::from(".changeset/changesets/updated.md")));
    }

    #[test]
    fn is_markdown_file_recognizes_md_extension() {
        assert!(is_markdown_file(Path::new("test.md")));
        assert!(is_markdown_file(Path::new("path/to/file.md")));
        assert!(!is_markdown_file(Path::new("test.rs")));
        assert!(!is_markdown_file(Path::new("test")));
    }
}
