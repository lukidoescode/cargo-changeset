use std::fmt::Debug;

use url::Url;

use crate::error::ChangelogError;

pub(crate) trait ForgeStrategy: Send + Sync + Debug {
    fn name(&self) -> &'static str;
    fn matches_host(&self, host: &str) -> bool;
    fn comparison_url(
        &self,
        base_url: &Url,
        owner: &str,
        repo: &str,
        base_tag: &str,
        target_tag: &str,
    ) -> String;
}

#[derive(Debug)]
pub(crate) struct GitHub;

impl ForgeStrategy for GitHub {
    fn name(&self) -> &'static str {
        "GitHub"
    }

    fn matches_host(&self, host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        host == "github.com" || host.ends_with(".github.com")
    }

    fn comparison_url(
        &self,
        base_url: &Url,
        owner: &str,
        repo: &str,
        base_tag: &str,
        target_tag: &str,
    ) -> String {
        format!("{base_url}{owner}/{repo}/compare/{base_tag}...{target_tag}")
    }
}

#[derive(Debug)]
pub(crate) struct GitLab;

impl ForgeStrategy for GitLab {
    fn name(&self) -> &'static str {
        "GitLab"
    }

    fn matches_host(&self, host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        host == "gitlab.com" || host.starts_with("gitlab.") || host.contains(".gitlab.")
    }

    fn comparison_url(
        &self,
        base_url: &Url,
        owner: &str,
        repo: &str,
        base_tag: &str,
        target_tag: &str,
    ) -> String {
        format!("{base_url}{owner}/{repo}/-/compare/{base_tag}...{target_tag}")
    }
}

#[derive(Debug)]
pub(crate) struct Bitbucket;

impl ForgeStrategy for Bitbucket {
    fn name(&self) -> &'static str {
        "Bitbucket"
    }

    fn matches_host(&self, host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        host == "bitbucket.org" || host.ends_with(".bitbucket.org")
    }

    fn comparison_url(
        &self,
        base_url: &Url,
        owner: &str,
        repo: &str,
        base_tag: &str,
        target_tag: &str,
    ) -> String {
        format!("{base_url}{owner}/{repo}/branches/compare/{target_tag}..{base_tag}")
    }
}

#[derive(Debug)]
pub(crate) struct Gitea;

impl ForgeStrategy for Gitea {
    fn name(&self) -> &'static str {
        "Gitea"
    }

    fn matches_host(&self, host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        host == "codeberg.org" || host.starts_with("gitea.")
    }

    fn comparison_url(
        &self,
        base_url: &Url,
        owner: &str,
        repo: &str,
        base_tag: &str,
        target_tag: &str,
    ) -> String {
        format!("{base_url}{owner}/{repo}/compare/{base_tag}...{target_tag}")
    }
}

#[derive(Debug)]
pub(crate) struct SourceHut;

impl ForgeStrategy for SourceHut {
    fn name(&self) -> &'static str {
        "SourceHut"
    }

    fn matches_host(&self, host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        host == "git.sr.ht" || host.ends_with(".sr.ht")
    }

    fn comparison_url(
        &self,
        base_url: &Url,
        owner: &str,
        repo: &str,
        base_tag: &str,
        target_tag: &str,
    ) -> String {
        format!("{base_url}~{owner}/{repo}/log/{base_tag}..{target_tag}")
    }
}

static FORGES: &[&dyn ForgeStrategy] = &[&GitHub, &GitLab, &Bitbucket, &Gitea, &SourceHut];

fn detect_forge(host: &str) -> Option<&'static dyn ForgeStrategy> {
    FORGES.iter().find(|f| f.matches_host(host)).copied()
}

pub struct RepositoryInfo {
    forge: &'static dyn ForgeStrategy,
    owner: String,
    repo: String,
    base_url: Url,
}

