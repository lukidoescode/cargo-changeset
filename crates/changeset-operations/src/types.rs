use changeset_core::{BumpType, PrereleaseSpec};
use derive_builder::Builder;
use gset::Getset;
use semver::Version;

#[derive(Debug, Clone, PartialEq, Eq, Getset)]
pub struct PackageVersion {
    #[getset(get, vis = "pub")]
    name: String,
    #[getset(get, vis = "pub")]
    current_version: Version,
    #[getset(get, vis = "pub")]
    new_version: Version,
    #[getset(get_copy, vis = "pub")]
    bump_type: BumpType,
    #[getset(get_copy, vis = "pub")]
    auto_bumped: bool,
}

impl PackageVersion {
    pub fn new(
        name: String,
        current_version: Version,
        new_version: Version,
        bump_type: BumpType,
        auto_bumped: bool,
    ) -> Self {
        Self {
            name,
            current_version,
            new_version,
            bump_type,
            auto_bumped,
        }
    }
}

#[derive(Debug, Clone, Default, Builder, Getset)]
#[builder(default)]
pub struct PackageReleaseConfig {
    #[getset(get_as_ref, vis = "pub", ty = "Option<&PrereleaseSpec>")]
    prerelease: Option<PrereleaseSpec>,
    #[getset(get_copy, vis = "pub")]
    graduate_zero: bool,
}

impl PackageReleaseConfig {
    pub fn set_prerelease(&mut self, spec: PrereleaseSpec) {
        self.prerelease = Some(spec);
    }

    pub fn set_graduate_zero(&mut self) {
        self.graduate_zero = true;
    }
}
