use std::collections::{HashMap, HashSet};

use changeset_core::{PackageInfo, PrereleaseSpec};
use changeset_project::{GraduationState, PrereleaseState, ProjectKind};
use changeset_version::{is_prerelease, is_zero_version};

/// Per-package release configuration from merged CLI + TOML sources.
#[derive(Debug, Clone, Default)]
pub struct PackageReleaseConfig {
    /// Prerelease tag for this package (e.g., "alpha", "beta")
    pub prerelease: Option<PrereleaseSpec>,
    /// Whether to graduate this 0.x package to 1.0.0
    pub graduate_zero: bool,
}

/// Input from CLI for validation.
#[derive(Debug, Clone, Default)]
pub struct ReleaseCliInput {
    /// Per-package prerelease from --prerelease crate:tag
    pub cli_prerelease: HashMap<String, PrereleaseSpec>,
    /// Global prerelease tag (applies to all packages)
    pub global_prerelease: Option<PrereleaseSpec>,
    /// Packages to graduate from --graduate crate
    pub cli_graduate: HashSet<String>,
    /// Whether --graduate was passed without specific crates
    pub graduate_all: bool,
}

/// A single validation error with actionable tip.
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// CLI tag differs from pre-release.toml tag
    ConflictingPrereleaseTag {
        package: String,
        cli_tag: String,
        toml_tag: String,
    },
    /// Package in prerelease version cannot be graduated to 1.0.0
    CannotGraduateFromPrerelease {
        package: String,
        current_version: String,
    },
    /// --graduate without crate names in workspace
    GraduateRequiresCratesInWorkspace,
    /// Package not found in workspace
    PackageNotFound {
        name: String,
        available: Vec<String>,
    },
    /// Cannot graduate package >= 1.0.0
    CannotGraduateStableVersion { package: String, version: String },
    /// Invalid prerelease tag in pre-release.toml (failed to parse)
    InvalidPrereleaseTag {
        package: String,
        tag: String,
        reason: String,
    },
}

impl ValidationError {
    /// Returns an actionable tip for resolving this error.
    #[must_use]
    pub fn tip(&self) -> String {
        match self {
            Self::ConflictingPrereleaseTag {
                package, toml_tag, ..
            } => {
                format!(
                    "Run `cargo changeset manage pre-release --remove {package}` to clear TOML, \
                     or use `--prerelease {package}:{toml_tag}` to match"
                )
            }
            Self::CannotGraduateFromPrerelease { package, .. } => {
                format!(
                    "First release {package} to stable with `cargo changeset release`, \
                     then graduate with `--graduate {package}`"
                )
            }
            Self::GraduateRequiresCratesInWorkspace => {
                "Specify crates: `--graduate crate-a --graduate crate-b`".to_string()
            }
            Self::PackageNotFound { name, available } => {
                let available_str = available.join(", ");
                format!("Package '{name}' not found. Available: {available_str}")
            }
            Self::CannotGraduateStableVersion { package, version } => {
                format!("Package {package} is already at {version}; graduation is for 0.x only")
            }
            Self::InvalidPrereleaseTag { package, .. } => {
                format!(
                    "Run `cargo changeset manage pre-release --remove {package}` and re-add with a valid tag"
                )
            }
        }
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConflictingPrereleaseTag {
                package,
                cli_tag,
                toml_tag,
            } => {
                write!(
                    f,
                    "conflicting prerelease tag for '{package}': CLI specifies '{cli_tag}', \
                     pre-release.toml specifies '{toml_tag}'"
                )
            }
            Self::CannotGraduateFromPrerelease {
                package,
                current_version,
            } => {
                write!(
                    f,
                    "cannot graduate '{package}': currently in prerelease ({current_version})"
                )
            }
            Self::GraduateRequiresCratesInWorkspace => {
                write!(f, "--graduate requires crate names in workspace")
            }
            Self::PackageNotFound { name, .. } => {
                write!(f, "package '{name}' not found in workspace")
            }
            Self::CannotGraduateStableVersion { package, version } => {
                write!(
                    f,
                    "cannot graduate '{package}': already at stable version {version}"
                )
            }
            Self::InvalidPrereleaseTag {
                package,
                tag,
                reason,
            } => {
                write!(
                    f,
                    "invalid prerelease tag '{tag}' in pre-release.toml for package '{package}': \
                     {reason}"
                )
            }
        }
    }
}

/// Collection of validation errors (guaranteed non-empty when constructed).
///
/// This type is only constructed when validation fails, so it always contains at least
/// one error. Use `ValidationErrorCollector` during validation, then convert to this
/// type only when errors are present.
///
/// The `is_empty` method is intentionally omitted because this type is guaranteed
/// to be non-empty by construction.
#[derive(Debug)]
#[allow(clippy::len_without_is_empty)]
pub struct ValidationErrors {
    first: ValidationError,
    rest: Vec<ValidationError>,
}

