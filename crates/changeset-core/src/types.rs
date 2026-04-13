use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use clap::ValueEnum;
use gset::Getset;
use semver::Version;
use serde::{Deserialize, Serialize};

pub const CARGO_MANIFEST_FILENAME: &str = "Cargo.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ManifestFormat {
    Toml,
    Yaml,
    Json,
}

impl fmt::Display for ManifestFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Toml => "toml",
            Self::Yaml => "yaml",
            Self::Json => "json",
        };
        write!(f, "{s}")
    }
}

impl FromStr for ManifestFormat {
    type Err = crate::error::ManifestFormatParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "toml" => Ok(Self::Toml),
            "yaml" => Ok(Self::Yaml),
            "json" => Ok(Self::Json),
            _ => Err(crate::error::ManifestFormatParseError(s.to_string())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum BumpType {
    None,
    Patch,
    Minor,
    Major,
}

impl BumpType {
    #[must_use]
    pub fn is_noop(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl fmt::Display for BumpType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::None => "none",
            Self::Patch => "patch",
            Self::Minor => "minor",
            Self::Major => "major",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "kebab-case")]
pub enum ZeroVersionBehavior {
    #[default]
    EffectiveMinor,
    AutoPromoteOnMajor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "kebab-case")]
pub enum NoneBumpBehavior {
    #[default]
    PromoteToPatch,
    Allow,
    Disallow,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getset)]
#[serde(rename_all = "kebab-case")]
pub struct AdditionalPackageManifest {
    #[getset(get, vis = "pub")]
    file_path: PathBuf,
    #[getset(get_copy, vis = "pub")]
    format: ManifestFormat,
    #[getset(get, vis = "pub")]
    version_field_path: String,
}

impl AdditionalPackageManifest {
    #[must_use]
    pub fn new(file_path: PathBuf, format: ManifestFormat, version_field_path: String) -> Self {
        Self {
            file_path,
            format,
            version_field_path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getset)]
#[serde(rename_all = "kebab-case")]
pub struct VersionTrackingManifest {
    #[getset(get, vis = "pub")]
    file_path: PathBuf,
    #[getset(get_copy, vis = "pub")]
    format: ManifestFormat,
    #[getset(get, vis = "pub")]
    version_field_path: String,
}

impl VersionTrackingManifest {
    #[must_use]
    pub fn new(file_path: PathBuf, format: ManifestFormat, version_field_path: String) -> Self {
        Self {
            file_path,
            format,
            version_field_path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getset)]
#[serde(rename_all = "kebab-case")]
pub struct VersionTrackingDependency {
    #[getset(get, vis = "pub")]
    dependency_name: String,
    #[getset(get, vis = "pub")]
    version_tracking_manifest: VersionTrackingManifest,
}

impl VersionTrackingDependency {
    #[must_use]
    pub fn new(
        dependency_name: String,
        version_tracking_manifest: VersionTrackingManifest,
    ) -> Self {
        Self {
            dependency_name,
            version_tracking_manifest,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getset)]
#[serde(rename_all = "kebab-case")]
pub struct AdditionalPackageDeclaration {
    #[getset(get, vis = "pub")]
    name: String,
    #[getset(get, vis = "pub")]
    path: PathBuf,
    #[getset(get, vis = "pub")]
    influence: Vec<String>,
    #[getset(get, vis = "pub")]
    manifest: AdditionalPackageManifest,
    #[serde(default)]
    #[getset(get, vis = "pub")]
    dependencies: Vec<VersionTrackingDependency>,
}

impl AdditionalPackageDeclaration {
    #[must_use]
    pub fn new(
        name: String,
        path: PathBuf,
        influence: Vec<String>,
        manifest: AdditionalPackageManifest,
        dependencies: Vec<VersionTrackingDependency>,
    ) -> Self {
        Self {
            name,
            path,
            influence,
            manifest,
            dependencies,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getset)]
pub struct PackageRelease {
    #[getset(get, vis = "pub")]
    name: String,
    #[getset(get_copy, vis = "pub")]
    bump_type: BumpType,
}

impl PackageRelease {
    #[must_use]
    pub fn new(name: String, bump_type: BumpType) -> Self {
        Self { name, bump_type }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Getset)]
pub struct Changeset {
    #[getset(get, vis = "pub")]
    summary: String,
    #[getset(get, vis = "pub")]
    releases: Vec<PackageRelease>,
    #[serde(default)]
    #[getset(get_copy, vis = "pub")]
    category: ChangeCategory,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "consumedForPrerelease"
    )]
    #[getset(get_as_ref, vis = "pub", ty = "Option<&String>")]
    consumed_for_prerelease: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[getset(get_copy, vis = "pub")]
    graduate: bool,
}

impl Changeset {
    #[must_use]
    pub fn new(summary: String, releases: Vec<PackageRelease>, category: ChangeCategory) -> Self {
        Self {
            summary,
            releases,
            category,
            consumed_for_prerelease: None,
            graduate: false,
        }
    }

