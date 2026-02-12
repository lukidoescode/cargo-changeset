use crate::{Repository, Result};

impl Repository {
    /// # Errors
    ///
    /// Returns an error if the remote lookup fails.
    pub fn remote_url(&self) -> Result<Option<String>> {
        let Ok(remote) = self.inner.find_remote("origin") else {
            return Ok(None);
        };

        Ok(remote.url().map(String::from))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::tests::setup_test_repo;

    #[test]
    fn remote_url_returns_none_when_no_remote() -> anyhow::Result<()> {
        let (_dir, repo) = setup_test_repo()?;

        let url = repo.remote_url()?;

        assert!(url.is_none());

        Ok(())
    }

    #[test]
    fn remote_url_returns_url_when_present() -> anyhow::Result<()> {
        let (dir, repo) = setup_test_repo()?;

        repo.inner
            .remote("origin", "https://github.com/owner/repo")?;

        let repository = Repository::open(dir.path())?;
        let url = repository.remote_url()?;

        assert_eq!(url.as_deref(), Some("https://github.com/owner/repo"));

        Ok(())
    }
}
