use std::env;
use std::path::PathBuf;

use tokio::fs;

use errors::ConfigError;

use crate::cli::Commands;
use crate::config::constants::{
    CONFIG_FILE_NAME, CONFIG_PATH, HOME_ENV, PROJECT_NAME, WINDOWS_APPDATA_ENV,
    WINDOWS_HOMEDRIVE_ENV, WINDOWS_HOMEPATH_ENV, WINDOWS_USERPROFILE_ENV, XDG_CONFIG_ENV,
};
use crate::config::models::ForgejoImportConfig;

mod constants;
pub(crate) mod errors;
mod models;

pub async fn apply_config(command: &mut Commands) -> anyhow::Result<()> {
    let config_path = search_config_in_default_locations()?;

    if let Some(config_path) = config_path {
        let config =
            serde_json::from_str::<ForgejoImportConfig>(&fs::read_to_string(config_path).await?)?;

        match command {
            Commands::MirrorOrg(cmd) => {
                if cmd.forgejo_url.is_none() {
                    cmd.forgejo_url = config.forgejo_url;
                }

                if cmd.forgejo_token.is_none() {
                    cmd.forgejo_token = config.forgejo_token;
                }

                if cmd.github_token.is_none() {
                    cmd.github_token = config.github_token;
                }

                if let Some(migrate_wiki) = config.migrate_wiki {
                    cmd.migrate_wiki = migrate_wiki;
                }

                if let Some(migrate_lfs) = config.migrate_lfs {
                    cmd.migrate_lfs = migrate_lfs;
                }
            }
            Commands::MirrorUser(cmd) => {
                if cmd.forgejo_url.is_none() {
                    cmd.forgejo_url = config.forgejo_url;
                }

                if cmd.forgejo_token.is_none() {
                    cmd.forgejo_token = config.forgejo_token;
                }

                if cmd.github_token.is_none() {
                    cmd.github_token = config.github_token;
                }

                if let Some(migrate_wiki) = config.migrate_wiki {
                    cmd.migrate_wiki = migrate_wiki;
                }

                if let Some(migrate_lfs) = config.migrate_lfs {
                    cmd.migrate_lfs = migrate_lfs;
                }
            }
            Commands::MirrorRepo(cmd) => {
                if cmd.forgejo_url.is_none() {
                    cmd.forgejo_url = config.forgejo_url;
                }

                if cmd.forgejo_token.is_none() {
                    cmd.forgejo_token = config.forgejo_token;
                }

                if cmd.github_token.is_none() {
                    cmd.github_token = config.github_token;
                }

                if let Some(migrate_wiki) = config.migrate_wiki {
                    cmd.migrate_wiki = migrate_wiki;
                }

                if let Some(migrate_lfs) = config.migrate_lfs {
                    cmd.migrate_lfs = migrate_lfs;
                }
            }
            Commands::DeleteOrg(cmd) => {
                if cmd.forgejo_url.is_none() {
                    cmd.forgejo_url = config.forgejo_url;
                }

                if cmd.forgejo_token.is_none() {
                    cmd.forgejo_token = config.forgejo_token;
                }
            }
        }
    }

    validate_cmd(command)?;

    Ok(())
}

fn validate_cmd(cmd: &Commands) -> anyhow::Result<()> {
    match cmd {
        Commands::MirrorOrg(cmd) => {
            if cmd.forgejo_url.is_none() {
                return Err(ConfigError::MissingRequiredArgument("forgejo-url".to_string()).into());
            }

            if cmd.forgejo_token.is_none() {
                return Err(
                    ConfigError::MissingRequiredArgument("forgejo-token".to_string()).into(),
                );
            }

            if cmd.github_token.is_none() {
                return Err(
                    ConfigError::MissingRequiredArgument("github-token".to_string()).into(),
                );
            }
        }
        Commands::MirrorUser(cmd) => {
            if cmd.forgejo_url.is_none() {
                return Err(ConfigError::MissingRequiredArgument("forgejo-url".to_string()).into());
            }

            if cmd.forgejo_token.is_none() {
                return Err(
                    ConfigError::MissingRequiredArgument("forgejo-token".to_string()).into(),
                );
            }

            if cmd.github_token.is_none() {
                return Err(
                    ConfigError::MissingRequiredArgument("github-token".to_string()).into(),
                );
            }
        }
        Commands::MirrorRepo(cmd) => {
            if cmd.forgejo_url.is_none() {
                return Err(ConfigError::MissingRequiredArgument("forgejo-url".to_string()).into());
            }

            if cmd.forgejo_token.is_none() {
                return Err(
                    ConfigError::MissingRequiredArgument("forgejo-token".to_string()).into(),
                );
            }

            if cmd.github_token.is_none() {
                return Err(
                    ConfigError::MissingRequiredArgument("github-token".to_string()).into(),
                );
            }
        }
        Commands::DeleteOrg(cmd) => {
            if cmd.forgejo_url.is_none() {
                return Err(ConfigError::MissingRequiredArgument("forgejo-url".to_string()).into());
            }

            if cmd.forgejo_token.is_none() {
                return Err(
                    ConfigError::MissingRequiredArgument("forgejo-token".to_string()).into(),
                );
            }
        }
    }

    Ok(())
}

fn search_config_in_default_locations() -> anyhow::Result<Option<PathBuf>> {
    let mut path;

    let current_dir = env::current_dir()?;
    let current_dir_config = current_dir.join(CONFIG_FILE_NAME);

    if current_dir_config.exists() {
        path = Some(current_dir_config);
        return Ok(path);
    }

    if cfg!(windows) {
        path = get_config_path_from_default_location_by_env(WINDOWS_APPDATA_ENV);

        if let Some(path) = path {
            return Ok(Some(path));
        }
    }

    path = get_config_path_from_default_location_by_env(XDG_CONFIG_ENV);

    if let Some(path) = path {
        return Ok(Some(path));
    }

    let home_env = get_home_env();

    if let Some(home) = home_env {
        let mut home_config = PathBuf::from(home.clone())
            .join(CONFIG_PATH)
            .join(PROJECT_NAME)
            .join(CONFIG_FILE_NAME);

        if home_config.exists() {
            path = Some(home_config);
            return Ok(path);
        }

        home_config = PathBuf::from(home).join(CONFIG_FILE_NAME);

        if home_config.exists() {
            path = Some(home_config);
            return Ok(path);
        }
    }

    Ok(path)
}

fn get_config_path_from_default_location_by_env(env: &str) -> Option<PathBuf> {
    let env_resolved = env::var(env).unwrap_or(String::new());

    if !env_resolved.is_empty() {
        let env_config_home = PathBuf::from(env_resolved)
            .join(PROJECT_NAME)
            .join(CONFIG_FILE_NAME);

        if env_config_home.exists() {
            return Some(env_config_home);
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn get_home_env() -> Option<String> {
    if let Ok(home) = env::var(HOME_ENV) {
        return Some(home);
    }

    if let Ok(user_profile) = env::var(WINDOWS_USERPROFILE_ENV) {
        return Some(user_profile);
    }

    let home_drive = env::var(WINDOWS_HOMEDRIVE_ENV).unwrap_or(String::new());
    let home_path = env::var(WINDOWS_HOMEPATH_ENV).unwrap_or(String::new());

    if !home_drive.is_empty() && !home_path.is_empty() {
        return Some(format!("{}\\{}", home_drive, home_path));
    }

    None
}

#[cfg(not(target_os = "windows"))]
fn get_home_env() -> Option<String> {
    if let Ok(home) = env::var(HOME_ENV) {
        return Some(home);
    }

    None
}
