mod add;
mod additional_packages;
mod changelog_aggregation;
mod init;
mod manage;
pub mod release;
mod status;
mod verify;

use changeset_core::PackageInfo;
use changeset_project::ProjectKind;

use crate::Result;
use crate::traits::ProjectProvider;

pub use crate::planner::{ReleasePlan, VersionPlanner};
pub use add::{AddInput, AddOperation, AddResult};
pub use additional_packages::{
    AdditionalPackageAddInput, AdditionalPackageDirectAddOperation,
    AdditionalPackageDirectEditOperation, AdditionalPackageDirectRemoveOperation,
    AdditionalPackageEditInput, AdditionalPackageEvent, AdditionalPackageInteractiveAddOperation,
    AdditionalPackageInteractiveEditOperation, AdditionalPackageInteractiveRemoveOperation,
    AdditionalPackageListOperation, AdditionalPackageSummaryData,
};
pub use init::{
    InitInput, InitInputBuilder, InitOperation, InitOutput, InitPlan, build_config_from_input,
};
pub use manage::{
    GraduationDirectInput, GraduationDirectOperation, GraduationEvent, GraduationManageOperation,
    PrereleaseDirectInput, PrereleaseDirectOperation, PrereleaseEvent, PrereleaseManageOperation,
};
pub use release::{
    ChangelogUpdate, CommitResult, GitOperationResult, PackageVersion, ReleaseInput,
    ReleaseInputBuilder, ReleaseOperation, ReleaseOutcome, ReleaseOutput, TagResult,
};
pub use release::{
    PackageReleaseConfig, PackageReleaseConfigBuilder, ReleaseCliInput, ReleaseValidator,
    ValidatedReleaseConfig, ValidationError, ValidationErrors,
};
pub use status::{StatusOperation, StatusOutput, StatusOutputBuilder};
pub use verify::{VerifyInput, VerifyInputBuilder, VerifyOperation, VerifyOutcome, VerifyResult};

pub(crate) fn discover_additional_packages_if_workspace<P: ProjectProvider>(
    provider: &P,
    project: &changeset_project::CargoProject,
    root_config: &changeset_project::RootChangesetConfig,
) -> Result<Vec<PackageInfo>> {
    if *project.kind() == ProjectKind::SinglePackage {
        return Ok(Vec::new());
    }
    provider.discover_additional_packages(project.root(), root_config)
}
