use crate::{Result, TagInfo};

use super::Repository;

impl Repository {
    /// Deletes a tag by name.
    ///
    /// Returns `Ok(true)` if the tag was deleted, `Ok(false)` if the tag was not found.
    ///
    /// # Errors
    ///
    /// Returns an error if the delete operation fails for reasons other than "not found".
    pub fn delete_tag(&self, name: &str) -> Result<bool> {
        let refname = format!("refs/tags/{name}");
        match self.inner.find_reference(&refname) {
            Ok(mut reference) => {
                reference.delete()?;
                Ok(true)
            }
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// # Errors
    ///
    /// Returns an error if the tag cannot be created or already exists.
    pub fn create_tag(&self, name: &str, message: &str) -> Result<TagInfo> {
        let head = self.inner.head()?.peel_to_commit()?;
        let sig = self.inner.signature()?;

        self.inner
            .tag(name, head.as_object(), &sig, message, false)?;

        Ok(TagInfo {
            name: name.to_string(),
            target_sha: head.id().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::setup_test_repo;

    #[test]
    fn create_annotated_tag() -> anyhow::Result<()> {
        let (_dir, repo) = setup_test_repo()?;

        let tag_info = repo.create_tag("v1.0.0", "Release version 1.0.0")?;

        assert_eq!(tag_info.name, "v1.0.0");

        let head = repo.inner.head()?.peel_to_commit()?;
        assert_eq!(tag_info.target_sha, head.id().to_string());

        let tag = repo.inner.find_reference("refs/tags/v1.0.0")?;
        assert!(tag.peel_to_tag().is_ok());

        Ok(())
    }

    #[test]
    fn create_tag_with_crate_prefix() -> anyhow::Result<()> {
        let (_dir, repo) = setup_test_repo()?;

        let tag_info = repo.create_tag("my-crate@v0.1.0", "Release my-crate version 0.1.0")?;

        assert_eq!(tag_info.name, "my-crate@v0.1.0");

        let tag = repo.inner.find_reference("refs/tags/my-crate@v0.1.0")?;
        assert!(tag.peel_to_tag().is_ok());

        Ok(())
    }

    #[test]
    fn duplicate_tag_fails() -> anyhow::Result<()> {
        let (_dir, repo) = setup_test_repo()?;

        repo.create_tag("v1.0.0", "First tag")?;
        let result = repo.create_tag("v1.0.0", "Duplicate tag");

        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn delete_existing_tag_returns_true() -> anyhow::Result<()> {
        let (_dir, repo) = setup_test_repo()?;

        repo.create_tag("v1.0.0", "Tag to delete")?;
        assert!(repo.inner.find_reference("refs/tags/v1.0.0").is_ok());

        let deleted = repo.delete_tag("v1.0.0")?;

        assert!(deleted);
        assert!(repo.inner.find_reference("refs/tags/v1.0.0").is_err());

        Ok(())
    }

    #[test]
    fn delete_nonexistent_tag_returns_false() -> anyhow::Result<()> {
        let (_dir, repo) = setup_test_repo()?;

        let deleted = repo.delete_tag("nonexistent-tag")?;

        assert!(!deleted);

        Ok(())
    }
}