impl std::fmt::Debug for RepositoryInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepositoryInfo")
            .field("forge", &self.forge.name())
            .field("owner", &self.owner)
            .field("repo", &self.repo)
            .field("base_url", &self.base_url)
            .finish()
    }
}

impl Clone for RepositoryInfo {
    fn clone(&self) -> Self {
        Self {
            forge: self.forge,
            owner: self.owner.clone(),
            repo: self.repo.clone(),
            base_url: self.base_url.clone(),
        }
    }
}

impl PartialEq for RepositoryInfo {
    fn eq(&self, other: &Self) -> bool {
        self.forge.name() == other.forge.name()
            && self.owner == other.owner
            && self.repo == other.repo
            && self.base_url == other.base_url
    }
}

impl Eq for RepositoryInfo {}

impl RepositoryInfo {
    /// # Errors
    ///
    /// Fails if the URL cannot be parsed or is missing required path segments.
    pub fn from_url(url_str: &str) -> Result<Self, ChangelogError> {
        let url = Url::parse(url_str).map_err(|source| ChangelogError::UrlParse {
            url: url_str.to_string(),
            source,
        })?;

        let host = url.host_str().ok_or_else(|| ChangelogError::MissingHost {
            url: url_str.to_string(),
        })?;

        let forge = detect_forge(host).unwrap_or(&GitHub);
        let (owner, repo) = extract_owner_repo(&url)?;

        let base_url = Url::parse(&format!("{}://{}", url.scheme(), host)).map_err(|source| {
            ChangelogError::UrlParse {
                url: url_str.to_string(),
                source,
            }
        })?;

        Ok(Self {
            forge,
            owner,
            repo,
            base_url,
        })
    }

    #[must_use]
    pub fn forge_name(&self) -> &'static str {
        self.forge.name()
    }

    #[must_use]
    pub fn owner(&self) -> &str {
        &self.owner
    }

    #[must_use]
    pub fn repo(&self) -> &str {
        &self.repo
    }

    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    #[must_use]
    pub fn comparison_url(&self, base_tag: &str, target_tag: &str) -> String {
        self.forge.comparison_url(
            &self.base_url,
            &self.owner,
            &self.repo,
            base_tag,
            target_tag,
        )
    }
}

fn extract_owner_repo(url: &Url) -> Result<(String, String), ChangelogError> {
    let path = url.path().trim_start_matches('/').trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);

    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if segments.len() < 2 {
        return Err(ChangelogError::InvalidRepositoryPath {
            url: url.to_string(),
        });
    }

    let owner = segments[0].trim_start_matches('~').to_string();
    let repo = segments[1].to_string();

    Ok((owner, repo))
}

