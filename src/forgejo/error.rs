use reqwest::StatusCode;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ForgejoApiError {
    #[error("Forgejo API returned {0} with error message: {1}")]
    NoSuccessStatusCodeError(StatusCode, String),
}
