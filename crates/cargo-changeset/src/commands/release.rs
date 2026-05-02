use std::collections::{HashMap, HashSet};
use std::path::Path;

use changeset_core::PrereleaseSpec;
use changeset_operations::OperationError;
use changeset_operations::operations::{
    GitOperationResult, PackageReleaseConfig, ReleaseInputBuilder, ReleaseOperation,
    ReleaseOutcome, ReleaseOutput,
};
use changeset_operations::providers::{
    FileSystemChangelogWriter, FileSystemChangesetIO, FileSystemManifestWriter,
    FileSystemProjectProvider, FileSystemReleaseStateIO, Git2Provider,
};
use changeset_operations::traits::ProjectProvider;
use changeset_version::is_prerelease;

use super::ReleaseArgs;
use crate::error::Result;
use crate::output::CliWriter;

#[derive(Debug, Clone)]
pub(crate) struct ParsedPrereleaseArgs {
    pub(crate) per_package: HashMap<String, PrereleaseSpec>,
    pub(crate) global: Option<PrereleaseSpec>,
}

#[derive(Debug, Clone)]
struct ParsedGraduateArgs {
    packages: HashSet<String>,
    all: bool,
}

pub(super) fn run(args: ReleaseArgs, start_path: &Path, writer: &dyn CliWriter) -> Result<()> {
    let project_provider = FileSystemProjectProvider::new();
    let project = project_provider.discover_project(start_path)?;
    let changeset_io = FileSystemChangesetIO::new(project.root());
    let manifest_writer = FileSystemManifestWriter::new();
    let changelog_writer = FileSystemChangelogWriter::new();
    let git_provider = Git2Provider::new(project.root())?;
    let release_state_io = FileSystemReleaseStateIO::new();

    let parsed_prerelease = parse_prerelease_args(&args.prerelease, &project)?;
    let parsed_graduate = parse_graduate_args(&args.graduate);

    let mut per_package_config = HashMap::new();
    if let Some(ref parsed) = parsed_prerelease {
        for (pkg, spec) in &parsed.per_package {
            per_package_config
                .entry(pkg.clone())
                .or_insert_with(PackageReleaseConfig::default)
                .set_prerelease(spec.clone());
        }
    }
    for pkg in &parsed_graduate.packages {
        per_package_config
            .entry(pkg.clone())
            .or_insert_with(PackageReleaseConfig::default)
            .set_graduate_zero();
    }

    let operation = ReleaseOperation::new(
        project_provider,
        changeset_io,
        manifest_writer,
        changelog_writer,
        git_provider,
        release_state_io,
    );
    let input = ReleaseInputBuilder::default()
        .dry_run(args.dry_run)
        .convert_inherited(args.convert)
        .no_commit(args.no_commit)
        .no_tags(args.no_tags)
        .keep_changesets(args.keep_changesets)
        .force(args.force)
        .per_package_config(per_package_config)
        .global_prerelease(parsed_prerelease.and_then(|p| p.global))
        .graduate_all(parsed_graduate.all)
        .build()
        .expect("all fields have defaults");
    let outcome = operation.execute(start_path, &input)?;

    print_outcome(&outcome, writer);

    Ok(())
}

fn parse_prerelease_args(
    args: &[String],
    project: &changeset_project::CargoProject,
) -> Result<Option<ParsedPrereleaseArgs>> {
    if args.is_empty() {
        return Ok(None);
    }

    let mut per_package = HashMap::new();
    let mut global = None;

    for arg in args {
        if arg.is_empty() {
            // Empty string means infer from existing prerelease
            let has_prerelease = project
                .packages()
                .iter()
                .any(|p| is_prerelease(p.version()));
            if has_prerelease {
                let first_prerelease = project
                    .packages()
                    .iter()
                    .find(|p| is_prerelease(p.version()))
                    .and_then(|p| changeset_version::extract_prerelease_tag(p.version()));

                if let Some(existing_tag) = first_prerelease {
                    global = Some(parse_prerelease_spec(&existing_tag)?);
                    continue;
                }
            }
            return Err(OperationError::PrereleaseTagRequired.into());
        }

        if let Some((crate_name, tag)) = arg.split_once(':') {
            // Per-package format: "crate:tag"
            let spec = parse_prerelease_spec(tag)?;
            per_package.insert(crate_name.to_string(), spec);
        } else {
            // Global format: just "tag"
            global = Some(parse_prerelease_spec(arg)?);
        }
    }

    Ok(Some(ParsedPrereleaseArgs {
        per_package,
        global,
    }))
}

fn parse_prerelease_spec(s: &str) -> Result<PrereleaseSpec> {
    Ok(changeset_operations::parse_prerelease_tag(s)?)
}

