use changeset_core::BumpType;
use semver::Version;

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

    new_version
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
}
