use std::path::PathBuf;

pub trait StartPathProvider: Send + Sync {
    fn start_path(&self) -> std::io::Result<PathBuf>;
}

#[derive(Default, Clone)]
pub struct CurrentDirProvider;

impl StartPathProvider for CurrentDirProvider {
    fn start_path(&self) -> std::io::Result<PathBuf> {
        std::env::current_dir()
    }
}

pub struct FixedPathProvider(PathBuf);

impl FixedPathProvider {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }
}

impl StartPathProvider for FixedPathProvider {
    fn start_path(&self) -> std::io::Result<PathBuf> {
        Ok(self.0.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_dir_provider_returns_current_directory() {
        let provider = CurrentDirProvider;
        let result = provider.start_path();

        assert!(result.is_ok());
        assert!(result.expect("start_path failed").exists());
    }

    #[test]
    fn fixed_path_provider_returns_configured_path() {
        let path = PathBuf::from("/some/test/path");
        let provider = FixedPathProvider::new(path.clone());

        let result = provider.start_path();

        assert!(result.is_ok());
        assert_eq!(result.expect("start_path failed"), path);
    }

    #[test]
    fn fixed_path_provider_accepts_string() {
        let provider = FixedPathProvider::new("/another/path");

        let result = provider.start_path();

        assert_eq!(
            result.expect("start_path failed"),
            PathBuf::from("/another/path")
        );
    }
}
