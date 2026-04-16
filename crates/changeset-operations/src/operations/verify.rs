use std::collections::HashSet;
use std::path::{Path, PathBuf};

use changeset_core::{NoneBumpBehavior, PackageInfo};
use changeset_git::{FileChange, FileStatus};
use changeset_project::{compile_influence_patterns, map_files_to_all_packages};

use derive_builder::Builder;
use gset::Getset;

use crate::Result;
use crate::traits::{
    ChangesetReader, DependencyGraphProvider, GitDiffProvider, GitStatusProvider,
    GitWorkdirDiffProvider, ProjectProvider,
};
use crate::verification::rules::{CoverageRule, DeletedChangesetsRule, NoneBumpDisallowedRule};
use crate::verification::{VerificationContext, VerificationEngine, VerificationResult};

#[derive(Builder, Default, Getset)]
#[builder(default)]
pub struct VerifyInput {
    #[getset(get, vis = "pub")]
    base: String,
    #[getset(get_as_ref, vis = "pub", ty = "Option<&String>")]
    head: Option<String>,
    #[getset(get_copy, vis = "pub")]
    allow_deleted_changesets: bool,
    #[getset(get_copy, vis = "pub")]
    exclude_dependents: bool,
    #[getset(get_copy, vis = "pub")]
    ignore_dirty: bool,
}

#[must_use]
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

#[must_use]
#[derive(Debug, Getset)]
pub struct VerifyResult {
    #[getset(get_copy, vis = "pub")]
    is_dirty: bool,
    #[getset(get, vis = "pub")]
    outcome: VerifyOutcome,
}

impl VerifyResult {
    pub(crate) fn new(is_dirty: bool, outcome: VerifyOutcome) -> Self {
        Self { is_dirty, outcome }
    }
}

struct CollectedChanges {
    is_dirty: bool,
    changeset_files: Vec<PathBuf>,
    deleted_changesets: Vec<PathBuf>,
    code_changes: Vec<PathBuf>,
}

pub struct VerifyOperation<P, G, R> {
    project_provider: P,
    git_provider: G,
    changeset_reader: R,
}

