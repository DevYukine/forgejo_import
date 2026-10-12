use serde::Deserialize;

/// Shared shape of `GET /orgs/{org}` and `GET /users/{user}`; fields that only one of them returns are optional.
#[derive(Debug, Deserialize)]
pub struct GithubAccount {
    pub login: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub blog: Option<String>,
    pub html_url: String,
    pub avatar_url: String,
}

#[derive(Debug, Deserialize)]
pub struct GithubRepository {
    pub name: String,
    pub description: Option<String>,
    pub clone_url: String,
    pub private: bool,
    pub size: i64,
}
