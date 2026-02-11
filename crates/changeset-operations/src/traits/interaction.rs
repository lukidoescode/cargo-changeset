use changeset_core::{BumpType, ChangeCategory, PackageInfo};

use crate::Result;

#[derive(Debug, Clone)]
pub enum PackageSelection {
    Selected(Vec<PackageInfo>),
    Cancelled,
}

#[derive(Debug, Clone)]
pub enum BumpSelection {
    Selected(BumpType),
    Cancelled,
}

#[derive(Debug, Clone)]
pub enum CategorySelection {
    Selected(ChangeCategory),
    Cancelled,
}

#[derive(Debug, Clone)]
pub enum DescriptionInput {
    Provided(String),
    Cancelled,
}

pub trait InteractionProvider: Send + Sync {
    /// # Errors
    ///
    /// Returns an error if the interaction cannot be completed.
    fn select_packages(&self, available: &[PackageInfo]) -> Result<PackageSelection>;

    /// # Errors
    ///
    /// Returns an error if the interaction cannot be completed.
    fn select_bump_type(&self, package_name: &str) -> Result<BumpSelection>;

    /// # Errors
    ///
    /// Returns an error if the interaction cannot be completed.
    fn select_category(&self) -> Result<CategorySelection>;

    /// # Errors
    ///
    /// Returns an error if the interaction cannot be completed.
    fn get_description(&self) -> Result<DescriptionInput>;
}