impl ValidationErrors {
    /// Creates a new `ValidationErrors` from a non-empty vector.
    ///
    /// # Panics
    ///
    /// Panics if the vector is empty. Use `try_from_vec` for a fallible version.
    #[must_use]
    pub fn from_vec(mut errors: Vec<ValidationError>) -> Self {
        assert!(
            !errors.is_empty(),
            "ValidationErrors must contain at least one error"
        );
        let first = errors.remove(0);
        Self {
            first,
            rest: errors,
        }
    }

    /// Creates a new `ValidationErrors` from a vector, returning `None` if empty.
    #[must_use]
    pub fn try_from_vec(mut errors: Vec<ValidationError>) -> Option<Self> {
        if errors.is_empty() {
            return None;
        }
        let first = errors.remove(0);
        Some(Self {
            first,
            rest: errors,
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        1 + self.rest.len()
    }

    #[must_use]
    pub fn into_vec(self) -> Vec<ValidationError> {
        let mut errors = vec![self.first];
        errors.extend(self.rest);
        errors
    }

    pub fn iter(&self) -> impl Iterator<Item = &ValidationError> {
        std::iter::once(&self.first).chain(self.rest.iter())
    }
}

/// Collector for validation errors during validation phase.
///
/// This is a builder type used internally by `ReleaseValidator`. It allows
/// accumulating errors and then converting to `ValidationErrors` only when
/// there are actual errors to report.
#[derive(Debug, Default)]
pub(crate) struct ValidationErrorCollector {
    errors: Vec<ValidationError>,
}

impl ValidationErrorCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    /// Converts to `ValidationErrors` if there are any errors.
    pub fn into_errors(self) -> Option<ValidationErrors> {
        ValidationErrors::try_from_vec(self.errors)
    }
}

impl std::fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "validation failed with {} error(s):", self.len())?;
        for (i, error) in self.iter().enumerate() {
            writeln!(f, "  {}. {error}", i + 1)?;
            writeln!(f, "     Tip: {}", error.tip())?;
        }
        Ok(())
    }
}

impl std::error::Error for ValidationErrors {}

impl IntoIterator for ValidationErrors {
    type Item = ValidationError;
    type IntoIter = std::vec::IntoIter<ValidationError>;

    fn into_iter(self) -> Self::IntoIter {
        self.into_vec().into_iter()
    }
}

impl<'a> IntoIterator for &'a ValidationErrors {
    type Item = &'a ValidationError;
    type IntoIter = Box<dyn Iterator<Item = &'a ValidationError> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(std::iter::once(&self.first).chain(self.rest.iter()))
    }
}

/// Result of successful validation: per-package configuration.
#[derive(Debug, Clone)]
pub struct ValidatedReleaseConfig {
    /// Per-package configuration (package name -> config)
    pub per_package: HashMap<String, PackageReleaseConfig>,
}

/// Intermediate cache for validated and parsed prerelease specs from TOML.
struct ParsedPrereleaseCache {
    specs: HashMap<String, PrereleaseSpec>,
}

/// Validates release configuration before execution.
///
/// This validator ensures:
/// 1. CLI and TOML prerelease tags are consistent
/// 2. Graduation targets are valid (0.x, not prerelease)
/// 3. All referenced packages exist
/// 4. No conflicting configurations
pub struct ReleaseValidator;

impl ReleaseValidator {
    /// Validates the release configuration, collecting ALL errors.
    ///
    /// Returns `Ok(ValidatedReleaseConfig)` if all validations pass,
    /// or `Err(ValidationErrors)` containing ALL validation errors.
    ///
    /// # Errors
    ///
    /// Returns `ValidationErrors` if any validation rule fails. All errors are
    /// collected before returning, so the caller receives a complete list of
    /// issues rather than just the first one.
    pub fn validate(
        cli_input: &ReleaseCliInput,
        prerelease_state: Option<&PrereleaseState>,
        graduation_state: Option<&GraduationState>,
        packages: &[PackageInfo],
        project_kind: &ProjectKind,
    ) -> Result<ValidatedReleaseConfig, ValidationErrors> {
        let mut collector = ValidationErrorCollector::new();
        let package_names: HashSet<_> = packages.iter().map(|p| p.name.as_str()).collect();
        let available_packages: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
        let package_lookup: HashMap<_, _> = packages.iter().map(|p| (p.name.as_str(), p)).collect();

        Self::validate_packages_exist(
            cli_input.cli_prerelease.keys().map(String::as_str),
            &package_names,
            &available_packages,
            &mut collector,
        );

        Self::validate_packages_exist(
            cli_input.cli_graduate.iter().map(String::as_str),
            &package_names,
            &available_packages,
            &mut collector,
        );

        let parsed_cache =
            Self::validate_and_parse_toml_prerelease(prerelease_state, &mut collector);

        Self::validate_prerelease_consistency(cli_input, prerelease_state, &mut collector);

        Self::validate_graduation_not_from_prerelease(
            cli_input,
            graduation_state,
            &package_lookup,
            &mut collector,
        );

        Self::validate_graduation_targets(
            cli_input,
            graduation_state,
            &package_lookup,
            &mut collector,
        );

        Self::validate_workspace_graduation(cli_input, project_kind, &mut collector);

        if let Some(errors) = collector.into_errors() {
            Err(errors)
        } else {
            Ok(Self::build_config(
                cli_input,
                &parsed_cache,
                graduation_state,
                packages,
            ))
        }
    }

