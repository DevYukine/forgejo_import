use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

const FILE_NAME: &str = "forgejo_import.config.json";
const PROJECT_DIR: &str = "forgejo_import";

/// Optional defaults for CLI arguments. Keys are camelCase.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub forgejo_url: Option<String>,
    pub forgejo_token: Option<String>,
    pub github_token: Option<String>,
    /// Base URL of the GitHub REST API, for GitHub Enterprise Server. Defaults to api.github.com.
    pub github_api_url: Option<String>,
    pub migrate_wiki: Option<bool>,
    pub migrate_lfs: Option<bool>,
}

impl Config {
    /// Loads the first config file found in the default locations, or an empty config.
    pub fn load() -> Result<Config> {
        match candidate_paths().into_iter().find(|path| path.is_file()) {
            Some(path) => Config::load_from(&path),
            None => Ok(Config::default()),
        }
    }

    pub fn load_from(path: &Path) -> Result<Config> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("cannot read config file {}", path.display()))?;

        serde_json::from_str(&text)
            .with_context(|| format!("invalid config file {}", path.display()))
    }
}

/// Lookup order: current directory, `%APPDATA%` (Windows), `$XDG_CONFIG_HOME`, `~/.config`, `~`.
fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(cwd) = env::current_dir() {
        paths.push(cwd.join(FILE_NAME));
    }

    if cfg!(windows)
        && let Some(appdata) = env_dir("APPDATA")
    {
        paths.push(appdata.join(PROJECT_DIR).join(FILE_NAME));
    }

    if let Some(xdg) = env_dir("XDG_CONFIG_HOME") {
        paths.push(xdg.join(PROJECT_DIR).join(FILE_NAME));
    }

    if let Some(home) = home_dir() {
        paths.push(home.join(".config").join(PROJECT_DIR).join(FILE_NAME));
        paths.push(home.join(FILE_NAME));
    }

    paths
}

fn env_dir(key: &str) -> Option<PathBuf> {
    env::var_os(key)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn home_dir() -> Option<PathBuf> {
    if let Some(home) = env_dir("HOME") {
        return Some(home);
    }

    if cfg!(windows) {
        if let Some(profile) = env_dir("USERPROFILE") {
            return Some(profile);
        }

        if let (Some(drive), Some(path)) = (env::var_os("HOMEDRIVE"), env::var_os("HOMEPATH"))
            && !drive.is_empty()
            && !path.is_empty()
        {
            let mut home = drive;
            home.push(path);
            return Some(PathBuf::from(home));
        }
    }

    None
}
