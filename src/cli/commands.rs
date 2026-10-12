use anyhow::{Context, bail};
use log::{debug, info, warn};

use crate::cli::{
    DeleteForgejoOrganisationCommand, MirrorOrganisationCommand, MirrorRepositoryCommand,
    MirrorUserCommand,
};
use crate::forgejo::api::ForgejoApi;
use crate::forgejo::models::{
    ForgejoCreateOrganisationRequest, ForgejoMigrateRepoService, ForgejoMigrateRepositoryRequest,
    ForgejoVisibility,
};
use crate::github::api::GithubApi;
use crate::github::models::GithubRepository;

pub async fn mirror_organisation(cmd: MirrorOrganisationCommand) -> anyhow::Result<()> {
    let mut github = GithubApi::new(cmd.github_token.clone().unwrap())?;
    let mut forgejo = ForgejoApi::new(cmd.forgejo_url.unwrap(), cmd.forgejo_token.unwrap())?;

    let gh_org = github
        .get_organisation(&cmd.github_organisation_name)
        .await?;

    let gh_org_username = gh_org.login;
    let gh_org_display_name = gh_org.name;

    let log_name = gh_org_display_name.as_ref().unwrap_or(&gh_org_username);

    debug!("Fetching repositories of organisation: {}", log_name);

    let repos = github.get_repositories_of_org(&gh_org_username).await?;

    let forgejo_org_username = cmd.org_username.unwrap_or(gh_org_username.clone());
    let forgejo_org_display_name = cmd.org_display_name.or(gh_org_display_name);

    let exists = forgejo.organisation_exists(&forgejo_org_username).await?;

    debug!("Organisation exists: {}", exists);

    if !exists {
        debug!("Organisation does not exist, creating it");

        let full_name = if let Some(display_name) = &forgejo_org_display_name {
            if display_name != &forgejo_org_username {
                Some(display_name.clone())
            } else {
                None
            }
        } else {
            None
        };

        forgejo
            .create_organization(&mut ForgejoCreateOrganisationRequest {
                description: Some(format!(
                    "Mirror of {}\n\n{}",
                    gh_org.html_url,
                    gh_org.description.unwrap_or("".to_string())
                )),
                email: None,
                full_name,
                location: None,
                repo_admin_change_team_access: Some(false),
                username: forgejo_org_username.clone(),
                visibility: Some(cmd.visibility.clone()),
                website: gh_org.blog,
            })
            .await?;

        info!("Created organisation: {}", &forgejo_org_username);

        let avatar_bytes = github.get_organisation_avatar(&gh_org_username).await?;

        debug!("Setting avatar for organisation: {}", &forgejo_org_username);

        forgejo
            .set_organisation_avatar(&forgejo_org_username, avatar_bytes)
            .await?;

        info!("Updated avatar for organisation: {}", &forgejo_org_username);
    }

    let base_repository_request = ForgejoMigrateRepositoryRequest {
        auth_token: cmd.github_token.clone().unwrap(),
        clone_addr: "".to_string(),
        description: None,
        issues: cmd.migrate_issues,
        labels: cmd.migrate_labels,
        lfs: cmd.migrate_lfs,
        lfs_endpoint: None,
        milestones: cmd.migrate_milestones,
        mirror: true,
        mirror_interval: None,
        private: cmd.visibility == ForgejoVisibility::Private,
        pull_requests: cmd.migrate_pull_requests,
        releases: cmd.migrate_releases,
        repo_name: "".to_string(),
        repo_owner: forgejo_org_username.clone(),
        service: ForgejoMigrateRepoService::Github,
        wiki: cmd.migrate_wiki,
    };

    create_migrations_if_not_exist(
        &mut forgejo,
        &forgejo_org_username,
        &base_repository_request,
        repos,
    )
    .await?;

    Ok(())
}

