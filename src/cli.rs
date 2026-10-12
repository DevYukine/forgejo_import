use clap::{Args, Parser, Subcommand};

use crate::forgejo::Visibility;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Mirror a github organisation, including all repositories you have access to, to forgejo
    MirrorOrg(MirrorOrg),

    /// Mirror a github user to forgejo, including private repositories when the token belongs to that user
    MirrorUser(MirrorUser),

    /// Mirror a github repository to forgejo
    MirrorRepo(MirrorRepo),

    /// Delete a forgejo organisation including all repositories
    DeleteOrg(DeleteOrg),
}

/// Connection to the Forgejo instance; falls back to the config file when omitted.
#[derive(Args)]
pub struct ForgejoArgs {
    /// the url of the forgejo instance to use
    #[arg(long)]
    pub forgejo_url: Option<String>,

    /// the api token to use for forgejo
    #[arg(long)]
    pub forgejo_token: Option<String>,
}

/// GitHub credentials; fall back to the config file when omitted.
#[derive(Args)]
pub struct GithubArgs {
    /// the github token to use for obtaining information from the github api
    #[arg(long)]
    pub github_token: Option<String>,
}

/// What to migrate besides the git data. Forgejo mirrors cannot carry issues, labels, pull
/// requests, releases or milestones, so only wiki and LFS are available.
#[derive(Args, Clone, Copy)]
pub struct MirrorArgs {
    /// if set then forgejo will also migrate the L(arge) F(ile) S(torage) of the repositories
    #[arg(long)]
    pub migrate_lfs: bool,

    /// if set then forgejo will also migrate the wiki of the repositories
    #[arg(long)]
    pub migrate_wiki: bool,
}

#[derive(Args)]
pub struct MirrorOrg {
    #[command(flatten)]
    pub forgejo: ForgejoArgs,

    #[command(flatten)]
    pub github: GithubArgs,

    #[command(flatten)]
    pub mirror: MirrorArgs,

    /// the visibility of the forgejo organisation if it gets created; private also makes every mirrored repository private
    #[arg(short, long, default_value = "public")]
    pub visibility: Visibility,

    /// the display name of the forgejo organisation to create, by default it will be the same as the github organisation
    #[arg(short, long)]
    pub org_display_name: Option<String>,

    /// the username of the forgejo organisation to create, by default it will be the same as the github organisation
    #[arg(long)]
    pub org_username: Option<String>,

    /// the name of the github organisation to mirror
    pub github_organisation_name: String,
}

#[derive(Args)]
pub struct MirrorUser {
    #[command(flatten)]
    pub forgejo: ForgejoArgs,

    #[command(flatten)]
    pub github: GithubArgs,

    #[command(flatten)]
    pub mirror: MirrorArgs,

    /// the visibility of the forgejo organisation if it gets created; private also makes every mirrored repository private
    #[arg(short, long, default_value = "public")]
    pub visibility: Visibility,

    /// the name of the forgejo organisation to create the repositories in, defaults to the name of the github user
    #[arg(long)]
    pub output_organisation_name: Option<String>,

    /// the name of the github user to mirror
    pub github_user_name: String,
}

#[derive(Args)]
pub struct MirrorRepo {
    #[command(flatten)]
    pub forgejo: ForgejoArgs,

    #[command(flatten)]
    pub github: GithubArgs,

    #[command(flatten)]
    pub mirror: MirrorArgs,

    /// the name of the forgejo owner (either an user or organisation) to create the repository in
    #[arg(long)]
    pub output_owner: String,

    /// the name of the forgejo repository to create, by default it will be the same as the github repository
    #[arg(long)]
    pub output_repository_name: Option<String>,

    /// if set then the repository will be private, otherwise it inherits the privacy of the github repository
    #[arg(short, long)]
    pub private: bool,

    /// the url of the repository to mirror
    pub github_repository_url: String,
}

#[derive(Args)]
pub struct DeleteOrg {
    #[command(flatten)]
    pub forgejo: ForgejoArgs,

    /// skip the confirmation prompt
    #[arg(short, long)]
    pub yes: bool,

    /// the name of the forgejo organisation to delete
    pub forgejo_organisation_name: String,
}