fn parse_graduate_args(args: &[String]) -> ParsedGraduateArgs {
    if args.is_empty() {
        return ParsedGraduateArgs {
            packages: HashSet::new(),
            all: false,
        };
    }

    let packages: HashSet<String> = args.iter().filter(|s| !s.is_empty()).cloned().collect();

    let all = args.iter().any(std::string::String::is_empty);

    ParsedGraduateArgs { packages, all }
}

fn print_outcome(outcome: &ReleaseOutcome, writer: &dyn CliWriter) {
    match outcome {
        ReleaseOutcome::NoChangesets => {
            writer.line("No pending changesets to release.");
        }
        ReleaseOutcome::DryRun(output) => {
            writer.line("Dry run - no changes will be made.");
            writer.blank();
            print_release_output(output, writer);
        }
        ReleaseOutcome::Executed(output) => {
            print_release_output(output, writer);
            writer.blank();
            writer.message(crate::output::MessageLevel::Success, "Release complete.");
        }
    }
}

fn print_release_output(output: &ReleaseOutput, writer: &dyn CliWriter) {
    if output.planned_releases().is_empty() {
        writer.line("No packages to release.");
        return;
    }

    writer.heading("Releases:");
    for release in output.planned_releases() {
        let auto_label = if release.auto_bumped() {
            " (dependency update)"
        } else {
            ""
        };
        writer.list_item(&format!(
            "{} {} -> {}{}",
            release.name(),
            release.current_version(),
            release.new_version(),
            auto_label
        ));
    }

    if !output.unchanged_packages().is_empty() {
        writer.blank();
        writer.heading("Unchanged packages:");
        for name in output.unchanged_packages() {
            writer.list_item(name);
        }
    }

    if !output.changelog_updates().is_empty() {
        writer.blank();
        writer.heading("Changelogs updated:");
        for update in output.changelog_updates() {
            let status = if update.created() {
                "created"
            } else {
                "updated"
            };
            writer.list_item(&format!("{} ({})", update.path().display(), status));
        }
    }

    if let Some(git_result) = output.git_result() {
        print_git_result(git_result, writer);
    }

    if !output.changesets_consumed().is_empty() {
        writer.blank();
        writer.line(&format!(
            "Consumed {} changeset file(s)",
            output.changesets_consumed().len()
        ));
    }
}

