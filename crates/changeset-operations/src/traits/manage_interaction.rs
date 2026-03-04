use changeset_core::PackageInfo;

use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrereleaseAction {
    Add,
    Remove,
    Graduate,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraduationAction {
    Add,
    Remove,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuSelection<T> {
    Selected(T),
    Cancelled,
}

/// Provides user interaction for prerelease management workflows.
pub trait PrereleaseInteractionProvider: Send + Sync {
    /// # Errors
    ///
    /// Fails on IO errors from the underlying input mechanism.
    fn select_prerelease_action(&self) -> Result<MenuSelection<PrereleaseAction>>;

    /// # Errors
    ///
    /// Fails on IO errors from the underlying input mechanism.
    fn select_package_for_prerelease(
        &self,
        available: &[&PackageInfo],
    ) -> Result<MenuSelection<usize>>;

    /// # Errors
    ///
    /// Fails on IO errors from the underlying input mechanism.
    fn get_prerelease_tag(&self) -> Result<String>;

    /// Presents the list of prerelease packages for removal selection.
    /// Each item is a `(package_name, prerelease_tag)` pair; the
    /// implementation decides how to display them.
    ///
    /// # Errors
    ///
    /// Fails on IO errors from the underlying input mechanism.
    fn select_package_to_remove_prerelease(
        &self,
        items: &[(&str, &str)],
    ) -> Result<MenuSelection<usize>>;
}

/// Provides user interaction for graduation management workflows.
pub trait GraduationInteractionProvider: Send + Sync {
    /// # Errors
    ///
    /// Fails on IO errors from the underlying input mechanism.
    fn select_graduation_action(&self) -> Result<MenuSelection<GraduationAction>>;

    /// # Errors
    ///
    /// Fails on IO errors from the underlying input mechanism.
    fn select_package_for_graduation(
        &self,
        eligible: &[&PackageInfo],
    ) -> Result<MenuSelection<usize>>;

    /// # Errors
    ///
    /// Fails on IO errors from the underlying input mechanism.
    fn select_package_to_remove_graduation(&self, items: &[String])
    -> Result<MenuSelection<usize>>;
}
