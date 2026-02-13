use changeset_core::{BumpType, PrereleaseSpec};
use semver::{Prerelease, Version};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VersionError {
    #[error("invalid prerelease identifier: {identifier}")]
    InvalidPrerelease { identifier: String },
}

#[must_use]
pub fn max_bump_type(bumps: &[BumpType]) -> Option<BumpType> {
    bumps.iter().max().copied()
}

pub fn bump_version(version: &Version, bump_type: BumpType) -> Version {
    let mut new_version = version.clone();

    match bump_type {
        BumpType::Major => {
            new_version.major += 1;
            new_version.minor = 0;
            new_version.patch = 0;
        }
        BumpType::Minor => {
            new_version.minor += 1;
            new_version.patch = 0;
        }
        BumpType::Patch => {
            new_version.patch += 1;
        }
    }

    new_version.pre = Prerelease::EMPTY;
    new_version
}

fn parse_prerelease(pre: &Prerelease) -> Option<(String, u64)> {
    let pre_str = pre.as_str();
    if pre_str.is_empty() {
        return None;
    }

    let parts: Vec<&str> = pre_str.split('.').collect();
    if parts.len() < 2 {
        return Some((pre_str.to_string(), 1));
    }

    let last = parts.last()?;
    if let Ok(num) = last.parse::<u64>() {
        let tag = parts[..parts.len() - 1].join(".");
        Some((tag, num))
    } else {
        Some((pre_str.to_string(), 1))
    }
}

/// Calculates a new version based on bump type and optional prerelease spec.
///
/// # Errors
///
/// Returns `VersionError::InvalidPrerelease` if the prerelease identifier
/// produces an invalid semver prerelease string.
pub fn calculate_new_version(
    current: &Version,
    bump_type: Option<BumpType>,
    prerelease: Option<&PrereleaseSpec>,
) -> Result<Version, VersionError> {
    let mut new_version = current.clone();

    match prerelease {
        Some(spec) => {
            let tag = spec.identifier();

            if current.pre.is_empty() {
                let bump = bump_type.unwrap_or(BumpType::Patch);
                new_version = bump_version(current, bump);
                new_version.pre = make_prerelease(tag, 1)?;
            } else if let Some((current_tag, current_num)) = parse_prerelease(&current.pre) {
                if current_tag == tag {
                    new_version.pre = make_prerelease(tag, current_num + 1)?;
                } else {
                    new_version.pre = make_prerelease(tag, 1)?;
                }
            } else {
                new_version.pre = make_prerelease(tag, 1)?;
            }
        }
        None => {
            if !current.pre.is_empty() {
                new_version.pre = Prerelease::EMPTY;
            } else if let Some(bump) = bump_type {
                new_version = bump_version(current, bump);
            }
        }
    }

    Ok(new_version)
}

fn make_prerelease(tag: &str, num: u64) -> Result<Prerelease, VersionError> {
    let identifier = format!("{tag}.{num}");
    Prerelease::new(&identifier).map_err(|_| VersionError::InvalidPrerelease { identifier })
}

#[must_use]
pub fn is_prerelease(version: &Version) -> bool {
    !version.pre.is_empty()
}