impl<P, G, R> VerifyOperation<P, G, R>
where
    P: ProjectProvider + DependencyGraphProvider,
    G: GitDiffProvider + GitWorkdirDiffProvider + GitStatusProvider,
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
    /// Returns an error if project discovery, git operations, or changeset reads fail.
    pub fn execute(&self, start_path: &Path, input: &VerifyInput) -> Result<VerifyResult> {
        let project = self.project_provider.discover_project(start_path)?;
        let (root_config, package_configs) = self.project_provider.load_configs(&project)?;

        let additional_packages = super::discover_additional_packages_if_workspace(
            &self.project_provider,
            &project,
            &root_config,
        )?;

        let all_package_names: HashSet<String> = project
            .packages()
            .iter()
            .map(|p| p.name().clone())
            .chain(additional_packages.iter().map(|p| p.name().clone()))
            .collect();
        changeset_project::validate_version_tracking_dependencies(
            &root_config,
            &package_configs,
            &all_package_names,
        )?;

        let additional_with_patterns: Vec<_> = if additional_packages.is_empty() {
            Vec::new()
        } else {
            let influence_patterns = compile_influence_patterns(root_config.additional_packages())?;
            additional_packages
                .into_iter()
                .zip(influence_patterns)
                .collect()
        };

        let collected = self.collect_changes(&project, root_config.changeset_dir(), input)?;
        let is_dirty = collected.is_dirty;
        let changeset_files = collected.changeset_files;
        let deleted_changesets = collected.deleted_changesets;
        let changed_paths = collected.code_changes;

        let has_code_changes = !changed_paths.is_empty();
        let has_deleted_changesets = !deleted_changesets.is_empty();

        if !has_code_changes && !has_deleted_changesets {
            return Ok(VerifyResult::new(is_dirty, VerifyOutcome::NoChanges));
        }

        let mapping = has_code_changes.then(|| {
            map_files_to_all_packages(
                &project,
                &changed_paths,
                &root_config,
                &package_configs,
                &additional_with_patterns,
            )
        });

        let (affected_packages, transitive_dependents) =
            self.resolve_affected_packages(&project, mapping.as_ref(), input)?;

        if affected_packages.is_empty() && !has_deleted_changesets {
            let (project_file_count, ignored_file_count) = mapping
                .as_ref()
                .map_or((0, 0), |m| (m.project().len(), m.ignored().len()));
            return Ok(VerifyResult::new(
                is_dirty,
                VerifyOutcome::NoPackagesAffected {
                    project_file_count,
                    ignored_file_count,
                },
            ));
        }

        let context = build_context(
            mapping.as_ref(),
            affected_packages,
            transitive_dependents,
            changeset_files,
            deleted_changesets,
        );

        let result =
            self.run_verification(&context, input.allow_deleted_changesets(), &root_config)?;

        let outcome = if result.is_success() {
            VerifyOutcome::Success(result)
        } else {
            VerifyOutcome::Failed(result)
        };

        Ok(VerifyResult::new(is_dirty, outcome))
    }

    fn collect_changes(
        &self,
        project: &changeset_project::CargoProject,
        changeset_dir: &Path,
        input: &VerifyInput,
    ) -> Result<CollectedChanges> {
        let working_tree_dirty = if input.ignore_dirty() {
            false
        } else {
            !self.git_provider.is_working_tree_clean(project.root())?
        };

        let changed_files = if working_tree_dirty {
            self.git_provider.uncommitted_changes(project.root())?
        } else {
            let head_ref = input.head().map_or("HEAD", String::as_str);
            self.git_provider
                .changed_files(project.root(), input.base(), head_ref)?
        };

        let is_dirty = working_tree_dirty && !changed_files.is_empty();

        let (changeset_changes, code_changes): (Vec<_>, Vec<_>) = changed_files
            .into_iter()
            .partition(|change| change.path().starts_with(changeset_dir));

        let deleted_changesets = extract_deleted_changesets(&changeset_changes, changeset_dir);
        let changeset_files = extract_active_changesets(&changeset_changes);
        let code_changes = code_changes
            .into_iter()
            .map(|change| change.path().clone())
            .collect();

        Ok(CollectedChanges {
            is_dirty,
            changeset_files,
            deleted_changesets,
            code_changes,
        })
    }

    fn resolve_affected_packages(
        &self,
        project: &changeset_project::CargoProject,
        mapping: Option<&changeset_project::FileMapping>,
        input: &VerifyInput,
    ) -> Result<(Vec<PackageInfo>, HashSet<String>)> {
        let mut affected_packages: Vec<PackageInfo> = mapping.map_or(Vec::new(), |m| {
            m.affected_packages().into_iter().cloned().collect()
        });

        let mut transitive_dependents: HashSet<String> = HashSet::new();

        if !input.exclude_dependents()
            && project.packages().len() > 1
            && !affected_packages.is_empty()
        {
            let graph = self.project_provider.build_dependency_graph(project)?;
            let affected_names: Vec<&str> = affected_packages
                .iter()
                .map(|p| p.name().as_str())
                .collect();
            let dependents = graph.transitive_dependents_of_set(&affected_names);

            for pkg in project.packages() {
                if dependents.contains(pkg.name().as_str())
                    && !affected_packages.iter().any(|p| p.name() == pkg.name())
                {
                    transitive_dependents.insert(pkg.name().clone());
                    affected_packages.push(pkg.clone());
                }
            }
        }

        Ok((affected_packages, transitive_dependents))
    }

    fn run_verification(
        &self,
        context: &VerificationContext,
        allow_deleted_changesets: bool,
        root_config: &changeset_project::RootChangesetConfig,
    ) -> Result<crate::verification::VerificationResult> {
        let deleted_rule = DeletedChangesetsRule::new(allow_deleted_changesets);
        let coverage_rule = CoverageRule::new(&self.changeset_reader);
        let none_bump_rule = NoneBumpDisallowedRule::new(&self.changeset_reader);

        let mut engine = VerificationEngine::new();
        engine.add_rule(&deleted_rule);
        engine.add_rule(&coverage_rule);

        if root_config.none_bump_behavior() == NoneBumpBehavior::Disallow {
            engine.add_rule(&none_bump_rule);
        }

        engine.verify(context)
    }
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "md")
}

fn extract_deleted_changesets(changes: &[FileChange], changeset_dir: &Path) -> Vec<PathBuf> {
    changes
        .iter()
        .filter_map(|change| match change.status() {
            FileStatus::Deleted if is_markdown_file(change.path()) => Some(change.path().clone()),
            FileStatus::Renamed => change
                .old_path()
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
            is_markdown_file(change.path())
                && matches!(
                    change.status(),
                    FileStatus::Added
                        | FileStatus::Modified
                        | FileStatus::Renamed
                        | FileStatus::Typechange
                )
        })
        .map(|change| change.path().clone())
        .collect()
}