#[must_use]
pub fn expand_comparison_template(
    template: &str,
    repository: &str,
    base_tag: &str,
    target_tag: &str,
) -> String {
    template
        .replace("{repository}", repository)
        .replace("{base}", base_tag)
        .replace("{target}", target_tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_github_from_url() {
        let info = RepositoryInfo::from_url("https://github.com/owner/repo").expect("should parse");
        assert_eq!(info.forge_name(), "GitHub");
        assert_eq!(info.owner(), "owner");
        assert_eq!(info.repo(), "repo");
    }

    #[test]
    fn detect_gitlab_from_url() {
        let info = RepositoryInfo::from_url("https://gitlab.com/owner/repo").expect("should parse");
        assert_eq!(info.forge_name(), "GitLab");
        assert_eq!(info.owner(), "owner");
        assert_eq!(info.repo(), "repo");
    }

    #[test]
    fn detect_bitbucket_from_url() {
        let info =
            RepositoryInfo::from_url("https://bitbucket.org/owner/repo").expect("should parse");
        assert_eq!(info.forge_name(), "Bitbucket");
        assert_eq!(info.owner(), "owner");
        assert_eq!(info.repo(), "repo");
    }

    #[test]
    fn detect_codeberg_as_gitea() {
        let info =
            RepositoryInfo::from_url("https://codeberg.org/owner/repo").expect("should parse");
        assert_eq!(info.forge_name(), "Gitea");
    }

    #[test]
    fn detect_sourcehut_from_url() {
        let info = RepositoryInfo::from_url("https://git.sr.ht/~owner/repo").expect("should parse");
        assert_eq!(info.forge_name(), "SourceHut");
        assert_eq!(info.owner(), "owner");
        assert_eq!(info.repo(), "repo");
    }

    #[test]
    fn strip_git_suffix_from_url() {
        let info =
            RepositoryInfo::from_url("https://github.com/owner/repo.git").expect("should parse");
        assert_eq!(info.repo(), "repo");
    }

    #[test]
    fn github_comparison_url() {
        let info = RepositoryInfo::from_url("https://github.com/owner/repo").expect("should parse");
        let url = info.comparison_url("v1.0.0", "v1.1.0");
        assert_eq!(url, "https://github.com/owner/repo/compare/v1.0.0...v1.1.0");
    }

    #[test]
    fn gitlab_comparison_url() {
        let info = RepositoryInfo::from_url("https://gitlab.com/owner/repo").expect("should parse");
        let url = info.comparison_url("v1.0.0", "v1.1.0");
        assert_eq!(
            url,
            "https://gitlab.com/owner/repo/-/compare/v1.0.0...v1.1.0"
        );
    }

    #[test]
    fn bitbucket_comparison_url_reversed() {
        let info =
            RepositoryInfo::from_url("https://bitbucket.org/owner/repo").expect("should parse");
        let url = info.comparison_url("v1.0.0", "v1.1.0");
        assert_eq!(
            url,
            "https://bitbucket.org/owner/repo/branches/compare/v1.1.0..v1.0.0"
        );
    }

    #[test]
    fn sourcehut_comparison_url() {
        let info = RepositoryInfo::from_url("https://git.sr.ht/~owner/repo").expect("should parse");
        let url = info.comparison_url("v1.0.0", "v1.1.0");
        assert_eq!(url, "https://git.sr.ht/~owner/repo/log/v1.0.0..v1.1.0");
    }

    #[test]
    fn expand_custom_template() {
        let template = "https://my-forge.example.com/{repository}/compare/{base}...{target}";
        let result = expand_comparison_template(template, "owner/repo", "v1.0.0", "v1.1.0");
        assert_eq!(
            result,
            "https://my-forge.example.com/owner/repo/compare/v1.0.0...v1.1.0"
        );
    }

    #[test]
    fn error_invalid_url() {
        let result = RepositoryInfo::from_url("not-a-valid-url");
        assert!(result.is_err());
    }

    #[test]
    fn error_missing_repo_path() {
        let result = RepositoryInfo::from_url("https://github.com/");
        assert!(result.is_err());
    }

    #[test]
    fn self_hosted_gitlab() {
        let info = RepositoryInfo::from_url("https://gitlab.mycompany.com/team/project")
            .expect("should parse");
        assert_eq!(info.forge_name(), "GitLab");
        assert_eq!(info.owner(), "team");
        assert_eq!(info.repo(), "project");
    }

    #[test]
    fn unknown_host_defaults_to_github() {
        let info =
            RepositoryInfo::from_url("https://example.com/owner/repo").expect("should parse");
        assert_eq!(info.forge_name(), "GitHub");
    }

    #[test]
    fn error_single_path_segment() {
        let result = RepositoryInfo::from_url("https://github.com/owner");
        assert!(matches!(
            result,
            Err(ChangelogError::InvalidRepositoryPath { .. })
        ));
    }

    #[test]
    fn sourcehut_produces_single_tilde_in_url() {
        let info = RepositoryInfo::from_url("https://git.sr.ht/~owner/repo").expect("should parse");
        let url = info.comparison_url("v1.0.0", "v1.1.0");

        assert!(
            url.contains("/~owner/") && !url.contains("/~~"),
            "URL should contain single tilde: {url}"
        );
    }
}