    fn validate_packages_exist<'a>(
        names: impl Iterator<Item = &'a str>,
        valid_names: &HashSet<&str>,
        available_packages: &[String],
        collector: &mut ValidationErrorCollector,
    ) {
        for name in names {
            if !valid_names.contains(name) {
                collector.push(ValidationError::PackageNotFound {
                    name: name.to_string(),
                    available: available_packages.to_vec(),
                });
            }
        }
    }

    fn validate_prerelease_consistency(
        cli_input: &ReleaseCliInput,
        prerelease_state: Option<&PrereleaseState>,
        collector: &mut ValidationErrorCollector,
    ) {
        let Some(state) = prerelease_state else {
            return;
        };

        for (pkg, cli_spec) in &cli_input.cli_prerelease {
            if let Some(toml_tag) = state.get(pkg) {
                let cli_tag = cli_spec.to_string();
                if cli_tag != toml_tag {
                    collector.push(ValidationError::ConflictingPrereleaseTag {
                        package: pkg.clone(),
                        cli_tag,
                        toml_tag: toml_tag.to_string(),
                    });
                }
            }
        }
    }

    fn validate_graduation_not_from_prerelease(
        cli_input: &ReleaseCliInput,
        graduation_state: Option<&GraduationState>,
        package_lookup: &HashMap<&str, &PackageInfo>,
        collector: &mut ValidationErrorCollector,
    ) {
        for pkg_name in &cli_input.cli_graduate {
            if let Some(pkg) = package_lookup.get(pkg_name.as_str()) {
                if is_prerelease(&pkg.version) {
                    collector.push(ValidationError::CannotGraduateFromPrerelease {
                        package: pkg_name.clone(),
                        current_version: pkg.version.to_string(),
                    });
                }
            }
        }

        if let Some(state) = graduation_state {
            for pkg_name in state.iter() {
                if let Some(pkg) = package_lookup.get(pkg_name) {
                    if is_prerelease(&pkg.version) {
                        collector.push(ValidationError::CannotGraduateFromPrerelease {
                            package: pkg_name.to_string(),
                            current_version: pkg.version.to_string(),
                        });
                    }
                }
            }
        }
    }

    fn validate_graduation_targets(
        cli_input: &ReleaseCliInput,
        graduation_state: Option<&GraduationState>,
        package_lookup: &HashMap<&str, &PackageInfo>,
        collector: &mut ValidationErrorCollector,
    ) {
        for pkg_name in &cli_input.cli_graduate {
            if let Some(pkg) = package_lookup.get(pkg_name.as_str()) {
                if !is_zero_version(&pkg.version) && !is_prerelease(&pkg.version) {
                    collector.push(ValidationError::CannotGraduateStableVersion {
                        package: pkg_name.clone(),
                        version: pkg.version.to_string(),
                    });
                }
            }
        }

        if let Some(state) = graduation_state {
            for pkg_name in state.iter() {
                if let Some(pkg) = package_lookup.get(pkg_name) {
                    if !is_zero_version(&pkg.version) && !is_prerelease(&pkg.version) {
                        collector.push(ValidationError::CannotGraduateStableVersion {
                            package: pkg_name.to_string(),
                            version: pkg.version.to_string(),
                        });
                    }
                }
            }
        }
    }

    fn validate_workspace_graduation(
        cli_input: &ReleaseCliInput,
        project_kind: &ProjectKind,
        collector: &mut ValidationErrorCollector,
    ) {
        if *project_kind == ProjectKind::SinglePackage {
            return;
        }

        if cli_input.graduate_all && cli_input.cli_graduate.is_empty() {
            collector.push(ValidationError::GraduateRequiresCratesInWorkspace);
        }
    }

    /// Validates and parses TOML prerelease tags, caching successfully parsed specs.
    ///
    /// This method performs two validations:
    /// 1. Basic identifier validation (non-empty, valid characters)
    /// 2. Full parsing into `PrereleaseSpec`
    ///
    /// Successfully parsed specs are cached for use in `build_config`.
    fn validate_and_parse_toml_prerelease(
        prerelease_state: Option<&PrereleaseState>,
        collector: &mut ValidationErrorCollector,
    ) -> ParsedPrereleaseCache {
        let mut specs = HashMap::new();

        let Some(state) = prerelease_state else {
            return ParsedPrereleaseCache { specs };
        };

        for (pkg, tag) in state.iter() {
            match tag.parse::<PrereleaseSpec>() {
                Ok(spec) => {
                    specs.insert(pkg.to_string(), spec);
                }
                Err(e) => {
                    collector.push(ValidationError::InvalidPrereleaseTag {
                        package: pkg.to_string(),
                        tag: tag.to_string(),
                        reason: e.to_string(),
                    });
                }
            }
        }

        ParsedPrereleaseCache { specs }
    }

    /// Builds the final configuration from validated inputs.
    ///
    /// This method is infallible because all validation has already occurred
    /// in the validation phase. It uses the pre-parsed `PrereleaseSpec` values
    /// from the cache rather than re-parsing.
    fn build_config(
        cli_input: &ReleaseCliInput,
        parsed_cache: &ParsedPrereleaseCache,
        graduation_state: Option<&GraduationState>,
        packages: &[PackageInfo],
    ) -> ValidatedReleaseConfig {
        let mut per_package = HashMap::new();

        for (pkg, spec) in &parsed_cache.specs {
            per_package
                .entry(pkg.clone())
                .or_insert_with(PackageReleaseConfig::default)
                .prerelease = Some(spec.clone());
        }

        for (pkg, spec) in &cli_input.cli_prerelease {
            per_package
                .entry(pkg.clone())
                .or_insert_with(PackageReleaseConfig::default)
                .prerelease = Some(spec.clone());
        }

        if let Some(global) = &cli_input.global_prerelease {
            for pkg in packages {
                per_package
                    .entry(pkg.name.clone())
                    .or_insert_with(PackageReleaseConfig::default)
                    .prerelease = Some(global.clone());
            }
        }

        if let Some(state) = graduation_state {
            for pkg in state.iter() {
                per_package
                    .entry(pkg.to_string())
                    .or_insert_with(PackageReleaseConfig::default)
                    .graduate_zero = true;
            }
        }

        for pkg in &cli_input.cli_graduate {
            per_package
                .entry(pkg.clone())
                .or_insert_with(PackageReleaseConfig::default)
                .graduate_zero = true;
        }

        if cli_input.graduate_all {
            for pkg in packages {
                if is_zero_version(&pkg.version) {
                    per_package
                        .entry(pkg.name.clone())
                        .or_insert_with(PackageReleaseConfig::default)
                        .graduate_zero = true;
                }
            }
        }

        ValidatedReleaseConfig { per_package }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_package(name: &str, version: &str) -> PackageInfo {
        PackageInfo {
            name: name.to_string(),
            version: version.parse().expect("valid version"),
            path: PathBuf::from(format!("/mock/{name}")),
        }
    }

    mod prerelease_consistency {
        use super::*;

        #[test]
        fn matching_tags_pass() {
            let packages = vec![make_package("crate-a", "1.0.0")];
            let mut cli_input = ReleaseCliInput::default();
            cli_input
                .cli_prerelease
                .insert("crate-a".to_string(), PrereleaseSpec::Alpha);

            let mut prerelease_state = PrereleaseState::new();
            prerelease_state.insert("crate-a".to_string(), "alpha".to_string());

            let result = ReleaseValidator::validate(
                &cli_input,
                Some(&prerelease_state),
                None,
                &packages,
                &ProjectKind::SinglePackage,
            );

            assert!(result.is_ok());
        }

        #[test]
        fn conflicting_tags_fail() {
            let packages = vec![make_package("crate-a", "1.0.0")];
            let mut cli_input = ReleaseCliInput::default();
            cli_input
                .cli_prerelease
                .insert("crate-a".to_string(), PrereleaseSpec::Beta);

            let mut prerelease_state = PrereleaseState::new();
            prerelease_state.insert("crate-a".to_string(), "alpha".to_string());

            let result = ReleaseValidator::validate(
                &cli_input,
                Some(&prerelease_state),
                None,
                &packages,
                &ProjectKind::SinglePackage,
            );

            assert!(result.is_err());
            let errors = result.expect_err("validation should fail");
            assert_eq!(errors.len(), 1);
            assert!(matches!(
                errors.iter().next().expect("at least one error"),
                ValidationError::ConflictingPrereleaseTag { .. }
            ));
        }
    }

    mod graduation_validation {
        use super::*;

        #[test]
        fn cannot_graduate_prerelease_version() {
            let packages = vec![make_package("crate-a", "1.0.0-alpha.1")];
            let mut cli_input = ReleaseCliInput::default();
            cli_input.cli_graduate.insert("crate-a".to_string());

            let result = ReleaseValidator::validate(
                &cli_input,
                None,
                None,
                &packages,
                &ProjectKind::SinglePackage,
            );

            assert!(result.is_err());
            let errors = result.expect_err("validation should fail");
            assert!(matches!(
                errors.iter().next().expect("at least one error"),
                ValidationError::CannotGraduateFromPrerelease { .. }
            ));
        }

        #[test]
        fn cannot_graduate_stable_version() {
            let packages = vec![make_package("crate-a", "2.0.0")];
            let mut cli_input = ReleaseCliInput::default();
            cli_input.cli_graduate.insert("crate-a".to_string());

            let result = ReleaseValidator::validate(
                &cli_input,
                None,
                None,
                &packages,
                &ProjectKind::SinglePackage,
            );

            assert!(result.is_err());
            let errors = result.expect_err("validation should fail");
            assert!(matches!(
                errors.iter().next().expect("at least one error"),
                ValidationError::CannotGraduateStableVersion { .. }
            ));
        }

        #[test]
        fn zero_version_graduation_passes() {
            let packages = vec![make_package("crate-a", "0.5.0")];
            let mut cli_input = ReleaseCliInput::default();
            cli_input.cli_graduate.insert("crate-a".to_string());

            let result = ReleaseValidator::validate(
                &cli_input,
                None,
                None,
                &packages,
                &ProjectKind::SinglePackage,
            );

            assert!(result.is_ok());
        }

        #[test]
        fn workspace_graduate_requires_crate_names() {
            let packages = vec![
                make_package("crate-a", "0.5.0"),
                make_package("crate-b", "0.3.0"),
            ];
            let cli_input = ReleaseCliInput {
                graduate_all: true,
                ..Default::default()
            };

            let result = ReleaseValidator::validate(
                &cli_input,
                None,
                None,
                &packages,
                &ProjectKind::VirtualWorkspace,
            );

            assert!(result.is_err());
            let errors = result.expect_err("validation should fail");
            assert!(matches!(
                errors.iter().next().expect("at least one error"),
                ValidationError::GraduateRequiresCratesInWorkspace
            ));
        }

        #[test]
        fn single_package_graduate_without_name_passes() {
            let packages = vec![make_package("my-crate", "0.5.0")];
            let cli_input = ReleaseCliInput {
                graduate_all: true,
                ..Default::default()
            };

            let result = ReleaseValidator::validate(
                &cli_input,
                None,
                None,
                &packages,
                &ProjectKind::SinglePackage,
            );

            assert!(result.is_ok());
        }
    }

    mod package_existence {
        use super::*;

        #[test]
        fn unknown_package_in_prerelease_fails() {
            let packages = vec![make_package("known", "1.0.0")];
            let mut cli_input = ReleaseCliInput::default();
            cli_input
                .cli_prerelease
                .insert("unknown".to_string(), PrereleaseSpec::Alpha);

            let result = ReleaseValidator::validate(
                &cli_input,
                None,
                None,
                &packages,
                &ProjectKind::SinglePackage,
            );

            assert!(result.is_err());
            let errors = result.expect_err("validation should fail");
            assert!(matches!(
                errors.iter().next().expect("at least one error"),
                ValidationError::PackageNotFound { .. }
            ));
        }
    }

    mod graduation_with_prerelease {
        use super::*;

        #[test]
        fn graduation_with_prerelease_toml_succeeds() {
            let packages = vec![make_package("crate-a", "0.5.0")];
            let mut cli_input = ReleaseCliInput::default();
            cli_input.cli_graduate.insert("crate-a".to_string());

            let mut prerelease_state = PrereleaseState::new();
            prerelease_state.insert("crate-a".to_string(), "alpha".to_string());

            let result = ReleaseValidator::validate(
                &cli_input,
                Some(&prerelease_state),
                None,
                &packages,
                &ProjectKind::SinglePackage,
            );

            assert!(
                result.is_ok(),
                "graduation with prerelease TOML should succeed"
            );
            let config = result.expect("validation should pass");
            let pkg_config = config
                .per_package
                .get("crate-a")
                .expect("crate-a should have config");
            assert!(pkg_config.graduate_zero, "should be marked for graduation");
            assert!(
                matches!(pkg_config.prerelease, Some(PrereleaseSpec::Alpha)),
                "should have alpha prerelease tag"
            );
        }
    }

    mod multiple_errors {
        use super::*;

        #[test]
        fn collects_all_errors() {
            let packages = vec![make_package("known", "1.0.0")];
            let mut cli_input = ReleaseCliInput::default();
            cli_input
                .cli_prerelease
                .insert("unknown1".to_string(), PrereleaseSpec::Alpha);
            cli_input.cli_graduate.insert("unknown2".to_string());

            let result = ReleaseValidator::validate(
                &cli_input,
                None,
                None,
                &packages,
                &ProjectKind::SinglePackage,
            );

            assert!(result.is_err());
            let errors = result.expect_err("validation should fail");
            assert_eq!(errors.len(), 2);
        }
    }

    mod toml_prerelease_validation {
        use super::*;

        #[test]
        fn invalid_prerelease_tag_in_toml_fails() {
            let packages = vec![make_package("crate-a", "1.0.0")];
            let cli_input = ReleaseCliInput::default();

            let mut prerelease_state = PrereleaseState::new();
            prerelease_state.insert("crate-a".to_string(), "not-a-valid-tag!!!".to_string());

            let result = ReleaseValidator::validate(
                &cli_input,
                Some(&prerelease_state),
                None,
                &packages,
                &ProjectKind::SinglePackage,
            );

            assert!(result.is_err());
            let errors = result.expect_err("validation should fail");
            assert_eq!(errors.len(), 1);
            assert!(matches!(
                errors.iter().next().expect("at least one error"),
                ValidationError::InvalidPrereleaseTag { .. }
            ));
        }

        #[test]
        fn valid_prerelease_tag_in_toml_passes() {
            let packages = vec![make_package("crate-a", "1.0.0")];
            let cli_input = ReleaseCliInput::default();

            let mut prerelease_state = PrereleaseState::new();
            prerelease_state.insert("crate-a".to_string(), "alpha".to_string());

            let result = ReleaseValidator::validate(
                &cli_input,
                Some(&prerelease_state),
                None,
                &packages,
                &ProjectKind::SinglePackage,
            );

            assert!(result.is_ok());
        }
    }

    mod config_building {
        use super::*;

        #[test]
        fn builds_config_from_all_sources() {
            let packages = vec![
                make_package("crate-a", "0.5.0"),
                make_package("crate-b", "0.3.0"),
                make_package("crate-c", "1.0.0"),
            ];

            let mut cli_input = ReleaseCliInput::default();
            cli_input
                .cli_prerelease
                .insert("crate-a".to_string(), PrereleaseSpec::Beta);

            let mut prerelease_state = PrereleaseState::new();
            prerelease_state.insert("crate-b".to_string(), "alpha".to_string());

            let mut graduation_state = GraduationState::new();
            graduation_state.add("crate-a".to_string());

            let result = ReleaseValidator::validate(
                &cli_input,
                Some(&prerelease_state),
                Some(&graduation_state),
                &packages,
                &ProjectKind::VirtualWorkspace,
            );

            assert!(result.is_ok());
            let config = result.expect("validation should pass");

            let config_a = config
                .per_package
                .get("crate-a")
                .expect("crate-a should have config");
            assert!(matches!(config_a.prerelease, Some(PrereleaseSpec::Beta)));
            assert!(config_a.graduate_zero);

            let config_b = config
                .per_package
                .get("crate-b")
                .expect("crate-b should have config");
            assert!(matches!(config_b.prerelease, Some(PrereleaseSpec::Alpha)));
            assert!(!config_b.graduate_zero);
        }
    }

    mod advanced_error_scenarios {
        use super::*;

        #[test]
        fn collects_three_or_more_errors() {
            let packages = vec![make_package("known", "1.0.0")];
            let mut cli_input = ReleaseCliInput::default();
            cli_input
                .cli_prerelease
                .insert("unknown1".to_string(), PrereleaseSpec::Alpha);
            cli_input
                .cli_prerelease
                .insert("unknown2".to_string(), PrereleaseSpec::Beta);
            cli_input.cli_graduate.insert("unknown3".to_string());

            let result = ReleaseValidator::validate(
                &cli_input,
                None,
                None,
                &packages,
                &ProjectKind::SinglePackage,
            );

            assert!(result.is_err());
            let errors = result.expect_err("validation should fail");
            assert_eq!(errors.len(), 3, "should collect all three errors");
        }

        #[test]
        fn graduation_from_toml_for_prerelease_version_fails() {
            let packages = vec![make_package("crate-a", "0.5.0-alpha.1")];
            let cli_input = ReleaseCliInput::default();

            let mut graduation_state = GraduationState::new();
            graduation_state.add("crate-a".to_string());

            let result = ReleaseValidator::validate(
                &cli_input,
                None,
                Some(&graduation_state),
                &packages,
                &ProjectKind::SinglePackage,
            );

            assert!(result.is_err());
            let errors = result.expect_err("validation should fail");
            assert!(matches!(
                errors.iter().next().expect("at least one error"),
                ValidationError::CannotGraduateFromPrerelease { .. }
            ));
        }

        #[test]
        fn graduation_from_toml_for_stable_version_fails() {
            let packages = vec![make_package("crate-a", "2.0.0")];
            let cli_input = ReleaseCliInput::default();

            let mut graduation_state = GraduationState::new();
            graduation_state.add("crate-a".to_string());

            let result = ReleaseValidator::validate(
                &cli_input,
                None,
                Some(&graduation_state),
                &packages,
                &ProjectKind::SinglePackage,
            );

            assert!(result.is_err());
            let errors = result.expect_err("validation should fail");
            assert!(matches!(
                errors.iter().next().expect("at least one error"),
                ValidationError::CannotGraduateStableVersion { .. }
            ));
        }

        #[test]
        fn graduation_toml_with_prerelease_toml_succeeds() {
            let packages = vec![make_package("crate-a", "0.5.0")];
            let cli_input = ReleaseCliInput::default();

            let mut prerelease_state = PrereleaseState::new();
            prerelease_state.insert("crate-a".to_string(), "alpha".to_string());

            let mut graduation_state = GraduationState::new();
            graduation_state.add("crate-a".to_string());

            let result = ReleaseValidator::validate(
                &cli_input,
                Some(&prerelease_state),
                Some(&graduation_state),
                &packages,
                &ProjectKind::SinglePackage,
            );

            assert!(
                result.is_ok(),
                "graduation TOML + prerelease TOML should succeed"
            );
            let config = result.expect("validation should pass");
            let pkg_config = config
                .per_package
                .get("crate-a")
                .expect("crate-a should have config");
            assert!(pkg_config.graduate_zero, "should be marked for graduation");
            assert!(
                matches!(pkg_config.prerelease, Some(PrereleaseSpec::Alpha)),
                "should have alpha prerelease tag from TOML"
            );
        }

        #[test]
        fn empty_prerelease_tag_in_toml_fails() {
            let packages = vec![make_package("crate-a", "1.0.0")];
            let cli_input = ReleaseCliInput::default();

            let mut prerelease_state = PrereleaseState::new();
            prerelease_state.insert("crate-a".to_string(), String::new());

            let result = ReleaseValidator::validate(
                &cli_input,
                Some(&prerelease_state),
                None,
                &packages,
                &ProjectKind::SinglePackage,
            );

            assert!(result.is_err());
            let errors = result.expect_err("validation should fail");
            assert!(matches!(
                errors.iter().next().expect("at least one error"),
                ValidationError::InvalidPrereleaseTag { .. }
            ));
        }

        #[test]
        fn global_prerelease_applies_to_all_packages() {
            let packages = vec![
                make_package("crate-a", "1.0.0"),
                make_package("crate-b", "2.0.0"),
            ];
            let cli_input = ReleaseCliInput {
                global_prerelease: Some(PrereleaseSpec::Beta),
                ..Default::default()
            };

            let result = ReleaseValidator::validate(
                &cli_input,
                None,
                None,
                &packages,
                &ProjectKind::VirtualWorkspace,
            );

            assert!(result.is_ok());
            let config = result.expect("validation should pass");

            for pkg in &packages {
                let pkg_config = config
                    .per_package
                    .get(&pkg.name)
                    .expect("each package should have config");
                assert!(matches!(pkg_config.prerelease, Some(PrereleaseSpec::Beta)));
            }
        }

        #[test]
        fn graduate_all_applies_to_zero_versions_only() {
            let packages = vec![
                make_package("zero-crate", "0.5.0"),
                make_package("stable-crate", "1.0.0"),
            ];
            let cli_input = ReleaseCliInput {
                graduate_all: true,
                ..Default::default()
            };

            let result = ReleaseValidator::validate(
                &cli_input,
                None,
                None,
                &packages,
                &ProjectKind::SinglePackage,
            );

            assert!(result.is_ok());
            let config = result.expect("validation should pass");

            let zero_config = config.per_package.get("zero-crate");
            assert!(
                zero_config.is_some_and(|c| c.graduate_zero),
                "zero version should graduate"
            );

            let stable_config = config.per_package.get("stable-crate");
            assert!(
                stable_config.is_none() || !stable_config.is_some_and(|c| c.graduate_zero),
                "stable version should not graduate"
            );
        }
    }

    mod validation_error_display {
        use super::*;

        #[test]
        fn conflicting_prerelease_tag_display() {
            let error = ValidationError::ConflictingPrereleaseTag {
                package: "my-crate".to_string(),
                cli_tag: "beta".to_string(),
                toml_tag: "alpha".to_string(),
            };

            let display = error.to_string();

            assert!(display.contains("my-crate"));
            assert!(display.contains("beta"));
            assert!(display.contains("alpha"));
            assert!(display.contains("conflicting"));
        }

        #[test]
        fn conflicting_prerelease_tag_tip() {
            let error = ValidationError::ConflictingPrereleaseTag {
                package: "my-crate".to_string(),
                cli_tag: "beta".to_string(),
                toml_tag: "alpha".to_string(),
            };

            let tip = error.tip();

            assert!(tip.contains("cargo changeset manage pre-release"));
            assert!(tip.contains("--remove my-crate"));
        }

        #[test]
        fn cannot_graduate_from_prerelease_display() {
            let error = ValidationError::CannotGraduateFromPrerelease {
                package: "my-crate".to_string(),
                current_version: "0.5.0-alpha.1".to_string(),
            };

            let display = error.to_string();

            assert!(display.contains("my-crate"));
            assert!(display.contains("prerelease"));
        }

        #[test]
        fn cannot_graduate_from_prerelease_tip() {
            let error = ValidationError::CannotGraduateFromPrerelease {
                package: "my-crate".to_string(),
                current_version: "0.5.0-alpha.1".to_string(),
            };

            let tip = error.tip();

            assert!(tip.contains("release"));
            assert!(tip.contains("my-crate"));
        }

        #[test]
        fn graduate_requires_crates_in_workspace_display() {
            let error = ValidationError::GraduateRequiresCratesInWorkspace;

            let display = error.to_string();

            assert!(display.contains("--graduate"));
            assert!(display.contains("workspace"));
        }

        #[test]
        fn graduate_requires_crates_in_workspace_tip() {
            let error = ValidationError::GraduateRequiresCratesInWorkspace;

            let tip = error.tip();

            assert!(tip.contains("--graduate"));
        }

        #[test]
        fn package_not_found_display() {
            let error = ValidationError::PackageNotFound {
                name: "missing".to_string(),
                available: vec!["crate-a".to_string(), "crate-b".to_string()],
            };

            let display = error.to_string();

            assert!(display.contains("missing"));
            assert!(display.contains("not found"));
        }

        #[test]
        fn package_not_found_tip() {
            let error = ValidationError::PackageNotFound {
                name: "missing".to_string(),
                available: vec!["crate-a".to_string(), "crate-b".to_string()],
            };

            let tip = error.tip();

            assert!(tip.contains("missing"));
            assert!(tip.contains("crate-a"));
            assert!(tip.contains("crate-b"));
        }

        #[test]
        fn cannot_graduate_stable_version_display() {
            let error = ValidationError::CannotGraduateStableVersion {
                package: "my-crate".to_string(),
                version: "2.0.0".to_string(),
            };

            let display = error.to_string();

            assert!(display.contains("my-crate"));
            assert!(display.contains("stable"));
            assert!(display.contains("2.0.0"));
        }

        #[test]
        fn cannot_graduate_stable_version_tip() {
            let error = ValidationError::CannotGraduateStableVersion {
                package: "my-crate".to_string(),
                version: "2.0.0".to_string(),
            };

            let tip = error.tip();

            assert!(tip.contains("my-crate"));
            assert!(tip.contains("0.x"));
        }

        #[test]
        fn invalid_prerelease_tag_display() {
            let error = ValidationError::InvalidPrereleaseTag {
                package: "my-crate".to_string(),
                tag: "bad.tag".to_string(),
                reason: "contains invalid character".to_string(),
            };

            let display = error.to_string();

            assert!(display.contains("my-crate"));
            assert!(display.contains("bad.tag"));
            assert!(display.contains("invalid"));
        }

        #[test]
        fn invalid_prerelease_tag_tip() {
            let error = ValidationError::InvalidPrereleaseTag {
                package: "my-crate".to_string(),
                tag: "bad.tag".to_string(),
                reason: "contains invalid character".to_string(),
            };

            let tip = error.tip();

            assert!(tip.contains("--remove my-crate"));
            assert!(tip.contains("re-add"));
        }
    }

    mod validation_errors_collection {
        use super::*;

        #[test]
        fn from_vec_creates_with_single_error() {
            let errors = vec![ValidationError::GraduateRequiresCratesInWorkspace];

            let collection = ValidationErrors::from_vec(errors);

            assert_eq!(collection.len(), 1);
        }

        #[test]
        fn from_vec_creates_with_multiple_errors() {
            let errors = vec![
                ValidationError::GraduateRequiresCratesInWorkspace,
                ValidationError::PackageNotFound {
                    name: "test".to_string(),
                    available: vec![],
                },
            ];

            let collection = ValidationErrors::from_vec(errors);

            assert_eq!(collection.len(), 2);
        }

        #[test]
        #[should_panic(expected = "at least one error")]
        fn from_vec_panics_on_empty() {
            let errors: Vec<ValidationError> = vec![];
            let _ = ValidationErrors::from_vec(errors);
        }

        #[test]
        fn try_from_vec_returns_none_for_empty() {
            let errors: Vec<ValidationError> = vec![];

            let result = ValidationErrors::try_from_vec(errors);

            assert!(result.is_none());
        }

        #[test]
        fn try_from_vec_returns_some_for_nonempty() {
            let errors = vec![ValidationError::GraduateRequiresCratesInWorkspace];

            let result = ValidationErrors::try_from_vec(errors);

            assert!(result.is_some());
            assert_eq!(result.expect("should have errors").len(), 1);
        }

        #[test]
        fn into_vec_returns_all_errors() {
            let errors = vec![
                ValidationError::GraduateRequiresCratesInWorkspace,
                ValidationError::PackageNotFound {
                    name: "test".to_string(),
                    available: vec![],
                },
            ];

            let collection = ValidationErrors::from_vec(errors);
            let vec = collection.into_vec();

            assert_eq!(vec.len(), 2);
        }

        #[test]
        fn iter_yields_all_errors() {
            let errors = vec![
                ValidationError::GraduateRequiresCratesInWorkspace,
                ValidationError::PackageNotFound {
                    name: "test".to_string(),
                    available: vec![],
                },
            ];

            let collection = ValidationErrors::from_vec(errors);
            let count = collection.iter().count();

            assert_eq!(count, 2);
        }

        #[test]
        fn display_shows_all_errors_with_tips() {
            let errors = vec![
                ValidationError::GraduateRequiresCratesInWorkspace,
                ValidationError::PackageNotFound {
                    name: "test".to_string(),
                    available: vec!["crate-a".to_string()],
                },
            ];

            let collection = ValidationErrors::from_vec(errors);
            let display = collection.to_string();

            assert!(display.contains("2 error(s)"));
            assert!(display.contains("Tip:"));
            assert!(display.contains("--graduate"));
        }

        #[test]
        fn into_iterator_for_owned() {
            let errors = vec![ValidationError::GraduateRequiresCratesInWorkspace];
            let collection = ValidationErrors::from_vec(errors);

            let count = collection.into_iter().count();

            assert_eq!(count, 1);
        }

        #[test]
        fn into_iterator_for_ref() {
            let errors = vec![ValidationError::GraduateRequiresCratesInWorkspace];
            let collection = ValidationErrors::from_vec(errors);

            let count = (&collection).into_iter().count();

            assert_eq!(count, 1);
        }
    }
}