fn build_context(
    mapping: Option<&changeset_project::FileMapping>,
    affected_packages: Vec<PackageInfo>,
    transitive_dependents: HashSet<String>,
    changeset_files: Vec<PathBuf>,
    deleted_changesets: Vec<PathBuf>,
) -> VerificationContext {
    let (project_files, ignored_files) = mapping.map_or((Vec::new(), Vec::new()), |m| {
        (m.project().clone(), m.ignored().clone())
    });
    VerificationContext::new(
        affected_packages,
        transitive_dependents,
        changeset_files,
        deleted_changesets,
        project_files,
        ignored_files,
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use changeset_core::{BumpType, NoneBumpBehavior};
    use changeset_git::FileStatus;
    use changeset_project::RootChangesetConfig;

    use crate::mocks::{MockChangesetReader, MockGitProvider, MockProjectProvider, make_package};

    #[test]
    fn returns_no_changes_when_no_files_changed() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");
        let git_provider = MockGitProvider::new();
        let changeset_reader = MockChangesetReader::new();

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("VerifyOperation failed when no files changed");

        assert!(matches!(result.outcome(), VerifyOutcome::NoChanges));
    }

    #[test]
    fn returns_success_when_changeset_covers_affected_package() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");

        let git_provider = MockGitProvider::new().with_changed_files(vec![
            FileChange::new(
                PathBuf::from(".changeset/changesets/test.md"),
                FileStatus::Added,
            ),
            FileChange::new(PathBuf::from("src/lib.rs"), FileStatus::Modified),
        ]);

        let changeset = crate::mocks::make_changeset("my-crate", BumpType::Patch, "Fix bug");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/test.md"), changeset);

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("VerifyOperation failed when changeset covers affected package");

        match result.outcome() {
            VerifyOutcome::Success(verification_result) => {
                assert!(verification_result.uncovered_packages().is_empty());
                assert!(verification_result.covered_packages().contains("my-crate"));
            }
            other => panic!("Expected VerifyOutcome::Success, got {other:?}"),
        }
    }

    #[test]
    fn returns_failed_when_package_not_covered() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");

        let git_provider = MockGitProvider::new().with_changed_files(vec![FileChange::new(
            PathBuf::from("src/lib.rs"),
            FileStatus::Modified,
        )]);

        let changeset_reader = MockChangesetReader::new();

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("VerifyOperation failed unexpectedly when package not covered");

        match result.outcome() {
            VerifyOutcome::Failed(verification_result) => {
                assert!(!verification_result.uncovered_packages().is_empty());
            }
            other => panic!("Expected VerifyOutcome::Failed, got {other:?}"),
        }
    }

    #[test]
    fn extract_deleted_changesets_identifies_deleted_md_files() {
        let changes = vec![
            FileChange::new(
                PathBuf::from(".changeset/changesets/old.md"),
                FileStatus::Deleted,
            ),
            FileChange::new(PathBuf::from("src/main.rs"), FileStatus::Deleted),
        ];

        let deleted = extract_deleted_changesets(&changes, Path::new(".changeset"));

        assert_eq!(deleted.len(), 1);
        assert_eq!(deleted[0], PathBuf::from(".changeset/changesets/old.md"));
    }

    #[test]
    fn extract_active_changesets_identifies_added_and_modified() {
        let changes = vec![
            FileChange::new(
                PathBuf::from(".changeset/changesets/new.md"),
                FileStatus::Added,
            ),
            FileChange::new(
                PathBuf::from(".changeset/changesets/updated.md"),
                FileStatus::Modified,
            ),
            FileChange::new(
                PathBuf::from(".changeset/changesets/deleted.md"),
                FileStatus::Deleted,
            ),
        ];

        let active = extract_active_changesets(&changes);

        assert_eq!(active.len(), 2);
        assert!(active.contains(&PathBuf::from(".changeset/changesets/new.md")));
        assert!(active.contains(&PathBuf::from(".changeset/changesets/updated.md")));
    }

    #[test]
    fn returns_success_when_changeset_has_none_bump_type() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");

        let git_provider = MockGitProvider::new().with_changed_files(vec![
            FileChange::new(
                PathBuf::from(".changeset/changesets/internal.md"),
                FileStatus::Added,
            ),
            FileChange::new(PathBuf::from("src/lib.rs"), FileStatus::Modified),
        ]);

        let changeset =
            crate::mocks::make_changeset("my-crate", BumpType::None, "Internal refactoring");
        let changeset_reader = MockChangesetReader::new().with_changeset(
            PathBuf::from(".changeset/changesets/internal.md"),
            changeset,
        );

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("VerifyOperation failed when changeset has None bump type");

        match result.outcome() {
            VerifyOutcome::Success(verification_result) => {
                assert!(verification_result.uncovered_packages().is_empty());
                assert!(verification_result.covered_packages().contains("my-crate"));
            }
            other => panic!("Expected VerifyOutcome::Success, got {other:?}"),
        }
    }

    #[test]
    fn is_markdown_file_recognizes_md_extension() {
        assert!(is_markdown_file(Path::new("test.md")));
        assert!(is_markdown_file(Path::new("path/to/file.md")));
        assert!(!is_markdown_file(Path::new("test.rs")));
        assert!(!is_markdown_file(Path::new("test")));
    }

    #[test]
    fn fails_when_transitive_dependent_not_covered() {
        let project_provider =
            MockProjectProvider::workspace(vec![("core", "1.0.0"), ("app", "1.0.0")])
                .with_dependency_edges(vec![("app", "core")]);

        let git_provider = MockGitProvider::new().with_changed_files(vec![
            FileChange::new(
                PathBuf::from(".changeset/changesets/fix.md"),
                FileStatus::Added,
            ),
            FileChange::new(
                PathBuf::from("crates/core/src/lib.rs"),
                FileStatus::Modified,
            ),
        ]);

        let changeset = crate::mocks::make_changeset("core", BumpType::Patch, "Fix core bug");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("operation should not error");

        match result.outcome() {
            VerifyOutcome::Failed(verification_result) => {
                assert!(
                    verification_result
                        .uncovered_packages()
                        .iter()
                        .any(|p| p.name() == "app"),
                    "app should be uncovered as a transitive dependent of core"
                );
            }
            other => panic!("Expected VerifyOutcome::Failed, got {other:?}"),
        }
    }

    #[test]
    fn succeeds_when_transitive_dependent_is_covered() {
        let project_provider =
            MockProjectProvider::workspace(vec![("core", "1.0.0"), ("app", "1.0.0")])
                .with_dependency_edges(vec![("app", "core")]);

        let git_provider = MockGitProvider::new().with_changed_files(vec![
            FileChange::new(
                PathBuf::from(".changeset/changesets/fix.md"),
                FileStatus::Added,
            ),
            FileChange::new(
                PathBuf::from("crates/core/src/lib.rs"),
                FileStatus::Modified,
            ),
        ]);

        let changeset = changeset_core::Changeset::new(
            "Fix core bug".to_string(),
            vec![
                changeset_core::PackageRelease::new("core".to_string(), BumpType::Patch),
                changeset_core::PackageRelease::new("app".to_string(), BumpType::Patch),
            ],
            changeset_core::ChangeCategory::Changed,
        );
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("operation should not error");

        match result.outcome() {
            VerifyOutcome::Success(verification_result) => {
                assert!(verification_result.covered_packages().contains("core"));
                assert!(verification_result.covered_packages().contains("app"));
                assert!(verification_result.uncovered_packages().is_empty());
            }
            other => panic!("Expected VerifyOutcome::Success, got {other:?}"),
        }
    }

    #[test]
    fn exclude_dependents_skips_transitive_expansion() {
        let project_provider =
            MockProjectProvider::workspace(vec![("core", "1.0.0"), ("app", "1.0.0")])
                .with_dependency_edges(vec![("app", "core")]);

        let git_provider = MockGitProvider::new().with_changed_files(vec![
            FileChange::new(
                PathBuf::from(".changeset/changesets/fix.md"),
                FileStatus::Added,
            ),
            FileChange::new(
                PathBuf::from("crates/core/src/lib.rs"),
                FileStatus::Modified,
            ),
        ]);

        let changeset = crate::mocks::make_changeset("core", BumpType::Patch, "Fix core bug");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .exclude_dependents(true)
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("operation should not error");

        match result.outcome() {
            VerifyOutcome::Success(verification_result) => {
                assert!(verification_result.covered_packages().contains("core"));
                assert!(verification_result.uncovered_packages().is_empty());
            }
            other => panic!("Expected VerifyOutcome::Success, got {other:?}"),
        }
    }

    #[test]
    fn single_package_skips_dependency_computation() {
        let project_provider = MockProjectProvider::single_package("solo", "1.0.0");

        let git_provider = MockGitProvider::new().with_changed_files(vec![
            FileChange::new(
                PathBuf::from(".changeset/changesets/fix.md"),
                FileStatus::Added,
            ),
            FileChange::new(PathBuf::from("src/lib.rs"), FileStatus::Modified),
        ]);

        let changeset = crate::mocks::make_changeset("solo", BumpType::Patch, "Fix bug");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("operation should not error");

        match result.outcome() {
            VerifyOutcome::Success(verification_result) => {
                assert!(verification_result.covered_packages().contains("solo"));
                assert!(verification_result.uncovered_packages().is_empty());
            }
            other => panic!("Expected VerifyOutcome::Success, got {other:?}"),
        }
    }

    #[test]
    fn dirty_tree_uses_uncommitted_changes() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");

        let uncommitted = vec![
            FileChange::new(
                PathBuf::from(".changeset/changesets/local.md"),
                FileStatus::Added,
            ),
            FileChange::new(PathBuf::from("src/lib.rs"), FileStatus::Modified),
        ];

        let git_provider = MockGitProvider::new()
            .is_clean(false)
            .with_uncommitted_changes(uncommitted);

        let changeset = crate::mocks::make_changeset("my-crate", BumpType::Patch, "Local fix");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/local.md"), changeset);

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("operation should not error");

        assert!(result.is_dirty());
        match result.outcome() {
            VerifyOutcome::Success(verification_result) => {
                assert!(verification_result.covered_packages().contains("my-crate"));
            }
            other => panic!("Expected VerifyOutcome::Success, got {other:?}"),
        }
    }

    #[test]
    fn clean_tree_uses_branch_diff() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");

        let git_provider = MockGitProvider::new()
            .is_clean(true)
            .with_changed_files(vec![
                FileChange::new(
                    PathBuf::from(".changeset/changesets/test.md"),
                    FileStatus::Added,
                ),
                FileChange::new(PathBuf::from("src/lib.rs"), FileStatus::Modified),
            ]);

        let changeset = crate::mocks::make_changeset("my-crate", BumpType::Patch, "Fix bug");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/test.md"), changeset);

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("operation should not error");

        assert!(!result.is_dirty());
        match result.outcome() {
            VerifyOutcome::Success(verification_result) => {
                assert!(verification_result.covered_packages().contains("my-crate"));
            }
            other => panic!("Expected VerifyOutcome::Success, got {other:?}"),
        }
    }

    #[test]
    fn dirty_tree_with_empty_uncommitted_changes_yields_no_changes() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");

        let git_provider = MockGitProvider::new()
            .is_clean(false)
            .with_uncommitted_changes(vec![]);

        let changeset_reader = MockChangesetReader::new();

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("operation should not error");

        assert!(!result.is_dirty());
        assert!(matches!(result.outcome(), VerifyOutcome::NoChanges));
    }

    #[test]
    fn dirty_tree_with_only_changeset_file_changes_reports_no_code_changes() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");

        let git_provider = MockGitProvider::new()
            .is_clean(false)
            .with_uncommitted_changes(vec![FileChange::new(
                PathBuf::from(".changeset/changesets/local.md"),
                FileStatus::Added,
            )]);

        let changeset_reader = MockChangesetReader::new();

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("operation should not error");

        assert!(result.is_dirty());
        assert!(matches!(result.outcome(), VerifyOutcome::NoChanges));
    }

    #[test]
    fn is_working_tree_clean_error_propagates() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");

        let git_provider = Arc::new(MockGitProvider::new());
        git_provider.set_fail_on_is_clean(true);

        let changeset_reader = MockChangesetReader::new();

        let operation = VerifyOperation::new(
            project_provider,
            Arc::clone(&git_provider),
            changeset_reader,
        );

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation.execute(Path::new("/any"), &input);

        assert!(result.is_err());
    }

    #[test]
    fn dirty_tree_fails_when_package_not_covered() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");

        let git_provider = MockGitProvider::new()
            .is_clean(false)
            .with_uncommitted_changes(vec![FileChange::new(
                PathBuf::from("src/lib.rs"),
                FileStatus::Modified,
            )]);

        let changeset_reader = MockChangesetReader::new();

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("operation should not error");

        assert!(result.is_dirty());
        match result.outcome() {
            VerifyOutcome::Failed(verification_result) => {
                assert!(!verification_result.uncovered_packages().is_empty());
            }
            other => panic!("Expected VerifyOutcome::Failed, got {other:?}"),
        }
    }

    #[test]
    fn dirty_tree_fails_when_transitive_dependent_not_covered() {
        let project_provider =
            MockProjectProvider::workspace(vec![("core", "1.0.0"), ("app", "1.0.0")])
                .with_dependency_edges(vec![("app", "core")]);

        let git_provider = MockGitProvider::new()
            .is_clean(false)
            .with_uncommitted_changes(vec![
                FileChange::new(
                    PathBuf::from(".changeset/changesets/fix.md"),
                    FileStatus::Added,
                ),
                FileChange::new(
                    PathBuf::from("crates/core/src/lib.rs"),
                    FileStatus::Modified,
                ),
            ]);

        let changeset = crate::mocks::make_changeset("core", BumpType::Patch, "Fix core bug");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("operation should not error");

        assert!(result.is_dirty());
        match result.outcome() {
            VerifyOutcome::Failed(verification_result) => {
                assert!(
                    verification_result
                        .uncovered_packages()
                        .iter()
                        .any(|p| p.name() == "app"),
                    "app should be uncovered as a transitive dependent of core"
                );
            }
            other => panic!("Expected VerifyOutcome::Failed, got {other:?}"),
        }
    }

    #[test]
    fn ignore_dirty_skips_dirty_check() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0");

        let git_provider = Arc::new(
            MockGitProvider::new()
                .with_changed_files(vec![
                    FileChange::new(
                        PathBuf::from(".changeset/changesets/test.md"),
                        FileStatus::Added,
                    ),
                    FileChange::new(PathBuf::from("src/lib.rs"), FileStatus::Modified),
                ])
                .with_uncommitted_changes(vec![]),
        );
        git_provider.set_fail_on_is_clean(true);

        let changeset = crate::mocks::make_changeset("my-crate", BumpType::Patch, "Fix bug");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/test.md"), changeset);

        let operation = VerifyOperation::new(
            project_provider,
            Arc::clone(&git_provider),
            changeset_reader,
        );

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .ignore_dirty(true)
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("operation should not error");

        assert!(!result.is_dirty());
        match result.outcome() {
            VerifyOutcome::Success(verification_result) => {
                assert!(verification_result.covered_packages().contains("my-crate"));
            }
            other => panic!("Expected VerifyOutcome::Success, got {other:?}"),
        }
    }

    #[test]
    fn disallow_rejects_none_bump_in_verify() {
        let root_config =
            RootChangesetConfig::default().with_none_bump_behavior(NoneBumpBehavior::Disallow);
        let project_provider =
            MockProjectProvider::single_package("my-crate", "1.0.0").with_root_config(root_config);

        let git_provider = MockGitProvider::new().with_changed_files(vec![
            FileChange::new(
                PathBuf::from(".changeset/changesets/internal.md"),
                FileStatus::Added,
            ),
            FileChange::new(PathBuf::from("src/lib.rs"), FileStatus::Modified),
        ]);

        let changeset =
            crate::mocks::make_changeset("my-crate", BumpType::None, "Internal refactoring");
        let changeset_reader = MockChangesetReader::new().with_changeset(
            PathBuf::from(".changeset/changesets/internal.md"),
            changeset,
        );

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("operation should not error");

        match result.outcome() {
            VerifyOutcome::Failed(verification_result) => {
                assert!(
                    verification_result
                        .none_bump_violations()
                        .contains(&"my-crate".to_string()),
                    "my-crate should be in none_bump_violations"
                );
            }
            other => panic!("Expected VerifyOutcome::Failed, got {other:?}"),
        }
    }

    #[test]
    fn allow_permits_none_bump_in_verify() {
        let root_config =
            RootChangesetConfig::default().with_none_bump_behavior(NoneBumpBehavior::Allow);
        let project_provider =
            MockProjectProvider::single_package("my-crate", "1.0.0").with_root_config(root_config);

        let git_provider = MockGitProvider::new().with_changed_files(vec![
            FileChange::new(
                PathBuf::from(".changeset/changesets/internal.md"),
                FileStatus::Added,
            ),
            FileChange::new(PathBuf::from("src/lib.rs"), FileStatus::Modified),
        ]);

        let changeset =
            crate::mocks::make_changeset("my-crate", BumpType::None, "Internal refactoring");
        let changeset_reader = MockChangesetReader::new().with_changeset(
            PathBuf::from(".changeset/changesets/internal.md"),
            changeset,
        );

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("operation should not error");

        match result.outcome() {
            VerifyOutcome::Success(verification_result) => {
                assert!(verification_result.none_bump_violations().is_empty());
                assert!(verification_result.covered_packages().contains("my-crate"));
            }
            other => panic!("Expected VerifyOutcome::Success, got {other:?}"),
        }
    }

    fn make_declaration(
        name: &str,
        influence: &[&str],
    ) -> changeset_core::AdditionalPackageDeclaration {
        let influence_json: String = influence
            .iter()
            .map(|s| format!("\"{s}\""))
            .collect::<Vec<_>>()
            .join(",");
        let json = format!(
            r#"{{"name":"{name}","path":"{name}","influence":[{influence_json}],"manifest":{{"file-path":"/{name}/manifest.yaml","format":"yaml","version-field-path":"version"}}}}"#
        );
        serde_json::from_str(&json).expect("valid declaration JSON")
    }

    #[test]
    fn detects_additional_package_changes_via_influence_globs() {
        let decl = make_declaration("helm-chart", &["charts/**"]);
        let root_config =
            RootChangesetConfig::default().with_additional_packages(vec![decl.clone()]);
        let helm_chart = make_package("helm-chart", "1.0.0");
        let project_provider = MockProjectProvider::workspace(vec![("my-crate", "1.0.0")])
            .with_root_config(root_config)
            .with_additional_packages(vec![helm_chart]);

        let git_provider = MockGitProvider::new().with_changed_files(vec![FileChange::new(
            PathBuf::from("charts/values.yaml"),
            FileStatus::Modified,
        )]);

        let changeset_reader = MockChangesetReader::new();

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("operation should not error");

        match result.outcome() {
            VerifyOutcome::Failed(verification_result) => {
                assert!(
                    verification_result
                        .uncovered_packages()
                        .iter()
                        .any(|p| p.name() == "helm-chart"),
                    "helm-chart should be uncovered"
                );
            }
            other => panic!("Expected VerifyOutcome::Failed, got {other:?}"),
        }
    }

    #[test]
    fn succeeds_when_additional_package_covered_by_changeset() {
        let decl = make_declaration("helm-chart", &["charts/**"]);
        let root_config = RootChangesetConfig::default().with_additional_packages(vec![decl]);
        let helm_chart = make_package("helm-chart", "1.0.0");
        let project_provider = MockProjectProvider::workspace(vec![("my-crate", "1.0.0")])
            .with_root_config(root_config)
            .with_additional_packages(vec![helm_chart]);

        let git_provider = MockGitProvider::new().with_changed_files(vec![
            FileChange::new(
                PathBuf::from(".changeset/changesets/chart.md"),
                FileStatus::Added,
            ),
            FileChange::new(PathBuf::from("charts/values.yaml"), FileStatus::Modified),
        ]);

        let changeset = crate::mocks::make_changeset("helm-chart", BumpType::Patch, "Update chart");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/chart.md"), changeset);

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("operation should not error");

        match result.outcome() {
            VerifyOutcome::Success(verification_result) => {
                assert!(
                    verification_result
                        .covered_packages()
                        .contains("helm-chart")
                );
                assert!(verification_result.uncovered_packages().is_empty());
            }
            other => panic!("Expected VerifyOutcome::Success, got {other:?}"),
        }
    }

    #[test]
    fn files_outside_influence_globs_not_matched_to_additional_packages() {
        let decl = make_declaration("helm-chart", &["charts/**"]);
        let root_config = RootChangesetConfig::default().with_additional_packages(vec![decl]);
        let helm_chart = make_package("helm-chart", "1.0.0");
        let project_provider = MockProjectProvider::workspace(vec![("my-crate", "1.0.0")])
            .with_root_config(root_config)
            .with_additional_packages(vec![helm_chart]);

        let git_provider = MockGitProvider::new().with_changed_files(vec![FileChange::new(
            PathBuf::from("docs/README.md"),
            FileStatus::Modified,
        )]);

        let changeset_reader = MockChangesetReader::new();

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("operation should not error");

        assert!(matches!(
            result.outcome(),
            VerifyOutcome::NoPackagesAffected { .. }
        ));
    }

    #[test]
    fn mixed_rust_and_additional_packages_both_detected_and_covered() {
        let decl = make_declaration("helm-chart", &["charts/**"]);
        let root_config = RootChangesetConfig::default().with_additional_packages(vec![decl]);
        let helm_chart = make_package("helm-chart", "1.0.0");
        let project_provider = MockProjectProvider::workspace(vec![("core", "1.0.0")])
            .with_root_config(root_config)
            .with_additional_packages(vec![helm_chart]);

        let git_provider = MockGitProvider::new().with_changed_files(vec![
            FileChange::new(
                PathBuf::from(".changeset/changesets/both.md"),
                FileStatus::Added,
            ),
            FileChange::new(
                PathBuf::from("crates/core/src/lib.rs"),
                FileStatus::Modified,
            ),
            FileChange::new(PathBuf::from("charts/values.yaml"), FileStatus::Modified),
        ]);

        let changeset = changeset_core::Changeset::new(
            "Update both".to_string(),
            vec![
                changeset_core::PackageRelease::new("core".to_string(), BumpType::Patch),
                changeset_core::PackageRelease::new("helm-chart".to_string(), BumpType::Patch),
            ],
            changeset_core::ChangeCategory::Changed,
        );
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/both.md"), changeset);

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("operation should not error");

        match result.outcome() {
            VerifyOutcome::Success(verification_result) => {
                assert!(verification_result.covered_packages().contains("core"));
                assert!(
                    verification_result
                        .covered_packages()
                        .contains("helm-chart")
                );
                assert!(verification_result.uncovered_packages().is_empty());
            }
            other => panic!("Expected VerifyOutcome::Success, got {other:?}"),
        }
    }

    #[test]
    fn additional_packages_not_expanded_as_transitive_dependents() {
        let decl = make_declaration("helm-chart", &["charts/**"]);
        let root_config = RootChangesetConfig::default().with_additional_packages(vec![decl]);
        let helm_chart = make_package("helm-chart", "1.0.0");
        let project_provider =
            MockProjectProvider::workspace(vec![("core", "1.0.0"), ("app", "1.0.0")])
                .with_dependency_edges(vec![("app", "core")])
                .with_root_config(root_config)
                .with_additional_packages(vec![helm_chart]);

        let git_provider = MockGitProvider::new().with_changed_files(vec![
            FileChange::new(
                PathBuf::from(".changeset/changesets/fix.md"),
                FileStatus::Added,
            ),
            FileChange::new(
                PathBuf::from("crates/core/src/lib.rs"),
                FileStatus::Modified,
            ),
        ]);

        let changeset = changeset_core::Changeset::new(
            "Fix core and app".to_string(),
            vec![
                changeset_core::PackageRelease::new("core".to_string(), BumpType::Patch),
                changeset_core::PackageRelease::new("app".to_string(), BumpType::Patch),
            ],
            changeset_core::ChangeCategory::Changed,
        );
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("operation should not error");

        match result.outcome() {
            VerifyOutcome::Success(verification_result) => {
                assert!(verification_result.covered_packages().contains("core"));
                assert!(verification_result.covered_packages().contains("app"));
                assert!(
                    !verification_result
                        .uncovered_packages()
                        .iter()
                        .any(|p| p.name() == "helm-chart"),
                    "helm-chart should not appear as uncovered — it had no changes"
                );
            }
            other => panic!("Expected VerifyOutcome::Success, got {other:?}"),
        }
    }

    #[test]
    fn additional_package_uncovered_while_rust_packages_covered() {
        let decl = make_declaration("helm-chart", &["charts/**"]);
        let root_config = RootChangesetConfig::default().with_additional_packages(vec![decl]);
        let helm_chart = make_package("helm-chart", "1.0.0");
        let project_provider = MockProjectProvider::workspace(vec![("core", "1.0.0")])
            .with_root_config(root_config)
            .with_additional_packages(vec![helm_chart]);

        let git_provider = MockGitProvider::new().with_changed_files(vec![
            FileChange::new(
                PathBuf::from(".changeset/changesets/fix.md"),
                FileStatus::Added,
            ),
            FileChange::new(
                PathBuf::from("crates/core/src/lib.rs"),
                FileStatus::Modified,
            ),
            FileChange::new(PathBuf::from("charts/values.yaml"), FileStatus::Modified),
        ]);

        let changeset = crate::mocks::make_changeset("core", BumpType::Patch, "Fix core");
        let changeset_reader = MockChangesetReader::new()
            .with_changeset(PathBuf::from(".changeset/changesets/fix.md"), changeset);

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("operation should not error");

        match result.outcome() {
            VerifyOutcome::Failed(verification_result) => {
                assert!(
                    verification_result
                        .uncovered_packages()
                        .iter()
                        .any(|p| p.name() == "helm-chart"),
                    "helm-chart should be uncovered"
                );
                assert!(
                    !verification_result
                        .uncovered_packages()
                        .iter()
                        .any(|p| p.name() == "core"),
                    "core should be covered"
                );
            }
            other => panic!("Expected VerifyOutcome::Failed, got {other:?}"),
        }
    }

    #[test]
    fn promote_to_patch_permits_none_bump_in_verify() {
        let root_config = RootChangesetConfig::default()
            .with_none_bump_behavior(NoneBumpBehavior::PromoteToPatch);
        let project_provider =
            MockProjectProvider::single_package("my-crate", "1.0.0").with_root_config(root_config);

        let git_provider = MockGitProvider::new().with_changed_files(vec![
            FileChange::new(
                PathBuf::from(".changeset/changesets/internal.md"),
                FileStatus::Added,
            ),
            FileChange::new(PathBuf::from("src/lib.rs"), FileStatus::Modified),
        ]);

        let changeset =
            crate::mocks::make_changeset("my-crate", BumpType::None, "Internal refactoring");
        let changeset_reader = MockChangesetReader::new().with_changeset(
            PathBuf::from(".changeset/changesets/internal.md"),
            changeset,
        );

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("operation should not error");

        match result.outcome() {
            VerifyOutcome::Success(verification_result) => {
                assert!(verification_result.none_bump_violations().is_empty());
                assert!(verification_result.covered_packages().contains("my-crate"));
            }
            other => panic!("Expected VerifyOutcome::Success, got {other:?}"),
        }
    }

    #[test]
    fn single_package_never_calls_discover_additional_packages() {
        let project_provider = MockProjectProvider::single_package("my-crate", "1.0.0")
            .with_fail_on_discover_additional();

        let git_provider = MockGitProvider::new();
        let changeset_reader = MockChangesetReader::new();

        let operation = VerifyOperation::new(project_provider, git_provider, changeset_reader);

        let input = VerifyInputBuilder::default()
            .base("main".to_string())
            .build()
            .expect("all fields have defaults");

        let result = operation
            .execute(Path::new("/any"), &input)
            .expect("single-package should succeed without calling discover_additional_packages");

        assert!(matches!(result.outcome(), VerifyOutcome::NoChanges));
    }
}
