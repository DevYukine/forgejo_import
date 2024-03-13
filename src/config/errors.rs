use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("You didn't specify the argument {0} in either the CLI or the config file. Please specify it in one of those places.")]
    MissingRequiredArgument(String),
}
