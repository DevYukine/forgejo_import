use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ForgejoMigrateRepoService {
    Github,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ForgejoVisibility {
    Public,
    Limited,
    Private,
}

#[derive(Debug, Clone, Serialize)]
pub struct ForgejoMigrateRepositoryRequest {
    pub auth_token: String,
    pub clone_addr: String,
    pub description: Option<String>,
    pub issues: bool,
    pub labels: bool,
    pub lfs: bool,
    pub lfs_endpoint: Option<String>,
    pub milestones: bool,
    pub mirror: bool,
    pub mirror_interval: Option<String>,
    pub private: bool,
    pub pull_requests: bool,
    pub releases: bool,
    pub repo_name: String,
    pub repo_owner: String,
    pub service: ForgejoMigrateRepoService,
    pub wiki: bool,
}

#[derive(Debug, Serialize)]
pub struct ForgejoCreateOrganisationRequest {
    pub description: Option<String>,
    pub email: Option<String>,
    pub full_name: Option<String>,
    pub location: Option<String>,
    pub repo_admin_change_team_access: Option<bool>,
    pub username: String,
    pub visibility: Option<ForgejoVisibility>,
    pub website: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ForgejoRepository {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct ForgejoUpdateUserAvatarRequest {
    pub image: String,
}