pub async fn mirror_user(cmd: MirrorUserCommand) -> anyhow::Result<()> {
    let mut github = GithubApi::new(cmd.github_token.clone().unwrap())?;
    let mut forgejo = ForgejoApi::new(cmd.forgejo_url.unwrap(), cmd.forgejo_token.unwrap())?;

    let gh_user = github.get_user(&cmd.github_user_name).await?;

    let owner = cmd
        .output_organisation_name
        .unwrap_or(gh_user.login.clone());

    let exists = forgejo.organisation_exists(&owner).await?;

    debug!("Organisation exists: {}", exists);

    if !exists {
        debug!("Organisation does not exist, creating it");

        let full_name = if let Some(name) = &gh_user.name {
            if &gh_user.login != name {
                gh_user.name.clone()
            } else {
                None
            }
        } else {
            None
        };

        forgejo
            .create_organization(&mut ForgejoCreateOrganisationRequest {
                description: Some(format!("Mirror of {}", gh_user.html_url,)),
                email: None,
                full_name,
                location: None,
                repo_admin_change_team_access: Some(false),
                username: owner.clone(),
                visibility: Some(cmd.visibility.clone()),
                website: gh_user.blog,
            })
            .await?;

        info!("Created organisation: {}", &owner);

        let avatar_bytes = github.get_user_avatar(&gh_user.login).await?;

        debug!("Setting avatar for organisation: {}", &owner);

        forgejo.set_organisation_avatar(&owner, avatar_bytes).await?;

        info!("Updated avatar for organisation: {}", &owner);
    }

    let repos = github.get_repositories_of_user(&gh_user.login).await?;

    let base_repository_request = ForgejoMigrateRepositoryRequest {
        auth_token: cmd.github_token.clone().unwrap(),
        clone_addr: "".to_string(),
        description: None,
        issues: cmd.migrate_issues,
        labels: cmd.migrate_labels,
        lfs: cmd.migrate_lfs,
        lfs_endpoint: None,
        milestones: cmd.migrate_milestones,
        mirror: true,
        mirror_interval: None,
        private: cmd.visibility == ForgejoVisibility::Private,
        pull_requests: cmd.migrate_pull_requests,
        releases: cmd.migrate_releases,
        repo_name: "".to_string(),
        repo_owner: owner.clone(),
        service: ForgejoMigrateRepoService::Github,
        wiki: cmd.migrate_wiki,
    };

    create_migrations_if_not_exist(&mut forgejo, &owner, &base_repository_request, repos).await?;

    Ok(())
}
pub async fn mirror_repository(cmd: MirrorRepositoryCommand) -> anyhow::Result<()> {
    let mut forgejo = ForgejoApi::new(cmd.forgejo_url.unwrap(), cmd.forgejo_token.unwrap())?;
    let mut github = GithubApi::new(cmd.github_token.clone().unwrap())?;

    let (owner, repo_name) = parse_github_repository(&cmd.github_repository_url)?;

    debug!("Fetching repository: {}/{}", owner, repo_name);

    let repo = github.get_repository(owner, repo_name).await?;
    let forgejo_repo_name = cmd.output_repository_name.unwrap_or(repo.name.clone());

    let base_repository_request = ForgejoMigrateRepositoryRequest {
        auth_token: cmd.github_token.clone().unwrap(),
        clone_addr: repo.clone_url.clone(),
        description: Some(format!(
            "[MIRROR] {}",
            repo.description.clone().unwrap_or("".to_string())
        )),
        issues: cmd.migrate_issues,
        labels: cmd.migrate_labels,
        lfs: cmd.migrate_lfs,
        lfs_endpoint: None,
        milestones: cmd.migrate_milestones,
        mirror: true,
        mirror_interval: None,
        private: cmd.private || repo.private,
        pull_requests: cmd.migrate_pull_requests,
        releases: cmd.migrate_releases,
        repo_name: forgejo_repo_name,
        repo_owner: cmd.output_owner.clone(),
        service: ForgejoMigrateRepoService::Github,
        wiki: cmd.migrate_wiki,
    };

    create_migration_if_not_exist(
        &mut forgejo,
        &cmd.output_owner,
        &base_repository_request,
        repo,
    )
    .await?;

    Ok(())
}

