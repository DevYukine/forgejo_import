use anyhow::Result;
use bytes::Bytes;
use log::warn;
use reqwest::Url;
use serde::Deserialize;

use crate::http::RestClient;

const API_URL: &str = "https://api.github.com";
const PAGE_SIZE: &str = "100";

/// Shared shape of `GET /orgs/{org}` and `GET /users/{user}`; fields only one of them returns are optional.
#[derive(Debug, Deserialize)]
pub struct Account {
    pub login: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub blog: Option<String>,
    pub html_url: String,
    pub avatar_url: String,
}

#[derive(Debug, Deserialize)]
pub struct Repository {
    pub name: String,
    pub description: Option<String>,
    pub clone_url: String,
    pub private: bool,
    pub size: i64,
}

pub struct GithubApi {
    rest: RestClient,
}

impl GithubApi {
    pub fn new(token: String) -> Result<Self> {
        Self::with_base_url(Url::parse(API_URL)?, token)
    }

    /// `base` is the REST API root, e.g. `https://ghe.example.com/api/v3` for GitHub Enterprise Server.
    pub fn with_base_url(base: Url, token: String) -> Result<Self> {
        Ok(Self {
            rest: RestClient::new("GitHub", base, token)?,
        })
    }

    pub fn token(&self) -> &str {
        self.rest.token()
    }

    pub async fn organisation(&self, org: &str) -> Result<Account> {
        self.rest.get(&["orgs", org]).await
    }

    pub async fn user(&self, user: &str) -> Result<Account> {
        self.rest.get(&["users", user]).await
    }

    pub async fn repository(&self, owner: &str, name: &str) -> Result<Repository> {
        self.rest.get(&["repos", owner, name]).await
    }

    /// All repositories of an organisation the token can see, smallest first.
    pub async fn organisation_repositories(&self, org: &str) -> Result<Vec<Repository>> {
        self.repositories(&["orgs", org, "repos"], &[("per_page", PAGE_SIZE)])
            .await
    }

    /// Repositories of a user, smallest first. `GET /users/{user}/repos` only lists public ones,
    /// so when the token belongs to that user the private repositories come from `GET /user/repos`.
    pub async fn user_repositories(&self, user: &str) -> Result<Vec<Repository>> {
        let owns_token = match self.rest.get::<Account>(&["user"]).await {
            Ok(account) => account.login.eq_ignore_ascii_case(user),
            Err(err) => {
                warn!(
                    "Cannot tell whom the GitHub token belongs to, listing only public repositories: {err:#}"
                );
                false
            }
        };

        if owns_token {
            self.repositories(
                &["user", "repos"],
                &[("affiliation", "owner"), ("per_page", PAGE_SIZE)],
            )
            .await
        } else {
            self.repositories(&["users", user, "repos"], &[("per_page", PAGE_SIZE)])
                .await
        }
    }

    async fn repositories(&self, path: &[&str], query: &[(&str, &str)]) -> Result<Vec<Repository>> {
        let mut repos: Vec<Repository> = self.rest.get_all(path, query).await?;
        repos.sort_by_key(|repo| repo.size);

        Ok(repos)
    }

    /// Avatars live on a CDN host, so they are fetched without the API token.
    pub async fn avatar(&self, url: &str) -> Result<Bytes> {
        self.rest.download(url).await
    }
}
