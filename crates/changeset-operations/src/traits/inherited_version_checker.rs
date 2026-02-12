use std::path::Path;

use changeset_core::PackageInfo;

use crate::Result;

/// Checks whether packages use inherited workspace versions.
pub trait InheritedVersionChecker: Send + Sync {
    /// # Errors
    ///
    /// Returns an error if the manifest cannot be read.
    fn has_inherited_version(&self, manifest_path: &Path) -> Result<bool>;

    /// # Errors
    ///
    /// Returns an error if any manifest cannot be read.
    fn find_packages_with_inherited_versions(
        &self,
        packages: &[PackageInfo],
    ) -> Result<Vec<String>> {
        let mut inherited = Vec::new();
        for pkg in packages {
            let manifest_path = pkg.path.join("Cargo.toml");
            if self.has_inherited_version(&manifest_path)? {
                inherited.push(pkg.name.clone());
            }
        }
        Ok(inherited)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct TestChecker {
        inherited: std::collections::HashSet<PathBuf>,
        fail_on: Option<PathBuf>,
    }

    impl TestChecker {
        fn new() -> Self {
            Self {
                inherited: std::collections::HashSet::new(),
                fail_on: None,
            }
        }

        fn with_inherited(mut self, path: PathBuf) -> Self {
            self.inherited.insert(path);
            self
        }

        fn failing_on(mut self, path: PathBuf) -> Self {
            self.fail_on = Some(path);
            self
        }
    }

    impl InheritedVersionChecker for TestChecker {
        fn has_inherited_version(&self, manifest_path: &Path) -> Result<bool> {
            if let Some(ref fail_path) = self.fail_on {
                if manifest_path == fail_path {
                    return Err(crate::OperationError::Io(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "mock failure",
                    )));
                }
            }
            Ok(self.inherited.contains(manifest_path))
        }
    }

    fn make_package(name: &str, path: &str) -> PackageInfo {
        PackageInfo {
            name: name.to_string(),
            version: "1.0.0".parse().expect("valid version"),
            path: PathBuf::from(path),
        }
    }

    #[test]
    fn find_packages_returns_empty_for_no_inherited() {
        let checker = TestChecker::new();
        let packages = vec![make_package("crate-a", "/pkg/a")];

        let result = checker
            .find_packages_with_inherited_versions(&packages)
            .expect("should succeed");

        assert!(result.is_empty());
    }

    #[test]
    fn find_packages_returns_inherited_package_names() {
        let checker = TestChecker::new()
            .with_inherited(PathBuf::from("/pkg/a/Cargo.toml"))
            .with_inherited(PathBuf::from("/pkg/c/Cargo.toml"));

        let packages = vec![
            make_package("crate-a", "/pkg/a"),
            make_package("crate-b", "/pkg/b"),
            make_package("crate-c", "/pkg/c"),
        ];

        let result = checker
            .find_packages_with_inherited_versions(&packages)
            .expect("should succeed");

        assert_eq!(result, vec!["crate-a", "crate-c"]);
    }

    #[test]
    fn find_packages_propagates_errors() {
        let checker = TestChecker::new().failing_on(PathBuf::from("/pkg/b/Cargo.toml"));

        let packages = vec![
            make_package("crate-a", "/pkg/a"),
            make_package("crate-b", "/pkg/b"),
        ];

        let result = checker.find_packages_with_inherited_versions(&packages);

        assert!(result.is_err());
    }
}
