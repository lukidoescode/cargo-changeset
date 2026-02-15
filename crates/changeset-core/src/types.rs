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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ZeroVersionBehavior {
    #[default]
    EffectiveMinor,
    AutoPromoteOnMajor,
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

/// A changeset represents a single unit of change affecting one or more packages.
///
/// Changesets capture the intent to release: which packages are affected, what type of
/// version bump each requires, and a human-readable summary of the change.
///
/// # Prerelease Consumption
///
/// The `consumed_for_prerelease` field tracks whether this changeset has been included
/// in a prerelease. When set, it contains the prerelease version string (e.g., "1.0.1-alpha.1").
/// Consumed changesets are excluded from subsequent prereleases but are aggregated into
/// the changelog when graduating to a stable release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Changeset {
    pub summary: String,
    pub releases: Vec<PackageRelease>,
    #[serde(default)]
    pub category: ChangeCategory,
    /// Version string of the prerelease that consumed this changeset, if any.
    /// Set during prerelease creation, cleared during graduation to stable.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "consumedForPrerelease"
    )]
    pub consumed_for_prerelease: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub graduate: bool,
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
    type Err = crate::error::PrereleaseSpecParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(Self::Err::Empty);
        }

        if let Some(invalid_char) = s.chars().find(|c| !c.is_ascii_alphanumeric() && *c != '-') {
            return Err(Self::Err::InvalidCharacter(s.to_string(), invalid_char));
        }

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

    #[test]
    fn from_str_rejects_empty_string() {
        let result = "".parse::<PrereleaseSpec>();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            crate::error::PrereleaseSpecParseError::Empty
        );
    }

    #[test]
    fn from_str_rejects_invalid_characters() {
        let result = "alpha.1".parse::<PrereleaseSpec>();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            crate::error::PrereleaseSpecParseError::InvalidCharacter("alpha.1".to_string(), '.')
        );

        let result = "pre release".parse::<PrereleaseSpec>();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            crate::error::PrereleaseSpecParseError::InvalidCharacter(
                "pre release".to_string(),
                ' '
            )
        );

        let result = "alpha_beta".parse::<PrereleaseSpec>();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            crate::error::PrereleaseSpecParseError::InvalidCharacter("alpha_beta".to_string(), '_')
        );
    }

    #[test]
    fn from_str_accepts_valid_semver_identifiers() {
        assert!("alpha".parse::<PrereleaseSpec>().is_ok());
        assert!("alpha-1".parse::<PrereleaseSpec>().is_ok());
        assert!("pre-release-2".parse::<PrereleaseSpec>().is_ok());
        assert!("0".parse::<PrereleaseSpec>().is_ok());
        assert!("123".parse::<PrereleaseSpec>().is_ok());
        assert!("abc123".parse::<PrereleaseSpec>().is_ok());
        assert!("ABC-123-xyz".parse::<PrereleaseSpec>().is_ok());
    }
}
