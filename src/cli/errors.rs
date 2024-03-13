use std::io;

use thiserror::Error;

use config::errors::ConfigError;
use forgejo::error::ForgejoApiError;

use crate::{config, forgejo};

#[derive(Error, Debug)]
#[error(transparent)]
pub enum CliError {
    ConfigError(#[from] ConfigError),
    ForgejoApiError(#[from] ForgejoApiError),
    IoError(#[from] io::Error),
    ReqwestError(#[from] reqwest::Error),
    SerdeJsonError(#[from] serde_json::Error),
}