#[must_use]
pub fn extract_prerelease_tag(version: &Version) -> Option<String> {
    parse_prerelease(&version.pre).map(|(tag, _)| tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bump_patch() {
        let version = Version::parse("1.2.3").unwrap();
        let bumped = bump_version(&version, BumpType::Patch);
        assert_eq!(bumped, Version::parse("1.2.4").unwrap());
    }

    #[test]
    fn test_bump_minor() {
        let version = Version::parse("1.2.3").unwrap();
        let bumped = bump_version(&version, BumpType::Minor);
        assert_eq!(bumped, Version::parse("1.3.0").unwrap());
    }

    #[test]
    fn test_bump_major() {
        let version = Version::parse("1.2.3").unwrap();
        let bumped = bump_version(&version, BumpType::Major);
        assert_eq!(bumped, Version::parse("2.0.0").unwrap());
    }

    #[test]
    fn bump_version_strips_prerelease() {
        let version = Version::parse("1.2.3-alpha.1").unwrap();
        let bumped = bump_version(&version, BumpType::Patch);
        assert_eq!(bumped, Version::parse("1.2.4").unwrap());
    }

    mod max_bump_type_tests {
        use super::*;

        #[test]
        fn returns_none_for_empty_slice() {
            assert_eq!(max_bump_type(&[]), None);
        }

        #[test]
        fn returns_single_element() {
            assert_eq!(max_bump_type(&[BumpType::Patch]), Some(BumpType::Patch));
            assert_eq!(max_bump_type(&[BumpType::Minor]), Some(BumpType::Minor));
            assert_eq!(max_bump_type(&[BumpType::Major]), Some(BumpType::Major));
        }

        #[test]
        fn returns_minor_for_patch_and_minor() {
            assert_eq!(
                max_bump_type(&[BumpType::Patch, BumpType::Minor]),
                Some(BumpType::Minor)
            );
        }

        #[test]
        fn returns_major_for_minor_and_major() {
            assert_eq!(
                max_bump_type(&[BumpType::Minor, BumpType::Major]),
                Some(BumpType::Major)
            );
        }

        #[test]
        fn returns_major_for_all_three() {
            assert_eq!(
                max_bump_type(&[BumpType::Patch, BumpType::Minor, BumpType::Major]),
                Some(BumpType::Major)
            );
        }

        #[test]
        fn handles_duplicates() {
            assert_eq!(
                max_bump_type(&[BumpType::Patch, BumpType::Patch, BumpType::Minor]),
                Some(BumpType::Minor)
            );
        }

        #[test]
        fn order_does_not_matter() {
            assert_eq!(
                max_bump_type(&[BumpType::Major, BumpType::Patch, BumpType::Minor]),
                Some(BumpType::Major)
            );
        }
    }

    mod parse_prerelease_tests {
        use super::*;

        #[test]
        fn empty_prerelease_returns_none() {
            let pre = Prerelease::EMPTY;
            assert!(parse_prerelease(&pre).is_none());
        }

        #[test]
        fn parses_standard_format() {
            let pre = Prerelease::new("alpha.1").unwrap();
            let (tag, num) = parse_prerelease(&pre).unwrap();
            assert_eq!(tag, "alpha");
            assert_eq!(num, 1);
        }

        #[test]
        fn parses_higher_numbers() {
            let pre = Prerelease::new("rc.42").unwrap();
            let (tag, num) = parse_prerelease(&pre).unwrap();
            assert_eq!(tag, "rc");
            assert_eq!(num, 42);
        }

        #[test]
        fn handles_tag_without_number() {
            let pre = Prerelease::new("beta").unwrap();
            let (tag, num) = parse_prerelease(&pre).unwrap();
            assert_eq!(tag, "beta");
            assert_eq!(num, 1);
        }

        #[test]
        fn handles_complex_tag_with_dots() {
            let pre = Prerelease::new("pre.release.3").unwrap();
            let (tag, num) = parse_prerelease(&pre).unwrap();
            assert_eq!(tag, "pre.release");
            assert_eq!(num, 3);
        }
    }

    mod calculate_new_version_tests {
        use super::*;

        #[test]
        fn stable_to_alpha_with_patch() {
            let version = Version::parse("1.0.0").unwrap();
            let result = calculate_new_version(
                &version,
                Some(BumpType::Patch),
                Some(&PrereleaseSpec::Alpha),
            )
            .unwrap();
            assert_eq!(result, Version::parse("1.0.1-alpha.1").unwrap());
        }

        #[test]
        fn stable_to_alpha_with_minor() {
            let version = Version::parse("1.0.0").unwrap();
            let result = calculate_new_version(
                &version,
                Some(BumpType::Minor),
                Some(&PrereleaseSpec::Alpha),
            )
            .unwrap();
            assert_eq!(result, Version::parse("1.1.0-alpha.1").unwrap());
        }

        #[test]
        fn stable_to_alpha_with_major() {
            let version = Version::parse("1.0.0").unwrap();
            let result = calculate_new_version(
                &version,
                Some(BumpType::Major),
                Some(&PrereleaseSpec::Alpha),
            )
            .unwrap();
            assert_eq!(result, Version::parse("2.0.0-alpha.1").unwrap());
        }

        #[test]
        fn alpha_increment_same_tag() {
            let version = Version::parse("1.0.1-alpha.1").unwrap();
            let result =
                calculate_new_version(&version, None, Some(&PrereleaseSpec::Alpha)).unwrap();
            assert_eq!(result, Version::parse("1.0.1-alpha.2").unwrap());
        }

        #[test]
        fn alpha_to_beta_transition() {
            let version = Version::parse("1.0.1-alpha.3").unwrap();
            let result =
                calculate_new_version(&version, None, Some(&PrereleaseSpec::Beta)).unwrap();
            assert_eq!(result, Version::parse("1.0.1-beta.1").unwrap());
        }

        #[test]
        fn beta_to_rc_transition() {
            let version = Version::parse("1.0.1-beta.2").unwrap();
            let result = calculate_new_version(&version, None, Some(&PrereleaseSpec::Rc)).unwrap();
            assert_eq!(result, Version::parse("1.0.1-rc.1").unwrap());
        }

        #[test]
        fn rc_graduate_to_stable() {
            let version = Version::parse("1.0.1-rc.1").unwrap();
            let result = calculate_new_version(&version, None, None).unwrap();
            assert_eq!(result, Version::parse("1.0.1").unwrap());
        }

        #[test]
        fn alpha_graduate_to_stable() {
            let version = Version::parse("1.0.1-alpha.5").unwrap();
            let result = calculate_new_version(&version, None, None).unwrap();
            assert_eq!(result, Version::parse("1.0.1").unwrap());
        }

        #[test]
        fn custom_prerelease_tag() {
            let version = Version::parse("1.0.0").unwrap();
            let spec = PrereleaseSpec::Custom("dev".to_string());
            let result =
                calculate_new_version(&version, Some(BumpType::Patch), Some(&spec)).unwrap();
            assert_eq!(result, Version::parse("1.0.1-dev.1").unwrap());
        }

        #[test]
        fn stable_bump_without_prerelease() {
            let version = Version::parse("1.0.0").unwrap();
            let result = calculate_new_version(&version, Some(BumpType::Minor), None).unwrap();
            assert_eq!(result, Version::parse("1.1.0").unwrap());
        }

        #[test]
        fn stable_no_change_without_bump_or_prerelease() {
            let version = Version::parse("1.0.0").unwrap();
            let result = calculate_new_version(&version, None, None).unwrap();
            assert_eq!(result, Version::parse("1.0.0").unwrap());
        }

        #[test]
        fn prerelease_defaults_to_patch_when_no_bump_specified() {
            let version = Version::parse("1.0.0").unwrap();
            let result =
                calculate_new_version(&version, None, Some(&PrereleaseSpec::Alpha)).unwrap();
            assert_eq!(result, Version::parse("1.0.1-alpha.1").unwrap());
        }
    }

    mod is_prerelease_tests {
        use super::*;

        #[test]
        fn stable_version_is_not_prerelease() {
            let version = Version::parse("1.0.0").unwrap();
            assert!(!is_prerelease(&version));
        }

        #[test]
        fn alpha_version_is_prerelease() {
            let version = Version::parse("1.0.0-alpha.1").unwrap();
            assert!(is_prerelease(&version));
        }

        #[test]
        fn rc_version_is_prerelease() {
            let version = Version::parse("1.0.0-rc.1").unwrap();
            assert!(is_prerelease(&version));
        }
    }

    mod extract_prerelease_tag_tests {
        use super::*;

        #[test]
        fn stable_version_returns_none() {
            let version = Version::parse("1.0.0").unwrap();
            assert!(extract_prerelease_tag(&version).is_none());
        }

        #[test]
        fn extracts_alpha_tag() {
            let version = Version::parse("1.0.0-alpha.1").unwrap();
            assert_eq!(extract_prerelease_tag(&version), Some("alpha".to_string()));
        }

        #[test]
        fn extracts_rc_tag() {
            let version = Version::parse("1.0.0-rc.3").unwrap();
            assert_eq!(extract_prerelease_tag(&version), Some("rc".to_string()));
        }

        #[test]
        fn extracts_custom_tag() {
            let version = Version::parse("1.0.0-nightly.5").unwrap();
            assert_eq!(
                extract_prerelease_tag(&version),
                Some("nightly".to_string())
            );
        }
    }
}
