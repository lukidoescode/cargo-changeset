use strum::{EnumMessage, VariantArray};

use changeset_core::{BumpType, ChangeCategory};
use changeset_manifest::{
    ChangelogLocation, ComparisonLinks, NoneBumpBehavior, TagFormat, ZeroVersionBehavior,
};
use changeset_operations::traits::AdditionalPackageField;

#[derive(Clone, Copy, VariantArray, EnumMessage)]
pub(crate) enum BumpTypeSelectionOption {
    #[strum(message = "patch - Bug fixes (backwards compatible)")]
    Patch,
    #[strum(message = "minor - New features (backwards compatible)")]
    Minor,
    #[strum(message = "major - Breaking changes")]
    Major,
    #[strum(message = "none - No version bump (internal changes only)")]
    None,
}

impl From<BumpTypeSelectionOption> for BumpType {
    fn from(opt: BumpTypeSelectionOption) -> Self {
        match opt {
            BumpTypeSelectionOption::Patch => Self::Patch,
            BumpTypeSelectionOption::Minor => Self::Minor,
            BumpTypeSelectionOption::Major => Self::Major,
            BumpTypeSelectionOption::None => Self::None,
        }
    }
}

#[derive(Clone, Copy, VariantArray, EnumMessage)]
pub(crate) enum ChangeCategorySelectionOption {
    #[strum(message = "changed - General changes (default)")]
    Changed,
    #[strum(message = "added - New features")]
    Added,
    #[strum(message = "fixed - Bug fixes")]
    Fixed,
    #[strum(message = "deprecated - Deprecated features")]
    Deprecated,
    #[strum(message = "removed - Removed features")]
    Removed,
    #[strum(message = "security - Security fixes")]
    Security,
}

impl From<ChangeCategorySelectionOption> for ChangeCategory {
    fn from(opt: ChangeCategorySelectionOption) -> Self {
        match opt {
            ChangeCategorySelectionOption::Changed => Self::Changed,
            ChangeCategorySelectionOption::Added => Self::Added,
            ChangeCategorySelectionOption::Fixed => Self::Fixed,
            ChangeCategorySelectionOption::Deprecated => Self::Deprecated,
            ChangeCategorySelectionOption::Removed => Self::Removed,
            ChangeCategorySelectionOption::Security => Self::Security,
        }
    }
}

#[derive(Clone, Copy, VariantArray, EnumMessage)]
pub(crate) enum ChangelogLocationSelectionOption {
    #[strum(message = "root - Single CHANGELOG.md at project root (default)")]
    Root,
    #[strum(message = "per-package - CHANGELOG.md in each package directory")]
    PerPackage,
}

impl From<ChangelogLocationSelectionOption> for ChangelogLocation {
    fn from(opt: ChangelogLocationSelectionOption) -> Self {
        match opt {
            ChangelogLocationSelectionOption::Root => Self::Root,
            ChangelogLocationSelectionOption::PerPackage => Self::PerPackage,
        }
    }
}

#[derive(Clone, Copy, VariantArray, EnumMessage)]
pub(crate) enum ComparisonLinksSelectionOption {
    #[strum(message = "auto - Generate links if git remote detected (default)")]
    Auto,
    #[strum(message = "enabled - Always generate comparison links")]
    Enabled,
    #[strum(message = "disabled - Never generate comparison links")]
    Disabled,
}

impl From<ComparisonLinksSelectionOption> for ComparisonLinks {
    fn from(opt: ComparisonLinksSelectionOption) -> Self {
        match opt {
            ComparisonLinksSelectionOption::Auto => Self::Auto,
            ComparisonLinksSelectionOption::Enabled => Self::Enabled,
            ComparisonLinksSelectionOption::Disabled => Self::Disabled,
        }
    }
}

#[derive(Clone, Copy, VariantArray, EnumMessage)]
pub(crate) enum ZeroVersionBehaviorSelectionOption {
    #[strum(message = "effective-minor - Major bump on 0.x increments minor (default)")]
    EffectiveMinor,
    #[strum(message = "auto-promote-on-major - Major bump on 0.x promotes to 1.0.0")]
    AutoPromoteOnMajor,
}

impl From<ZeroVersionBehaviorSelectionOption> for ZeroVersionBehavior {
    fn from(opt: ZeroVersionBehaviorSelectionOption) -> Self {
        match opt {
            ZeroVersionBehaviorSelectionOption::EffectiveMinor => Self::EffectiveMinor,
            ZeroVersionBehaviorSelectionOption::AutoPromoteOnMajor => Self::AutoPromoteOnMajor,
        }
    }
}

#[derive(Clone, Copy, VariantArray, EnumMessage)]
pub(crate) enum NoneBumpBehaviorSelectionOption {
    #[strum(message = "promote-to-patch - Treat none bumps as patch releases (default)")]
    PromoteToPatch,
    #[strum(message = "allow - Allow none bumps without version change")]
    Allow,
    #[strum(message = "disallow - Reject changesets with none bump type")]
    Disallow,
}

impl From<NoneBumpBehaviorSelectionOption> for NoneBumpBehavior {
    fn from(opt: NoneBumpBehaviorSelectionOption) -> Self {
        match opt {
            NoneBumpBehaviorSelectionOption::PromoteToPatch => Self::PromoteToPatch,
            NoneBumpBehaviorSelectionOption::Allow => Self::Allow,
            NoneBumpBehaviorSelectionOption::Disallow => Self::Disallow,
        }
    }
}

#[derive(Clone, Copy, VariantArray, EnumMessage)]
pub(crate) enum TagFormatSelectionOption {
    #[strum(message = "version-only - Tags like v1.0.0")]
    VersionOnly,
    #[strum(message = "crate-prefixed - Tags like crate-name@1.0.0")]
    CratePrefixed,
}

impl From<TagFormatSelectionOption> for TagFormat {
    fn from(opt: TagFormatSelectionOption) -> Self {
        match opt {
            TagFormatSelectionOption::VersionOnly => Self::VersionOnly,
            TagFormatSelectionOption::CratePrefixed => Self::CratePrefixed,
        }
    }
}

#[derive(Clone, Copy, VariantArray, EnumMessage)]
pub(crate) enum AdditionalPackageFieldSelectionOption {
    #[strum(message = "path")]
    Path,
    #[strum(message = "influence patterns")]
    Influence,
    #[strum(message = "manifest file path")]
    ManifestFilePath,
    #[strum(message = "manifest format")]
    ManifestFormat,
    #[strum(message = "manifest version path")]
    ManifestVersionFieldPath,
    #[strum(message = "Done")]
    Done,
}

impl AdditionalPackageFieldSelectionOption {
    pub(crate) fn into_field(self) -> Option<AdditionalPackageField> {
        match self {
            Self::Path => Some(AdditionalPackageField::Path),
            Self::Influence => Some(AdditionalPackageField::Influence),
            Self::ManifestFilePath => Some(AdditionalPackageField::ManifestFilePath),
            Self::ManifestFormat => Some(AdditionalPackageField::ManifestFormat),
            Self::ManifestVersionFieldPath => {
                Some(AdditionalPackageField::ManifestVersionFieldPath)
            }
            Self::Done => None,
        }
    }
}
