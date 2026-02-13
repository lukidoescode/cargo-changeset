use std::fmt;
use std::str::FromStr;

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
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "consumedForPrerelease"
    )]
    pub consumed_for_prerelease: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageInfo {
    pub name: String,
    pub version: Version,
    pub path: std::path::PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PrereleaseSpec {
    Alpha,
    Beta,
    Rc,
    Custom(String),
}

impl PrereleaseSpec {
    #[must_use]
    pub fn identifier(&self) -> &str {
        match self {
            Self::Alpha => "alpha",
            Self::Beta => "beta",
            Self::Rc => "rc",
            Self::Custom(s) => s,
        }
    }
}

impl fmt::Display for PrereleaseSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.identifier())
    }
}

impl FromStr for PrereleaseSpec {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "alpha" => Self::Alpha,
            "beta" => Self::Beta,
            "rc" => Self::Rc,
            _ => Self::Custom(s.to_string()),
        })
    }
}

impl ValueEnum for PrereleaseSpec {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Alpha, Self::Beta, Self::Rc]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        match self {
            Self::Alpha => Some(clap::builder::PossibleValue::new("alpha")),
            Self::Beta => Some(clap::builder::PossibleValue::new("beta")),
            Self::Rc => Some(clap::builder::PossibleValue::new("rc")),
            Self::Custom(_) => None,
        }
    }
}

#[cfg(test)]
mod prerelease_spec_tests {
    use super::*;

    #[test]
    fn identifier_returns_correct_string() {
        assert_eq!(PrereleaseSpec::Alpha.identifier(), "alpha");
        assert_eq!(PrereleaseSpec::Beta.identifier(), "beta");
        assert_eq!(PrereleaseSpec::Rc.identifier(), "rc");
        assert_eq!(
            PrereleaseSpec::Custom("dev".to_string()).identifier(),
            "dev"
        );
    }

    #[test]
    fn display_matches_identifier() {
        assert_eq!(format!("{}", PrereleaseSpec::Alpha), "alpha");
        assert_eq!(format!("{}", PrereleaseSpec::Beta), "beta");
        assert_eq!(format!("{}", PrereleaseSpec::Rc), "rc");
        assert_eq!(
            format!("{}", PrereleaseSpec::Custom("nightly".to_string())),
            "nightly"
        );
    }

    #[test]
    fn from_str_parses_known_tags() {
        assert_eq!(
            "alpha".parse::<PrereleaseSpec>().unwrap(),
            PrereleaseSpec::Alpha
        );
        assert_eq!(
            "ALPHA".parse::<PrereleaseSpec>().unwrap(),
            PrereleaseSpec::Alpha
        );
        assert_eq!(
            "beta".parse::<PrereleaseSpec>().unwrap(),
            PrereleaseSpec::Beta
        );
        assert_eq!("rc".parse::<PrereleaseSpec>().unwrap(), PrereleaseSpec::Rc);
    }

    #[test]
    fn from_str_custom_for_unknown() {
        let spec: PrereleaseSpec = "nightly".parse().unwrap();
        assert_eq!(spec, PrereleaseSpec::Custom("nightly".to_string()));
    }

    #[test]
    fn value_enum_variants() {
        let variants = PrereleaseSpec::value_variants();
        assert_eq!(variants.len(), 3);
    }
}
