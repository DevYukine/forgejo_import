use clap::{Parser, Subcommand};

use crate::forgejo::models::ForgejoVisibility;

pub mod commands;
pub mod errors;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Mirror a github organisation, including all repositories you have access to, to forgejo
    MirrorOrg(MirrorOrganisationCommand),

    /// Mirror a github user, including all repositories you have access to, to forgejo
    MirrorUser(MirrorUserCommand),

    /// Mirror a github repository to forgejo
    MirrorRepo(MirrorRepositoryCommand),

    /// Delete a forgejo organisation including all repositories
    DeleteOrg(DeleteForgejoOrganisationCommand),
}

#[derive(Parser, Debug, Clone)]
pub struct MirrorOrganisationCommand {
    /// the url of the forgejo instance to use
    #[arg(long)]
    pub forgejo_url: Option<String>,

    /// the api token to use for forgejo
    #[arg(long)]
    pub forgejo_token: Option<String>,

    /// the github token to use for obtaining information from the github api
    #[arg(long)]
    pub github_token: Option<String>,

    /// the visibility of the created forgejo organisation
    #[arg(short, long, default_value = "public")]
    pub visibility: ForgejoVisibility,

    /// the display name of the forgejo organisation to create, by default it will be the same as the github organisation
    #[arg(short, long)]
    pub org_display_name: Option<String>,

    /// the username of the forgejo organisation to create, by default it will be the same as the github organisation
    #[arg(long)]
    pub org_username: Option<String>,

    /// if set then forgejo will also migrate the L(arge) F(ile) S(torage) of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_lfs: bool,

    /// if set then forgejo will also migrate the wiki of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_wiki: bool,

    /// if set then forgejo will also migrate the labels of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_labels: bool,

    /// if set then forgejo will also migrate the issues of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_issues: bool,

    /// if set then forgejo will also migrate the pull requests of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_pull_requests: bool,

    /// if set then forgejo will also migrate the releases of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_releases: bool,

    /// if set then forgejo will also migrate the milestones of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_milestones: bool,

    /// the name of the github organisation to mirror
    pub github_organisation_name: String,
}

#[derive(Parser, Debug, Clone)]
pub struct MirrorUserCommand {
    /// the url of the forgejo instance to use
    #[arg(long)]
    pub forgejo_url: Option<String>,

    /// the api token to use for forgejo
    #[arg(long)]
    pub forgejo_token: Option<String>,

    /// the github token to use for obtaining information from the github api
    #[arg(long)]
    pub github_token: Option<String>,

    /// the visibility of the created forgejo organisation
    #[arg(short, long, default_value = "public")]
    pub visibility: ForgejoVisibility,

    /// the name of the forgejo organisation to create the repositories in, defaults to the name of the github user
    #[arg(long)]
    pub output_organisation_name: Option<String>,

    /// if set then forgejo will also migrate the L(arge) F(ile) S(torage) of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_lfs: bool,

    /// if set then forgejo will also migrate the wiki of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_wiki: bool,

    /// if set then forgejo will also migrate the labels of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_labels: bool,

    /// if set then forgejo will also migrate the issues of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_issues: bool,

    /// if set then forgejo will also migrate the pull requests of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_pull_requests: bool,

    /// if set then forgejo will also migrate the releases of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_releases: bool,

    /// if set then forgejo will also migrate the milestones of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_milestones: bool,

    /// the name of the github user to mirror
    pub github_user_name: String,
}

#[derive(Parser, Debug, Clone)]
pub struct MirrorRepositoryCommand {
    /// the url of the forgejo instance to use
    #[arg(long)]
    pub forgejo_url: Option<String>,

    /// the api token to use for forgejo
    #[arg(long)]
    pub forgejo_token: Option<String>,

    /// the github token to use for obtaining information from the github api
    #[arg(long)]
    pub github_token: Option<String>,

    /// the name of the forgejo owner (either an user or organisation) to create the repository in
    #[arg(long)]
    pub output_owner: String,

    /// the name of the forgejo repository to create, by default it will be the same as the github repository
    #[arg(long)]
    pub output_repository_name: Option<String>,

    /// if set then forgejo will also migrate the L(arge) F(ile) S(torage) of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_lfs: bool,

    /// if set then forgejo will also migrate the wiki of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_wiki: bool,

    /// if set then forgejo will also migrate the labels of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_labels: bool,

    /// if set then forgejo will also migrate the issues of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_issues: bool,

    /// if set then forgejo will also migrate the pull requests of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_pull_requests: bool,

    /// if set then forgejo will also migrate the releases of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_releases: bool,

    /// if set then forgejo will also migrate the milestones of the repositories
    #[arg(long, default_value = "false")]
    pub migrate_milestones: bool,

    /// if set then the repository will be private, otherwise it will be public or inherit the visibility of the owner
    #[arg(short, long, default_value = "false")]
    pub private: bool,

    /// the url of the repository to mirror
    pub github_repository_url: String,
}

#[derive(Parser, Debug, Clone)]
pub struct DeleteForgejoOrganisationCommand {
    /// the url of the forgejo instance to use
    #[arg(long)]
    pub forgejo_url: Option<String>,

    /// the api token to use for forgejo
    #[arg(long)]
    pub forgejo_token: Option<String>,

    /// the name of the forgejo organisation to delete
    pub forgejo_organisation_name: String,
}
