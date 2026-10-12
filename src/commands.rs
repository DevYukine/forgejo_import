use anyhow::{Context, Result, bail};
use log::{error, info, warn};

use crate::cli::{
    DeleteOrg, ForgejoArgs, GithubArgs, MirrorArgs, MirrorOrg, MirrorRepo, MirrorUser,
};
use crate::config::Config;
use crate::forgejo::{self, ForgejoApi, Visibility};
use crate::github::{self, GithubApi};

pub async fn mirror_organisation(cmd: MirrorOrg, config: &Config) -> Result<()> {
    let github = github_api(cmd.github, config)?;
    let forgejo = forgejo_api(cmd.forgejo, config).await?;
    let mirror = merge_mirror_args(cmd.mirror, config);

    let org = github.organisation(&cmd.github_organisation_name).await?;
    let owner = cmd.org_username.unwrap_or_else(|| org.login.clone());
    let description = match &org.description {
        Some(text) => format!("Mirror of {}\n\n{text}", org.html_url),
        None => format!("Mirror of {}", org.html_url),
    };

    ensure_organisation(
        &forgejo,
        &github,
        Organisation {
            username: &owner,
            display_name: cmd.org_display_name.or(org.name),
            description,
            website: org.blog,
            visibility: cmd.visibility,
            avatar_url: &org.avatar_url,
        },
    )
    .await?;

    let repos = github.organisation_repositories(&org.login).await?;
    let force_private = cmd.visibility == Visibility::Private;

    mirror_repositories(&forgejo, &github, repos, &owner, force_private, mirror).await
}

pub async fn mirror_user(cmd: MirrorUser, config: &Config) -> Result<()> {
    let github = github_api(cmd.github, config)?;
    let forgejo = forgejo_api(cmd.forgejo, config).await?;
    let mirror = merge_mirror_args(cmd.mirror, config);

    let user = github.user(&cmd.github_user_name).await?;
    let owner = cmd
        .output_organisation_name
        .unwrap_or_else(|| user.login.clone());

    ensure_organisation(
        &forgejo,
        &github,
        Organisation {
            username: &owner,
            display_name: user.name,
            description: format!("Mirror of {}", user.html_url),
            website: user.blog,
            visibility: cmd.visibility,
            avatar_url: &user.avatar_url,
        },
    )
    .await?;

    let repos = github.user_repositories(&user.login).await?;
    let force_private = cmd.visibility == Visibility::Private;

    mirror_repositories(&forgejo, &github, repos, &owner, force_private, mirror).await
}

pub async fn mirror_repository(cmd: MirrorRepo, config: &Config) -> Result<()> {
    let github = github_api(cmd.github, config)?;
    let forgejo = forgejo_api(cmd.forgejo, config).await?;
    let mirror = merge_mirror_args(cmd.mirror, config);

    let (owner, name) = parse_github_repository(&cmd.github_repository_url)?;
    let repo = github.repository(owner, name).await?;
    let target_name = cmd
        .output_repository_name
        .unwrap_or_else(|| repo.name.clone());
    let request = migrate_request(
        &repo,
        &cmd.output_owner,
        target_name,
        cmd.private,
        mirror,
        github.token(),
    );

    mirror_one(&forgejo, &request).await
}

pub async fn delete_organisation(cmd: DeleteOrg, config: &Config) -> Result<()> {
    let forgejo = forgejo_api(cmd.forgejo, config).await?;
    let org = &cmd.forgejo_organisation_name;

    let repos = forgejo.organisation_repositories(org).await?;

    if !cmd.yes
        && !confirm(&format!(
            "Delete organisation {org} and all of its {} repositories? This cannot be undone. [y/N] ",
            repos.len()
        ))?
    {
        bail!("aborted");
    }

    for repo in repos {
        forgejo.delete_repository(org, &repo.name).await?;
        info!("Deleted repository {org}/{}", repo.name);
    }

    forgejo.delete_organisation(org).await?;
    info!("Deleted organisation {org}");

    Ok(())
}

fn confirm(prompt: &str) -> Result<bool> {
    use std::io::{BufRead, Write};

    print!("{prompt}");
    std::io::stdout().flush()?;

    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer)?;

    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES"))
}

fn github_api(args: GithubArgs, config: &Config) -> Result<GithubApi> {
    let token = args
        .github_token
        .or_else(|| config.github_token.clone())
        .context("missing --github-token (cli) or githubToken (config file)")?;

    GithubApi::new(token)
}

async fn forgejo_api(args: ForgejoArgs, config: &Config) -> Result<ForgejoApi> {
    let url = args
        .forgejo_url
        .or_else(|| config.forgejo_url.clone())
        .context("missing --forgejo-url (cli) or forgejoUrl (config file)")?;
    let token = args
        .forgejo_token
        .or_else(|| config.forgejo_token.clone())
        .context("missing --forgejo-token (cli) or forgejoToken (config file)")?;

    ForgejoApi::connect(&url, token).await
}

