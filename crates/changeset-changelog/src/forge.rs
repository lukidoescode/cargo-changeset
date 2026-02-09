use url::Url;

use crate::error::ChangelogError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forge {
    GitHub,
    GitLab,
    Bitbucket,
    Gitea,
    SourceHut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryInfo {
    pub forge: Forge,
    pub owner: String,
    pub repo: String,
    pub base_url: Url,
}

impl RepositoryInfo {
    /// # Errors
    ///
    /// Returns `ChangelogError::UrlParse` if the URL is invalid or missing required path segments.
    pub fn from_url(url_str: &str) -> Result<Self, ChangelogError> {
        let url = Url::parse(url_str).map_err(|source| ChangelogError::UrlParse {
            url: url_str.to_string(),
            source,
        })?;

        let host = url.host_str().ok_or_else(|| ChangelogError::UrlParse {
            url: url_str.to_string(),
            source: url::ParseError::EmptyHost,
        })?;

        let forge = detect_forge(host);
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
    pub fn comparison_url(&self, base_tag: &str, target_tag: &str) -> String {
        match self.forge {
            Forge::GitHub | Forge::Gitea => format!(
                "{}{}/{}/compare/{}...{}",
                self.base_url, self.owner, self.repo, base_tag, target_tag
            ),
            Forge::GitLab => format!(
                "{}{}/{}/-/compare/{}...{}",
                self.base_url, self.owner, self.repo, base_tag, target_tag
            ),
            Forge::Bitbucket => format!(
                "{}{}/{}/branches/compare/{}..{}",
                self.base_url, self.owner, self.repo, target_tag, base_tag
            ),
            Forge::SourceHut => format!(
                "{}~{}/{}/log/{}..{}",
                self.base_url, self.owner, self.repo, base_tag, target_tag
            ),
        }
    }
}

fn detect_forge(host: &str) -> Forge {
    let host_lower = host.to_lowercase();

    if host_lower == "github.com" || host_lower.ends_with(".github.com") {
        Forge::GitHub
    } else if host_lower == "gitlab.com"
        || host_lower.starts_with("gitlab.")
        || host_lower.contains(".gitlab.")
    {
        Forge::GitLab
    } else if host_lower == "bitbucket.org" || host_lower.ends_with(".bitbucket.org") {
        Forge::Bitbucket
    } else if host_lower == "codeberg.org" || host_lower.starts_with("gitea.") {
        Forge::Gitea
    } else if host_lower == "git.sr.ht" || host_lower.ends_with(".sr.ht") {
        Forge::SourceHut
    } else {
        Forge::GitHub
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
        assert_eq!(info.forge, Forge::GitHub);
        assert_eq!(info.owner, "owner");
        assert_eq!(info.repo, "repo");
    }

    #[test]
    fn detect_gitlab_from_url() {
        let info = RepositoryInfo::from_url("https://gitlab.com/owner/repo").expect("should parse");
        assert_eq!(info.forge, Forge::GitLab);
        assert_eq!(info.owner, "owner");
        assert_eq!(info.repo, "repo");
    }

    #[test]
    fn detect_bitbucket_from_url() {
        let info =
            RepositoryInfo::from_url("https://bitbucket.org/owner/repo").expect("should parse");
        assert_eq!(info.forge, Forge::Bitbucket);
        assert_eq!(info.owner, "owner");
        assert_eq!(info.repo, "repo");
    }

    #[test]
    fn detect_codeberg_as_gitea() {
        let info =
            RepositoryInfo::from_url("https://codeberg.org/owner/repo").expect("should parse");
        assert_eq!(info.forge, Forge::Gitea);
    }

    #[test]
    fn detect_sourcehut_from_url() {
        let info = RepositoryInfo::from_url("https://git.sr.ht/~owner/repo").expect("should parse");
        assert_eq!(info.forge, Forge::SourceHut);
        assert_eq!(info.owner, "owner");
        assert_eq!(info.repo, "repo");
    }

    #[test]
    fn strip_git_suffix_from_url() {
        let info =
            RepositoryInfo::from_url("https://github.com/owner/repo.git").expect("should parse");
        assert_eq!(info.repo, "repo");
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
        assert_eq!(info.forge, Forge::GitLab);
        assert_eq!(info.owner, "team");
        assert_eq!(info.repo, "project");
    }

    #[test]
    fn unknown_host_defaults_to_github() {
        let info =
            RepositoryInfo::from_url("https://example.com/owner/repo").expect("should parse");
        assert_eq!(info.forge, Forge::GitHub);
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
