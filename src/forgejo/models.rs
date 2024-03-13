use clap::ValueEnum;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ForgejoMigrateRepoService {
    Git,
    Github,
    Gitea,
    Gitlab,
    Gogs,
    OneDev,
    Gitbucket,
    Codebase,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ForgejoVisibility {
    Public,
    Limited,
    Private,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
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

pub type ForgejoGetOrganisationRepositoriesResponse = Vec<ForgejoRepository>;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgejoRepository {
    pub id: i64,
    pub owner: ForgejoOwner,
    pub name: String,
    #[serde(rename = "full_name")]
    pub full_name: String,
    pub description: String,
    pub empty: bool,
    pub private: bool,
    pub fork: bool,
    pub template: bool,
    pub mirror: bool,
    pub size: i64,
    pub language: String,
    #[serde(rename = "languages_url")]
    pub languages_url: String,
    #[serde(rename = "html_url")]
    pub html_url: String,
    pub url: String,
    pub link: String,
    #[serde(rename = "ssh_url")]
    pub ssh_url: String,
    #[serde(rename = "clone_url")]
    pub clone_url: String,
    #[serde(rename = "original_url")]
    pub original_url: String,
    pub website: String,
    #[serde(rename = "stars_count")]
    pub stars_count: i64,
    #[serde(rename = "forks_count")]
    pub forks_count: i64,
    #[serde(rename = "watchers_count")]
    pub watchers_count: i64,
    #[serde(rename = "open_issues_count")]
    pub open_issues_count: i64,
    #[serde(rename = "open_pr_counter")]
    pub open_pr_counter: i64,
    #[serde(rename = "release_counter")]
    pub release_counter: i64,
    #[serde(rename = "default_branch")]
    pub default_branch: String,
    pub archived: bool,
    #[serde(rename = "created_at")]
    pub created_at: String,
    #[serde(rename = "updated_at")]
    pub updated_at: String,
    #[serde(rename = "archived_at")]
    pub archived_at: String,
    pub permissions: ForgejoPermissions,
    #[serde(rename = "has_issues")]
    pub has_issues: bool,
    #[serde(rename = "internal_tracker")]
    pub internal_tracker: ForgejoInternalTracker,
    #[serde(rename = "has_wiki")]
    pub has_wiki: bool,
    #[serde(rename = "has_pull_requests")]
    pub has_pull_requests: bool,
    #[serde(rename = "has_projects")]
    pub has_projects: bool,
    #[serde(rename = "has_releases")]
    pub has_releases: bool,
    #[serde(rename = "has_packages")]
    pub has_packages: bool,
    #[serde(rename = "has_actions")]
    pub has_actions: bool,
    #[serde(rename = "ignore_whitespace_conflicts")]
    pub ignore_whitespace_conflicts: bool,
    #[serde(rename = "allow_merge_commits")]
    pub allow_merge_commits: bool,
    #[serde(rename = "allow_rebase")]
    pub allow_rebase: bool,
    #[serde(rename = "allow_rebase_explicit")]
    pub allow_rebase_explicit: bool,
    #[serde(rename = "allow_squash_merge")]
    pub allow_squash_merge: bool,
    #[serde(rename = "allow_rebase_update")]
    pub allow_rebase_update: bool,
    #[serde(rename = "default_delete_branch_after_merge")]
    pub default_delete_branch_after_merge: bool,
    #[serde(rename = "default_merge_style")]
    pub default_merge_style: String,
    #[serde(rename = "default_allow_maintainer_edit")]
    pub default_allow_maintainer_edit: bool,
    #[serde(rename = "avatar_url")]
    pub avatar_url: String,
    pub internal: bool,
    #[serde(rename = "mirror_interval")]
    pub mirror_interval: String,
    #[serde(rename = "mirror_updated")]
    pub mirror_updated: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgejoOwner {
    pub id: i64,
    pub login: String,
    #[serde(rename = "login_name")]
    pub login_name: String,
    #[serde(rename = "full_name")]
    pub full_name: String,
    pub email: String,
    #[serde(rename = "avatar_url")]
    pub avatar_url: String,
    pub language: String,
    #[serde(rename = "is_admin")]
    pub is_admin: bool,
    #[serde(rename = "last_login")]
    pub last_login: String,
    pub created: String,
    pub restricted: bool,
    pub active: bool,
    #[serde(rename = "prohibit_login")]
    pub prohibit_login: bool,
    pub location: String,
    pub website: String,
    pub description: String,
    pub visibility: String,
    #[serde(rename = "followers_count")]
    pub followers_count: i64,
    #[serde(rename = "following_count")]
    pub following_count: i64,
    #[serde(rename = "starred_repos_count")]
    pub starred_repos_count: i64,
    pub username: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgejoPermissions {
    pub admin: bool,
    pub push: bool,
    pub pull: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgejoInternalTracker {
    #[serde(rename = "enable_time_tracker")]
    pub enable_time_tracker: bool,
    #[serde(rename = "allow_only_contributors_to_track_time")]
    pub allow_only_contributors_to_track_time: bool,
    #[serde(rename = "enable_issue_dependencies")]
    pub enable_issue_dependencies: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForgejoUpdateUserAvatarRequest {
    pub image: String,
}
