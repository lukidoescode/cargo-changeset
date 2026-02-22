use changeset_core::{BumpType, PrereleaseSpec};
use semver::Version;

/// Represents a planned version change for a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageVersion {
    pub name: String,
    pub current_version: Version,
    pub new_version: Version,
    pub bump_type: BumpType,
}

/// Per-package release configuration from merged CLI + TOML sources.
#[derive(Debug, Clone, Default)]
pub struct PackageReleaseConfig {
    /// Prerelease tag for this package (e.g., "alpha", "beta")
    pub prerelease: Option<PrereleaseSpec>,
    /// Whether to graduate this 0.x package to 1.0.0
    pub graduate_zero: bool,
}
