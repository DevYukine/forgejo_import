use clap::Parser;

use forgejo_import::cli::{Cli, Command};
use forgejo_import::commands;
use forgejo_import::config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

    let cli = Cli::parse();
    let config = Config::load()?;

    match cli.command {
        Command::MirrorOrg(cmd) => commands::mirror_organisation(cmd, &config).await,
        Command::MirrorUser(cmd) => commands::mirror_user(cmd, &config).await,
        Command::MirrorRepo(cmd) => commands::mirror_repository(cmd, &config).await,
        Command::DeleteOrg(cmd) => commands::delete_organisation(cmd, &config).await,
    }
}