    #[must_use]
    pub fn with_consumed_for_prerelease(mut self, v: Option<String>) -> Self {
        self.consumed_for_prerelease = v;
        self
    }

    #[must_use]
    pub fn with_graduate(mut self, v: bool) -> Self {
        self.graduate = v;
        self
    }

    pub fn set_consumed_for_prerelease(&mut self, v: Option<String>) {
        self.consumed_for_prerelease = v;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Getset)]
pub struct PackageInfo {
    #[getset(get, vis = "pub")]
    name: String,
    #[getset(get, vis = "pub")]
    version: Version,
    #[getset(get, vis = "pub")]
    path: PathBuf,
}

impl PackageInfo {
    #[must_use]
    pub fn new(name: String, version: Version, path: PathBuf) -> Self {
        Self {
            name,
            version,
            path,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_type_ordering_none_is_smallest() {
        assert!(BumpType::None < BumpType::Patch);
        assert!(BumpType::None < BumpType::Minor);
        assert!(BumpType::None < BumpType::Major);
    }

    #[test]
    fn bump_type_ordering_patch_is_second() {
        assert!(BumpType::Patch > BumpType::None);
        assert!(BumpType::Patch < BumpType::Minor);
        assert!(BumpType::Patch < BumpType::Major);
    }

    #[test]
    fn bump_type_ordering_minor_is_middle() {
        assert!(BumpType::Minor > BumpType::None);
        assert!(BumpType::Minor > BumpType::Patch);
        assert!(BumpType::Minor < BumpType::Major);
    }

    #[test]
    fn bump_type_ordering_major_is_largest() {
        assert!(BumpType::Major > BumpType::None);
        assert!(BumpType::Major > BumpType::Patch);
        assert!(BumpType::Major > BumpType::Minor);
    }

    #[test]
    fn bump_type_max_returns_largest() {
        let bumps = [BumpType::Patch, BumpType::Minor, BumpType::Major];
        assert_eq!(bumps.iter().max(), Some(&BumpType::Major));
    }

    #[test]
    fn bump_type_max_none_and_patch_returns_patch() {
        let bumps = [BumpType::None, BumpType::Patch];
        assert_eq!(bumps.iter().max(), Some(&BumpType::Patch));
    }

    #[test]
    fn bump_type_max_all_none_returns_none() {
        let bumps = [BumpType::None, BumpType::None];
        assert_eq!(bumps.iter().max(), Some(&BumpType::None));
    }

    #[test]
    fn bump_type_is_noop() {
        assert!(BumpType::None.is_noop());
        assert!(!BumpType::Patch.is_noop());
        assert!(!BumpType::Minor.is_noop());
        assert!(!BumpType::Major.is_noop());
    }

    #[test]
    fn bump_type_display() {
        assert_eq!(format!("{}", BumpType::None), "none");
        assert_eq!(format!("{}", BumpType::Patch), "patch");
        assert_eq!(format!("{}", BumpType::Minor), "minor");
        assert_eq!(format!("{}", BumpType::Major), "major");
    }

    #[test]
    fn none_bump_behavior_default_is_promote_to_patch() {
        assert_eq!(
            NoneBumpBehavior::default(),
            NoneBumpBehavior::PromoteToPatch
        );
    }

    #[test]
    fn none_bump_behavior_serde_round_trip_promote_to_patch() {
        let serialized = serde_json::to_string(&NoneBumpBehavior::PromoteToPatch).unwrap();
        assert_eq!(serialized, r#""promote-to-patch""#);
        let deserialized: NoneBumpBehavior = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, NoneBumpBehavior::PromoteToPatch);
    }

    #[test]
    fn none_bump_behavior_serde_round_trip_allow() {
        let serialized = serde_json::to_string(&NoneBumpBehavior::Allow).unwrap();
        assert_eq!(serialized, r#""allow""#);
        let deserialized: NoneBumpBehavior = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, NoneBumpBehavior::Allow);
    }

    #[test]
    fn none_bump_behavior_serde_round_trip_disallow() {
        let serialized = serde_json::to_string(&NoneBumpBehavior::Disallow).unwrap();
        assert_eq!(serialized, r#""disallow""#);
        let deserialized: NoneBumpBehavior = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, NoneBumpBehavior::Disallow);
    }

    #[test]
    fn ordering_matches_keep_a_changelog_convention() {
        assert!(ChangeCategory::Added < ChangeCategory::Changed);
        assert!(ChangeCategory::Changed < ChangeCategory::Deprecated);
        assert!(ChangeCategory::Deprecated < ChangeCategory::Removed);
        assert!(ChangeCategory::Removed < ChangeCategory::Fixed);
        assert!(ChangeCategory::Fixed < ChangeCategory::Security);
    }

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

    #[test]
    fn manifest_format_display() {
        assert_eq!(format!("{}", ManifestFormat::Toml), "toml");
        assert_eq!(format!("{}", ManifestFormat::Yaml), "yaml");
        assert_eq!(format!("{}", ManifestFormat::Json), "json");
    }

    #[test]
    fn manifest_format_from_str_case_insensitive() {
        assert_eq!(
            "toml".parse::<ManifestFormat>().unwrap(),
            ManifestFormat::Toml
        );
        assert_eq!(
            "TOML".parse::<ManifestFormat>().unwrap(),
            ManifestFormat::Toml
        );
        assert_eq!(
            "Toml".parse::<ManifestFormat>().unwrap(),
            ManifestFormat::Toml
        );
        assert_eq!(
            "yaml".parse::<ManifestFormat>().unwrap(),
            ManifestFormat::Yaml
        );
        assert_eq!(
            "YAML".parse::<ManifestFormat>().unwrap(),
            ManifestFormat::Yaml
        );
        assert_eq!(
            "json".parse::<ManifestFormat>().unwrap(),
            ManifestFormat::Json
        );
        assert_eq!(
            "JSON".parse::<ManifestFormat>().unwrap(),
            ManifestFormat::Json
        );
    }

    #[test]
    fn manifest_format_from_str_rejects_unknown() {
        let err = "xml".parse::<ManifestFormat>().unwrap_err();
        assert_eq!(
            err,
            crate::error::ManifestFormatParseError("xml".to_string())
        );
    }

    #[test]
    fn manifest_format_serde_round_trip() {
        for (variant, expected) in [
            (ManifestFormat::Toml, r#""toml""#),
            (ManifestFormat::Yaml, r#""yaml""#),
            (ManifestFormat::Json, r#""json""#),
        ] {
            let serialized = serde_json::to_string(&variant).unwrap();
            assert_eq!(serialized, expected);
            let deserialized: ManifestFormat = serde_json::from_str(&serialized).unwrap();
            assert_eq!(deserialized, variant);
        }
    }

    #[test]
    fn additional_package_manifest_serde_round_trip() {
        let manifest = AdditionalPackageManifest {
            file_path: PathBuf::from("charts/my-chart/Chart.yaml"),
            format: ManifestFormat::Yaml,
            version_field_path: "version".to_string(),
        };
        let serialized = serde_json::to_string(&manifest).unwrap();
        assert!(serialized.contains(r#""file-path""#));
        assert!(serialized.contains(r#""version-field-path""#));
        let deserialized: AdditionalPackageManifest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, manifest);
    }

    #[test]
    fn additional_package_declaration_serde_round_trip() {
        let decl = AdditionalPackageDeclaration {
            name: "my-helm-chart".to_string(),
            path: PathBuf::from("charts/my-chart"),
            influence: vec!["charts/my-chart/**".to_string()],
            manifest: AdditionalPackageManifest {
                file_path: PathBuf::from("charts/my-chart/Chart.yaml"),
                format: ManifestFormat::Yaml,
                version_field_path: "version".to_string(),
            },
            dependencies: vec![],
        };
        let serialized = serde_json::to_string(&decl).unwrap();
        let deserialized: AdditionalPackageDeclaration = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, decl);
    }

    #[test]
    fn manifest_format_parse_error_display() {
        let err = crate::error::ManifestFormatParseError("bad".to_string());
        assert_eq!(
            err.to_string(),
            "unknown manifest format 'bad', expected one of: toml, yaml, json"
        );
    }

    #[test]
    fn additional_package_declaration_missing_required_field() {
        let json = r#"{
            "path": "charts/my-chart",
            "influence": ["charts/my-chart/**"],
            "manifest": {
                "file-path": "charts/my-chart/Chart.yaml",
                "format": "yaml",
                "version-field-path": "version"
            }
        }"#;
        let result = serde_json::from_str::<AdditionalPackageDeclaration>(json);
        assert!(result.is_err());
    }

    #[test]
    fn version_tracking_manifest_serde_round_trip() {
        let manifest = VersionTrackingManifest::new(
            PathBuf::from("charts/app/Chart.yaml"),
            ManifestFormat::Yaml,
            "appVersion".to_string(),
        );
        let serialized = toml::to_string(&manifest).expect("serialize to TOML");
        let deserialized: VersionTrackingManifest =
            toml::from_str(&serialized).expect("deserialize from TOML");
        assert_eq!(deserialized, manifest);
    }

    #[test]
    fn version_tracking_dependency_serde_round_trip() {
        let dep = VersionTrackingDependency::new(
            "my-lib".to_string(),
            VersionTrackingManifest::new(
                PathBuf::from("charts/app/Chart.yaml"),
                ManifestFormat::Yaml,
                "appVersion".to_string(),
            ),
        );
        let serialized = toml::to_string(&dep).expect("serialize to TOML");
        let deserialized: VersionTrackingDependency =
            toml::from_str(&serialized).expect("deserialize from TOML");
        assert_eq!(deserialized, dep);
    }

    #[test]
    fn additional_package_declaration_with_dependencies_serde_round_trip() {
        let decl = AdditionalPackageDeclaration::new(
            "my-helm-chart".to_string(),
            PathBuf::from("charts/my-chart"),
            vec!["charts/my-chart/**".to_string()],
            AdditionalPackageManifest::new(
                PathBuf::from("charts/my-chart/Chart.yaml"),
                ManifestFormat::Yaml,
                "version".to_string(),
            ),
            vec![VersionTrackingDependency::new(
                "my-lib".to_string(),
                VersionTrackingManifest::new(
                    PathBuf::from("charts/my-chart/Chart.yaml"),
                    ManifestFormat::Yaml,
                    "appVersion".to_string(),
                ),
            )],
        );
        let serialized = toml::to_string(&decl).expect("serialize to TOML");
        let deserialized: AdditionalPackageDeclaration =
            toml::from_str(&serialized).expect("deserialize from TOML");
        assert_eq!(deserialized, decl);
        assert_eq!(deserialized.dependencies().len(), 1);
        assert_eq!(deserialized.dependencies()[0].dependency_name(), "my-lib");
    }

    #[test]
    fn version_tracking_manifest_serde_key_names() {
        let manifest = VersionTrackingManifest::new(
            PathBuf::from("path/to/manifest.json"),
            ManifestFormat::Json,
            "version".to_string(),
        );
        let serialized = serde_json::to_string(&manifest).expect("serialize to JSON");
        assert!(
            serialized.contains(r#""file-path""#),
            "expected kebab-case key 'file-path' in JSON output: {serialized}"
        );
        assert!(
            serialized.contains(r#""version-field-path""#),
            "expected kebab-case key 'version-field-path' in JSON output: {serialized}"
        );
    }

    #[test]
    fn version_tracking_dependency_serde_key_names() {
        let dep = VersionTrackingDependency::new(
            "some-dep".to_string(),
            VersionTrackingManifest::new(
                PathBuf::from("tracking.json"),
                ManifestFormat::Json,
                "ver".to_string(),
            ),
        );
        let serialized = serde_json::to_string(&dep).expect("serialize to JSON");
        assert!(
            serialized.contains(r#""dependency-name""#),
            "expected kebab-case key 'dependency-name' in JSON output: {serialized}"
        );
        assert!(
            serialized.contains(r#""version-tracking-manifest""#),
            "expected kebab-case key 'version-tracking-manifest' in JSON output: {serialized}"
        );
    }

    #[test]
    fn additional_package_declaration_with_multiple_dependencies() {
        let decl = AdditionalPackageDeclaration::new(
            "multi-dep-pkg".to_string(),
            PathBuf::from("packages/multi"),
            vec!["packages/multi/**".to_string()],
            AdditionalPackageManifest::new(
                PathBuf::from("packages/multi/manifest.yaml"),
                ManifestFormat::Yaml,
                "version".to_string(),
            ),
            vec![
                VersionTrackingDependency::new(
                    "dep-alpha".to_string(),
                    VersionTrackingManifest::new(
                        PathBuf::from("tracking/alpha.json"),
                        ManifestFormat::Json,
                        "alphaVersion".to_string(),
                    ),
                ),
                VersionTrackingDependency::new(
                    "dep-beta".to_string(),
                    VersionTrackingManifest::new(
                        PathBuf::from("tracking/beta.yaml"),
                        ManifestFormat::Yaml,
                        "betaVersion".to_string(),
                    ),
                ),
            ],
        );

        let serialized = toml::to_string(&decl).expect("serialize to TOML");
        let deserialized: AdditionalPackageDeclaration =
            toml::from_str(&serialized).expect("deserialize from TOML");

        assert_eq!(deserialized, decl);
        assert_eq!(deserialized.dependencies().len(), 2);
        assert_eq!(
            deserialized.dependencies()[0].dependency_name(),
            "dep-alpha"
        );
        assert_eq!(
            deserialized.dependencies()[0]
                .version_tracking_manifest()
                .version_field_path(),
            "alphaVersion"
        );
        assert_eq!(deserialized.dependencies()[1].dependency_name(), "dep-beta");
        assert_eq!(
            deserialized.dependencies()[1]
                .version_tracking_manifest()
                .version_field_path(),
            "betaVersion"
        );
    }

    #[test]
    fn additional_package_declaration_without_dependencies_defaults_to_empty() {
        let toml_str = r#"
name = "my-helm-chart"
path = "charts/my-chart"
influence = ["charts/my-chart/**"]

[manifest]
file-path = "charts/my-chart/Chart.yaml"
format = "yaml"
version-field-path = "version"
"#;
        let deserialized: AdditionalPackageDeclaration =
            toml::from_str(toml_str).expect("deserialize from TOML");
        assert!(deserialized.dependencies().is_empty());
        assert_eq!(deserialized.name(), "my-helm-chart");
    }
}
