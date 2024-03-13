use reqwest::StatusCode;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GithubApiError {
    #[error("Github API returned {0} with error message: {1}")]
    NoSuccessStatusCodeError(StatusCode, String),
}
