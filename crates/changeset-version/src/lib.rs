use changeset_core::BumpType;
use semver::Version;

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
}