fn print_git_result(git_result: &GitOperationResult, writer: &dyn CliWriter) {
    if let Some(commit) = git_result.commit() {
        writer.blank();
        writer.line(&format!(
            "Commit created: {}",
            &commit.sha()[..7.min(commit.sha().len())]
        ));
    }

    if !git_result.tags_created().is_empty() {
        writer.blank();
        writer.heading("Tags created:");
        for tag in git_result.tags_created() {
            writer.list_item(tag.name());
        }
    }

    if !git_result.changesets_deleted().is_empty() {
        writer.blank();
        writer.line(&format!(
            "Deleted {} changeset file(s)",
            git_result.changesets_deleted().len()
        ));
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use changeset_core::BumpType;
    use changeset_operations::operations::{
        ChangelogUpdate, CommitResult, GitOperationResult, PackageVersion, ReleaseOutcome,
        ReleaseOutput, TagResult,
    };
    use semver::Version;

    use super::{print_git_result, print_outcome, print_release_output};
    use crate::output::BufferCliWriter;

    fn make_package_version(name: &str, from: &str, to: &str, auto_bumped: bool) -> PackageVersion {
        PackageVersion::new(
            name.to_string(),
            Version::parse(from).expect("valid semver"),
            Version::parse(to).expect("valid semver"),
            BumpType::Minor,
            auto_bumped,
        )
    }

    fn make_release_output_minimal() -> ReleaseOutput {
        ReleaseOutput::new(
            vec![make_package_version("my-crate", "1.0.0", "1.1.0", false)],
            vec![],
            vec![],
            vec![],
            None,
        )
    }

    #[test]
    fn print_outcome_no_changesets() {
        let writer = BufferCliWriter::new();

        print_outcome(&ReleaseOutcome::NoChangesets, &writer);

        let text = writer.stdout_text();
        assert!(text.contains("No pending changesets to release."));
    }

    #[test]
    fn print_outcome_dry_run_shows_prefix() {
        let writer = BufferCliWriter::new();
        let output = make_release_output_minimal();

        print_outcome(&ReleaseOutcome::DryRun(output), &writer);

        let text = writer.stdout_text();
        assert!(text.contains("Dry run - no changes will be made."));
        assert!(text.contains("my-crate"));
    }

    #[test]
    fn print_outcome_executed_shows_complete_message() {
        let writer = BufferCliWriter::new();
        let output = make_release_output_minimal();

        print_outcome(&ReleaseOutcome::Executed(output), &writer);

        let text = writer.stdout_text();
        assert!(text.contains("Release complete."));
        assert!(text.contains("my-crate"));
    }

    #[test]
    fn print_release_output_empty_planned_releases() {
        let writer = BufferCliWriter::new();
        let output = ReleaseOutput::new(vec![], vec![], vec![], vec![], None);

        print_release_output(&output, &writer);

        let text = writer.stdout_text();
        assert!(text.contains("No packages to release."));
    }

    #[test]
    fn print_release_output_shows_version_transitions() {
        let writer = BufferCliWriter::new();
        let output = ReleaseOutput::new(
            vec![
                make_package_version("crate-a", "1.0.0", "1.1.0", false),
                make_package_version("crate-b", "2.0.0", "2.1.0", true),
            ],
            vec![],
            vec![],
            vec![],
            None,
        );

        print_release_output(&output, &writer);

        let text = writer.stdout_text();
        assert!(text.contains("Releases:"));
        assert!(text.contains("crate-a 1.0.0 -> 1.1.0"));
        assert!(text.contains("crate-b 2.0.0 -> 2.1.0 (dependency update)"));
    }

    #[test]
    fn print_release_output_shows_unchanged_packages() {
        let writer = BufferCliWriter::new();
        let output = ReleaseOutput::new(
            vec![make_package_version("crate-a", "1.0.0", "1.1.0", false)],
            vec!["unchanged-crate".to_string()],
            vec![],
            vec![],
            None,
        );

        print_release_output(&output, &writer);

        let text = writer.stdout_text();
        assert!(text.contains("Unchanged packages:"));
        assert!(text.contains("unchanged-crate"));
    }

    #[test]
    fn print_release_output_shows_changelog_updates() {
        let writer = BufferCliWriter::new();
        let output = ReleaseOutput::new(
            vec![make_package_version("crate-a", "1.0.0", "1.1.0", false)],
            vec![],
            vec![],
            vec![
                ChangelogUpdate::new(
                    PathBuf::from("CHANGELOG.md"),
                    None,
                    Version::parse("1.1.0").expect("valid semver"),
                    false,
                ),
                ChangelogUpdate::new(
                    PathBuf::from("crates/b/CHANGELOG.md"),
                    Some("crate-b".to_string()),
                    Version::parse("2.0.0").expect("valid semver"),
                    true,
                ),
            ],
            None,
        );

        print_release_output(&output, &writer);

        let text = writer.stdout_text();
        assert!(text.contains("Changelogs updated:"));
        assert!(text.contains("CHANGELOG.md (updated)"));
        assert!(text.contains("crates/b/CHANGELOG.md (created)"));
    }

    #[test]
    fn print_release_output_shows_consumed_changesets() {
        let writer = BufferCliWriter::new();
        let output = ReleaseOutput::new(
            vec![make_package_version("crate-a", "1.0.0", "1.1.0", false)],
            vec![],
            vec![
                PathBuf::from(".changeset/abc.md"),
                PathBuf::from(".changeset/def.md"),
            ],
            vec![],
            None,
        );

        print_release_output(&output, &writer);

        let text = writer.stdout_text();
        assert!(text.contains("Consumed 2 changeset file(s)"));
    }

    #[test]
    fn print_git_result_with_commit() {
        let writer = BufferCliWriter::new();
        let git = GitOperationResult::new(
            Some(CommitResult::new(
                "abc1234def5678".to_string(),
                "release v1.1.0".to_string(),
            )),
            vec![],
            vec![],
        );

        print_git_result(&git, &writer);

        let text = writer.stdout_text();
        assert!(text.contains("Commit created: abc1234"));
    }

    #[test]
    fn print_git_result_with_tags() {
        let writer = BufferCliWriter::new();
        let git = GitOperationResult::new(
            None,
            vec![
                TagResult::new("v1.1.0".to_string(), "abc1234".to_string()),
                TagResult::new("crate-b@2.1.0".to_string(), "abc1234".to_string()),
            ],
            vec![],
        );

        print_git_result(&git, &writer);

        let text = writer.stdout_text();
        assert!(text.contains("Tags created:"));
        assert!(text.contains("v1.1.0"));
        assert!(text.contains("crate-b@2.1.0"));
    }

    #[test]
    fn print_git_result_with_deleted_changesets() {
        let writer = BufferCliWriter::new();
        let git = GitOperationResult::new(
            None,
            vec![],
            vec![
                PathBuf::from(".changeset/a.md"),
                PathBuf::from(".changeset/b.md"),
                PathBuf::from(".changeset/c.md"),
            ],
        );

        print_git_result(&git, &writer);

        let text = writer.stdout_text();
        assert!(text.contains("Deleted 3 changeset file(s)"));
    }

    #[test]
    fn print_git_result_empty_is_silent() {
        let writer = BufferCliWriter::new();
        let git = GitOperationResult::default();

        print_git_result(&git, &writer);

        assert!(writer.stdout_entries().is_empty());
    }
}