pub async fn delete_forgejo_organisation(
    cmd: DeleteForgejoOrganisationCommand,
) -> anyhow::Result<()> {
    let mut forgejo = ForgejoApi::new(cmd.forgejo_url.unwrap(), cmd.forgejo_token.unwrap())?;

    let repos = forgejo
        .get_organisation_repositories(&cmd.forgejo_organisation_name)
        .await?;

    for repo in repos {
        forgejo
            .delete_repository(&cmd.forgejo_organisation_name, &repo.name)
            .await?;

        info!("Deleted repository: {}", &repo.name);
    }

    forgejo
        .delete_organisation(&cmd.forgejo_organisation_name)
        .await?;

    info!("Deleted organisation: {}", &cmd.forgejo_organisation_name);

    Ok(())
}

async fn create_migrations_if_not_exist(
    forgejo: &mut ForgejoApi,
    forgejo_owner: &str,
    default_options: &ForgejoMigrateRepositoryRequest,
    repos: Vec<GithubRepository>,
) -> anyhow::Result<()> {
    for repo in repos {
        let mut options = default_options.clone();

        options.repo_name = repo.name.clone();
        options.clone_addr = repo.clone_url.clone();
        options.private = default_options.private || repo.private;
        options.description = Some(format!(
            "[MIRROR] {}",
            repo.description.clone().unwrap_or("".to_string())
        ));

        create_migration_if_not_exist(forgejo, forgejo_owner, &options, repo).await?;
    }

    Ok(())
}

async fn create_migration_if_not_exist(
    forgejo: &mut ForgejoApi,
    forgejo_owner: &str,
    request: &ForgejoMigrateRepositoryRequest,
    _repo: GithubRepository,
) -> anyhow::Result<()> {
    let repo_name = &request.repo_name;
    let exists = forgejo.repository_exists(forgejo_owner, repo_name).await?;

    if exists {
        warn!("Repository already exists: {}, skipping", repo_name);
        return Ok(());
    }

    debug!("Migrating repository: {}", repo_name);

    forgejo.mirror_repository(request).await?;

    info!("Repository mirrored: {}", repo_name);

    Ok(())
}

/// Accepts `https://github.com/owner/repo[.git]`, `http://…`, `git@github.com:owner/repo[.git]`
/// and `ssh://git@github.com/owner/repo[.git]`; returns `(owner, repo)`.
pub fn parse_github_repository(url: &str) -> anyhow::Result<(&str, &str)> {
    const PREFIXES: [&str; 4] = [
        "https://github.com/",
        "http://github.com/",
        "git@github.com:",
        "ssh://git@github.com/",
    ];

    let path = PREFIXES
        .iter()
        .find_map(|prefix| url.strip_prefix(prefix))
        .with_context(|| format!("unsupported github repository url: {url}"))?;
    let path = path.trim_end_matches('/').trim_end_matches(".git");

    match path.split_once('/') {
        Some((owner, repo)) if !owner.is_empty() && !repo.is_empty() && !repo.contains('/') => {
            Ok((owner, repo))
        }
        _ => bail!("github repository url must be owner/repository, got: {url}"),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_github_repository;

    #[test]
    fn parses_supported_url_forms() {
        for url in [
            "https://github.com/octo/repo",
            "https://github.com/octo/repo.git",
            "https://github.com/octo/repo/",
            "http://github.com/octo/repo",
            "git@github.com:octo/repo.git",
            "ssh://git@github.com/octo/repo",
        ] {
            assert_eq!(parse_github_repository(url).unwrap(), ("octo", "repo"), "{url}");
        }
    }

    #[test]
    fn rejects_malformed_urls() {
        for url in [
            "https://github.com/octo",
            "https://github.com/octo/repo/tree/main",
            "https://gitlab.com/octo/repo",
            "octo/repo",
            "https://github.com//repo",
        ] {
            assert!(parse_github_repository(url).is_err(), "{url}");
        }
    }
}
