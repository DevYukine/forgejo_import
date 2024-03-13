use clap::Parser;

use cli::commands;
use commands::{delete_forgejo_organisation, mirror_organisation, mirror_repository, mirror_user};

use crate::cli::{Cli, Commands};
use crate::config::apply_config;

mod cli;
mod config;
mod forgejo;
mod github;
mod util;

pub mod built_info {
    // The file has been placed there by the build script.
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

    let mut cli = Cli::parse();

    apply_config(&mut cli.command).await?;

    return match cli.command {
        Commands::MirrorOrg(cmd) => mirror_organisation(cmd).await,
        Commands::MirrorUser(cmd) => mirror_user(cmd).await,
        Commands::MirrorRepo(cmd) => mirror_repository(cmd).await,
        Commands::DeleteOrg(cmd) => delete_forgejo_organisation(cmd).await,
    };
}
