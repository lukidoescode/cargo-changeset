use std::fmt;

use clap::ValueEnum;
use semver::Version;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum BumpType {
    Patch,
    Minor,
    Major,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_type_ordering_patch_is_smallest() {
        assert!(BumpType::Patch < BumpType::Minor);
        assert!(BumpType::Patch < BumpType::Major);
    }

    #[test]
    fn bump_type_ordering_minor_is_middle() {
        assert!(BumpType::Minor > BumpType::Patch);
        assert!(BumpType::Minor < BumpType::Major);
    }

    #[test]
    fn bump_type_ordering_major_is_largest() {
        assert!(BumpType::Major > BumpType::Patch);
        assert!(BumpType::Major > BumpType::Minor);
    }

    #[test]
    fn bump_type_max_returns_largest() {
        let bumps = [BumpType::Patch, BumpType::Minor, BumpType::Major];
        assert_eq!(bumps.iter().max(), Some(&BumpType::Major));
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    Default,
    ValueEnum,
)]
#[serde(rename_all = "lowercase")]
pub enum ChangeCategory {
    Added,
    #[default]
    Changed,
    Deprecated,
    Removed,
    Fixed,
    Security,
}

impl fmt::Display for ChangeCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Added => "Added",
            Self::Changed => "Changed",
            Self::Deprecated => "Deprecated",
            Self::Removed => "Removed",
            Self::Fixed => "Fixed",
            Self::Security => "Security",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageRelease {
    pub name: String,
    pub bump_type: BumpType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Changeset {
    pub summary: String,
    pub releases: Vec<PackageRelease>,
    #[serde(default)]
    pub category: ChangeCategory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageInfo {
    pub name: String,
    pub version: Version,
    pub path: std::path::PathBuf,
}