/// CLI flags win; the config file can only switch a migration on, never off.
fn merge_mirror_args(args: MirrorArgs, config: &Config) -> MirrorArgs {
    MirrorArgs {
        migrate_lfs: args.migrate_lfs || config.migrate_lfs.unwrap_or(false),
        migrate_wiki: args.migrate_wiki || config.migrate_wiki.unwrap_or(false),
    }
}

struct Organisation<'a> {
    username: &'a str,
    display_name: Option<String>,
    description: String,
    website: Option<String>,
    visibility: Visibility,
    avatar_url: &'a str,
}

/// Creates the Forgejo organisation with the GitHub account's metadata unless it already exists.
async fn ensure_organisation(
    forgejo: &ForgejoApi,
    github: &GithubApi,
    org: Organisation<'_>,
) -> Result<()> {
    let username = org.username;

    if forgejo.organisation_exists(username).await? {
        info!("Organisation {username} already exists, leaving it unchanged");
        return Ok(());
    }

    forgejo
        .create_organisation(&forgejo::CreateOrganisation {
            username: username.to_string(),
            full_name: org.display_name.filter(|name| name != username),
            description: org.description,
            website: org.website.and_then(normalise_website),
            visibility: org.visibility,
        })
        .await?;
    info!("Created organisation {username}");

    // The organisation is usable without an avatar, and a rerun would skip this block anyway.
    if !forgejo.supports_organisation_avatar() {
        warn!(
            "Forgejo {} cannot set organisation avatars through the API (needs Gitea API 1.21), skipping",
            forgejo.version()
        );
    } else {
        match set_avatar(forgejo, github, username, org.avatar_url).await {
            Ok(()) => info!("Set avatar of organisation {username}"),
            Err(err) => warn!("Could not set avatar of organisation {username}: {err:#}"),
        }
    }

    Ok(())
}

async fn set_avatar(
    forgejo: &ForgejoApi,
    github: &GithubApi,
    username: &str,
    avatar_url: &str,
) -> Result<()> {
    let image = github.avatar(avatar_url).await?;

    forgejo.set_organisation_avatar(username, image).await
}

fn normalise_website(website: String) -> Option<String> {
    if website.is_empty() {
        None
    } else if website.starts_with("http://") || website.starts_with("https://") {
        Some(website)
    } else {
        Some(format!("https://{website}"))
    }
}

/// Mirrors every repository, continuing past failures so one bad repository does not stop the batch.
async fn mirror_repositories(
    forgejo: &ForgejoApi,
    github: &GithubApi,
    repos: Vec<github::Repository>,
    owner: &str,
    force_private: bool,
    mirror: MirrorArgs,
) -> Result<()> {
    let total = repos.len();
    let mut failed = 0;

    for repo in repos {
        let request = migrate_request(
            &repo,
            owner,
            repo.name.clone(),
            force_private,
            mirror,
            github.token(),
        );

        if let Err(err) = mirror_one(forgejo, &request).await {
            error!("Failed to mirror {}: {err:#}", repo.name);
            failed += 1;
        }
    }

    if failed > 0 {
        bail!("{failed} of {total} repositories failed to mirror");
    }

    Ok(())
}

fn migrate_request(
    repo: &github::Repository,
    owner: &str,
    name: String,
    force_private: bool,
    mirror: MirrorArgs,
    github_token: &str,
) -> forgejo::MigrateRepository {
    forgejo::MigrateRepository {
        auth_token: github_token.to_string(),
        clone_addr: repo.clone_url.clone(),
        description: match &repo.description {
            Some(text) => format!("[MIRROR] {text}"),
            None => "[MIRROR]".to_string(),
        },
        lfs: mirror.migrate_lfs,
        mirror: true,
        private: force_private || repo.private,
        repo_name: name,
        repo_owner: owner.to_string(),
        service: "github",
        wiki: mirror.migrate_wiki,
    }
}

async fn mirror_one(forgejo: &ForgejoApi, request: &forgejo::MigrateRepository) -> Result<()> {
    let target = format!("{}/{}", request.repo_owner, request.repo_name);

    if forgejo
        .repository_exists(&request.repo_owner, &request.repo_name)
        .await?
    {
        warn!("Repository {target} already exists, skipping");
        return Ok(());
    }

    forgejo.migrate_repository(request).await?;
    info!("Mirrored {} to {target}", request.clone_addr);

    Ok(())
}

/// Accepts `https://github.com/owner/repo[.git]`, `http://…`, `git@github.com:owner/repo[.git]`
/// and `ssh://git@github.com/owner/repo[.git]`; returns `(owner, repo)`.
pub fn parse_github_repository(url: &str) -> Result<(&str, &str)> {
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
    use super::{normalise_website, parse_github_repository};

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
            assert_eq!(
                parse_github_repository(url).unwrap(),
                ("octo", "repo"),
                "{url}"
            );
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

    #[test]
    fn website_gets_a_scheme_only_when_missing() {
        assert_eq!(normalise_website(String::new()), None);
        assert_eq!(
            normalise_website("example.org".into()).as_deref(),
            Some("https://example.org")
        );
        assert_eq!(
            normalise_website("http://example.org".into()).as_deref(),
            Some("http://example.org")
        );
    }
}
