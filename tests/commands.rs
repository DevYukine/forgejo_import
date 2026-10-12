mod common;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use reqwest::Url;
use serde_json::{Value, json};
use wiremock::matchers::{body_json, body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use common::{
    AVATAR_BYTES, body_json as request_body, config, fixture, mount_avatar, mount_pages,
    mount_status, mount_version, no_forgejo_args, no_github_args, no_mirror_args, ok, requests,
};
use forgejo_import::cli::{
    DeleteOrg, ForgejoArgs, GithubArgs, MirrorArgs, MirrorOrg, MirrorRepo, MirrorUser,
};
use forgejo_import::commands;
use forgejo_import::config::Config;
use forgejo_import::forgejo::Visibility;

struct Servers {
    github: MockServer,
    forgejo: MockServer,
}

impl Servers {
    async fn start(forgejo_version: &str) -> Self {
        let servers = Self {
            github: MockServer::start().await,
            forgejo: MockServer::start().await,
        };
        mount_version(&servers.forgejo, forgejo_version).await;
        servers
    }

    fn config(&self) -> Config {
        config(&self.github, &self.forgejo)
    }

    async fn github_org_with_repos(&self) {
        Mock::given(method("GET"))
            .and(path("/orgs/octo-org"))
            .respond_with(ok(fixture("github/org.json", &self.github)))
            .mount(&self.github)
            .await;
        mount_pages(
            &self.github,
            "/orgs/octo-org/repos",
            "page",
            &[fixture("github/repos.json", &self.github)],
        )
        .await;
    }

    async fn forgejo_repo_missing(&self, owner: &str, name: &str) {
        mount_status(
            &self.forgejo,
            "GET",
            &format!("/api/v1/repos/{owner}/{name}"),
            404,
        )
        .await;
    }

    async fn expect_migrate(&self, partial_body: Value, status: u16) {
        Mock::given(method("POST"))
            .and(path("/api/v1/repos/migrate"))
            .and(body_partial_json(partial_body))
            .respond_with(ResponseTemplate::new(status))
            .expect(1)
            .mount(&self.forgejo)
            .await;
    }

    async fn migrate_bodies(&self) -> Vec<Value> {
        requests(&self.forgejo)
            .await
            .iter()
            .filter(|req| req.url.path() == "/api/v1/repos/migrate")
            .map(request_body)
            .collect()
    }
}

fn mirror_org(name: &str, visibility: Visibility) -> MirrorOrg {
    MirrorOrg {
        forgejo: no_forgejo_args(),
        github: no_github_args(),
        mirror: no_mirror_args(),
        visibility,
        org_display_name: None,
        org_username: None,
        github_organisation_name: name.to_string(),
    }
}

#[tokio::test]
async fn mirror_org_creates_the_organisation_with_avatar_and_mirrors_every_repository() {
    let servers = Servers::start("7.0.0+gitea-1.22.0").await;
    servers.github_org_with_repos().await;
    mount_avatar(&servers.github, "/avatars/u/583231").await;
    mount_status(&servers.forgejo, "GET", "/api/v1/orgs/octo-org", 404).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/orgs"))
        .and(header("authorization", "Bearer fj-token"))
        .and(body_json(json!({
            "username": "octo-org",
            "full_name": "Octo Org",
            "description": "Mirror of https://github.com/octo-org",
            "website": "https://octo.example",
            "visibility": "public"
        })))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&servers.forgejo)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/orgs/octo-org/avatar"))
        .and(body_json(json!({ "image": STANDARD.encode(AVATAR_BYTES) })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&servers.forgejo)
        .await;
    for name in ["alpha", "beta", "empty"] {
        servers.forgejo_repo_missing("octo-org", name).await;
    }
    servers
        .expect_migrate(json!({ "repo_name": "alpha", "private": false, "description": "[MIRROR] First repository" }), 201)
        .await;
    servers
        .expect_migrate(
            json!({ "repo_name": "beta", "private": true, "description": "[MIRROR]" }),
            201,
        )
        .await;
    servers
        .expect_migrate(
            json!({ "repo_name": "empty", "private": false, "description": "[MIRROR]" }),
            201,
        )
        .await;

    commands::mirror_organisation(
        mirror_org("octo-org", Visibility::Public),
        &servers.config(),
    )
    .await
    .unwrap();

    let bodies = servers.migrate_bodies().await;
    let order: Vec<&str> = bodies
        .iter()
        .map(|b| b["repo_name"].as_str().unwrap())
        .collect();
    assert_eq!(
        order,
        ["empty", "beta", "alpha"],
        "smallest repositories first"
    );
    for body in &bodies {
        assert_eq!(body["repo_owner"], "octo-org");
        assert_eq!(body["auth_token"], "gh-token");
        assert_eq!(body["mirror"], true);
        assert_eq!(body["service"], "github");
        assert_eq!(body["wiki"], false);
        assert_eq!(body["lfs"], false);
        assert!(body["clone_addr"].as_str().unwrap().ends_with(".git"));
    }
}

#[tokio::test]
async fn mirror_org_skips_existing_repositories_and_continues_after_a_failure() {
    let servers = Servers::start("7.0.0+gitea-1.22.0").await;
    servers.github_org_with_repos().await;
    mount_status(&servers.forgejo, "GET", "/api/v1/orgs/octo-org", 200).await;
    servers.forgejo_repo_missing("octo-org", "empty").await;
    servers.forgejo_repo_missing("octo-org", "beta").await;
    mount_status(&servers.forgejo, "GET", "/api/v1/repos/octo-org/alpha", 200).await;
    servers
        .expect_migrate(json!({ "repo_name": "empty" }), 500)
        .await;
    servers
        .expect_migrate(json!({ "repo_name": "beta" }), 201)
        .await;

    let err = commands::mirror_organisation(
        mirror_org("octo-org", Visibility::Public),
        &servers.config(),
    )
    .await
    .unwrap_err();

    assert_eq!(err.to_string(), "1 of 3 repositories failed to mirror");
    let migrated: Vec<Value> = servers.migrate_bodies().await;
    assert_eq!(
        migrated.len(),
        2,
        "alpha exists already and must not be migrated"
    );
    assert!(
        !requests(&servers.forgejo)
            .await
            .iter()
            .any(|r| r.method == "POST" && r.url.path() == "/api/v1/orgs"),
        "existing organisation must not be recreated"
    );
}

#[tokio::test]
async fn mirror_org_private_visibility_makes_every_repository_private() {
    let servers = Servers::start("7.0.0+gitea-1.22.0").await;
    servers.github_org_with_repos().await;
    mount_status(&servers.forgejo, "GET", "/api/v1/orgs/octo-org", 200).await;
    for name in ["alpha", "beta", "empty"] {
        servers.forgejo_repo_missing("octo-org", name).await;
        servers
            .expect_migrate(json!({ "repo_name": name, "private": true }), 201)
            .await;
    }

    commands::mirror_organisation(
        mirror_org("octo-org", Visibility::Private),
        &servers.config(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn mirror_org_skips_the_avatar_on_forgejo_without_the_avatar_api() {
    let servers = Servers::start("1.20.6+0").await;
    servers.github_org_with_repos().await;
    mount_status(&servers.forgejo, "GET", "/api/v1/orgs/octo-org", 404).await;
    mount_status(&servers.forgejo, "POST", "/api/v1/orgs", 201).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/orgs/octo-org/avatar"))
        .respond_with(ResponseTemplate::new(404))
        .expect(0)
        .mount(&servers.forgejo)
        .await;
    for name in ["alpha", "beta", "empty"] {
        servers.forgejo_repo_missing("octo-org", name).await;
        servers
            .expect_migrate(json!({ "repo_name": name }), 201)
            .await;
    }

    commands::mirror_organisation(
        mirror_org("octo-org", Visibility::Public),
        &servers.config(),
    )
    .await
    .unwrap();

    assert!(
        !requests(&servers.github)
            .await
            .iter()
            .any(|r| r.url.path().starts_with("/avatars")),
        "avatar must not even be downloaded"
    );
}

#[tokio::test]
async fn mirror_org_avatar_failure_does_not_stop_the_run() {
    let servers = Servers::start("7.0.0+gitea-1.22.0").await;
    servers.github_org_with_repos().await;
    mount_status(&servers.github, "GET", "/avatars/u/583231", 500).await;
    mount_status(&servers.forgejo, "GET", "/api/v1/orgs/octo-org", 404).await;
    mount_status(&servers.forgejo, "POST", "/api/v1/orgs", 201).await;
    for name in ["alpha", "beta", "empty"] {
        servers.forgejo_repo_missing("octo-org", name).await;
        servers
            .expect_migrate(json!({ "repo_name": name }), 201)
            .await;
    }

    commands::mirror_organisation(
        mirror_org("octo-org", Visibility::Public),
        &servers.config(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn mirror_org_honours_custom_username_and_display_name() {
    let servers = Servers::start("7.0.0+gitea-1.22.0").await;
    servers.github_org_with_repos().await;
    mount_avatar(&servers.github, "/avatars/u/583231").await;
    mount_status(&servers.forgejo, "GET", "/api/v1/orgs/renamed", 404).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/orgs"))
        .and(body_partial_json(
            json!({ "username": "renamed", "full_name": "Shown Name" }),
        ))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&servers.forgejo)
        .await;
    mount_status(&servers.forgejo, "POST", "/api/v1/orgs/renamed/avatar", 204).await;
    for name in ["alpha", "beta", "empty"] {
        servers.forgejo_repo_missing("renamed", name).await;
        servers
            .expect_migrate(json!({ "repo_name": name, "repo_owner": "renamed" }), 201)
            .await;
    }
    let cmd = MirrorOrg {
        org_display_name: Some("Shown Name".into()),
        org_username: Some("renamed".into()),
        ..mirror_org("octo-org", Visibility::Public)
    };

    commands::mirror_organisation(cmd, &servers.config())
        .await
        .unwrap();
}

#[tokio::test]
async fn mirror_user_targets_the_output_organisation_everywhere() {
    let servers = Servers::start("7.0.0+gitea-1.22.0").await;
    Mock::given(method("GET"))
        .and(path("/users/octocat"))
        .respond_with(ok(fixture("github/user.json", &servers.github)))
        .mount(&servers.github)
        .await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(ok(
            json!({ "login": "octocat", "html_url": "", "avatar_url": "" }),
        ))
        .mount(&servers.github)
        .await;
    mount_pages(
        &servers.github,
        "/user/repos",
        "page",
        &[json!([fixture("github/repos.json", &servers.github)[1]])],
    )
    .await;
    mount_avatar(&servers.github, "/avatars/u/1").await;
    mount_status(&servers.forgejo, "GET", "/api/v1/orgs/octo-mirror", 404).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/orgs"))
        .and(body_json(json!({
            "username": "octo-mirror",
            "full_name": "The Octocat",
            "description": "Mirror of https://github.com/octocat",
            "website": "https://github.blog",
            "visibility": "limited"
        })))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&servers.forgejo)
        .await;
    mount_status(
        &servers.forgejo,
        "POST",
        "/api/v1/orgs/octo-mirror/avatar",
        204,
    )
    .await;
    servers.forgejo_repo_missing("octo-mirror", "beta").await;
    servers
        .expect_migrate(
            json!({ "repo_name": "beta", "repo_owner": "octo-mirror", "private": true }),
            201,
        )
        .await;
    let cmd = MirrorUser {
        forgejo: no_forgejo_args(),
        github: no_github_args(),
        mirror: no_mirror_args(),
        visibility: Visibility::Limited,
        output_organisation_name: Some("octo-mirror".into()),
        github_user_name: "octocat".into(),
    };

    commands::mirror_user(cmd, &servers.config()).await.unwrap();
}

/// `path` is `owner/repo…`; the host is the mock GitHub, which the host check requires.
fn mirror_repo(github: &MockServer, path: &str) -> MirrorRepo {
    let host = Url::parse(&github.uri())
        .unwrap()
        .host_str()
        .unwrap()
        .to_string();

    MirrorRepo {
        forgejo: no_forgejo_args(),
        github: no_github_args(),
        mirror: no_mirror_args(),
        output_owner: "me".into(),
        output_repository_name: None,
        private: false,
        github_repository_url: format!("git@{host}:{path}"),
    }
}

#[tokio::test]
async fn mirror_repo_honours_output_name_and_private_flag() {
    let servers = Servers::start("7.0.0+gitea-1.22.0").await;
    Mock::given(method("GET"))
        .and(path("/repos/octo-org/alpha"))
        .respond_with(ok(fixture("github/repos.json", &servers.github)[0].clone()))
        .expect(1)
        .mount(&servers.github)
        .await;
    servers.forgejo_repo_missing("me", "renamed").await;
    servers
        .expect_migrate(
            json!({
                "repo_name": "renamed",
                "repo_owner": "me",
                "private": true,
                "clone_addr": "https://github.com/octo-org/alpha.git"
            }),
            201,
        )
        .await;
    let cmd = MirrorRepo {
        output_repository_name: Some("renamed".into()),
        private: true,
        ..mirror_repo(&servers.github, "octo-org/alpha.git")
    };

    commands::mirror_repository(cmd, &servers.config())
        .await
        .unwrap();
}

#[tokio::test]
async fn mirror_repo_skips_when_the_target_already_exists() {
    let servers = Servers::start("7.0.0+gitea-1.22.0").await;
    Mock::given(method("GET"))
        .and(path("/repos/octo-org/alpha"))
        .respond_with(ok(fixture("github/repos.json", &servers.github)[0].clone()))
        .mount(&servers.github)
        .await;
    mount_status(&servers.forgejo, "GET", "/api/v1/repos/me/alpha", 200).await;

    commands::mirror_repository(
        mirror_repo(&servers.github, "octo-org/alpha"),
        &servers.config(),
    )
    .await
    .unwrap();

    assert!(servers.migrate_bodies().await.is_empty());
}

#[tokio::test]
async fn mirror_repo_rejects_malformed_urls_before_any_request() {
    let (github, forgejo) = (MockServer::start().await, MockServer::start().await);

    let cmd = MirrorRepo {
        github_repository_url: "https://gitlab.com/octo/alpha".into(),
        ..mirror_repo(&github, "octo/alpha")
    };

    let err = commands::mirror_repository(cmd, &config(&github, &forgejo))
        .await
        .unwrap_err();

    assert!(
        err.to_string()
            .contains("does not belong to the configured github api"),
        "{err}"
    );
    assert!(requests(&github).await.is_empty());
    assert!(requests(&forgejo).await.is_empty());
}

#[tokio::test]
async fn cli_credentials_win_over_config_and_migrate_flags_merge() {
    let servers = Servers::start("7.0.0+gitea-1.22.0").await;
    let mut config = servers.config();
    config.forgejo_token = Some("config-token".into());
    config.github_token = Some("config-token".into());
    config.migrate_wiki = Some(true);
    config.migrate_lfs = Some(false);
    Mock::given(method("GET"))
        .and(path("/repos/octo-org/alpha"))
        .and(header("authorization", "Bearer cli-gh"))
        .respond_with(ok(fixture("github/repos.json", &servers.github)[0].clone()))
        .expect(1)
        .mount(&servers.github)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/repos/me/alpha"))
        .and(header("authorization", "Bearer cli-fj"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&servers.forgejo)
        .await;
    servers
        .expect_migrate(
            json!({ "auth_token": "cli-gh", "wiki": true, "lfs": true }),
            201,
        )
        .await;
    let cmd = MirrorRepo {
        forgejo: ForgejoArgs {
            forgejo_url: None,
            forgejo_token: Some("cli-fj".into()),
        },
        github: GithubArgs {
            github_token: Some("cli-gh".into()),
            github_api_url: None,
        },
        mirror: MirrorArgs {
            migrate_lfs: true,
            migrate_wiki: false,
        },
        ..mirror_repo(&servers.github, "octo-org/alpha")
    };

    commands::mirror_repository(cmd, &config).await.unwrap();
}

#[tokio::test]
async fn missing_credentials_name_the_argument_without_contacting_anything() {
    let (github, forgejo) = (MockServer::start().await, MockServer::start().await);
    let config = Config {
        github_token: None,
        ..config(&github, &forgejo)
    };

    let err = commands::mirror_repository(mirror_repo(&github, "o/r"), &config)
        .await
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "missing --github-token (cli) or githubToken (config file)"
    );

    let err = commands::delete_organisation(
        DeleteOrg {
            forgejo: no_forgejo_args(),
            yes: true,
            forgejo_organisation_name: "x".into(),
        },
        &Config::default(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "missing --forgejo-url (cli) or forgejoUrl (config file)"
    );

    assert!(requests(&github).await.is_empty());
    assert!(requests(&forgejo).await.is_empty());
}

#[tokio::test]
async fn delete_org_removes_every_repository_before_the_organisation() {
    let servers = Servers::start("7.0.0+gitea-1.22.0").await;
    mount_pages(
        &servers.forgejo,
        "/api/v1/orgs/mirror-org/repos",
        "page",
        &[fixture("forgejo/repos.json", &servers.github)],
    )
    .await;
    mount_status(
        &servers.forgejo,
        "DELETE",
        "/api/v1/repos/mirror-org/alpha",
        204,
    )
    .await;
    mount_status(
        &servers.forgejo,
        "DELETE",
        "/api/v1/repos/mirror-org/beta",
        204,
    )
    .await;
    mount_status(&servers.forgejo, "DELETE", "/api/v1/orgs/mirror-org", 204).await;
    let cmd = DeleteOrg {
        forgejo: no_forgejo_args(),
        yes: true,
        forgejo_organisation_name: "mirror-org".into(),
    };

    commands::delete_organisation(cmd, &servers.config())
        .await
        .unwrap();

    let deletes: Vec<String> = requests(&servers.forgejo)
        .await
        .iter()
        .filter(|r| r.method == "DELETE")
        .map(|r| r.url.path().to_string())
        .collect();
    assert_eq!(
        deletes,
        [
            "/api/v1/repos/mirror-org/alpha",
            "/api/v1/repos/mirror-org/beta",
            "/api/v1/orgs/mirror-org"
        ]
    );
}

#[tokio::test]
async fn delete_org_stops_at_the_first_failed_repository_deletion() {
    let servers = Servers::start("7.0.0+gitea-1.22.0").await;
    mount_pages(
        &servers.forgejo,
        "/api/v1/orgs/mirror-org/repos",
        "page",
        &[fixture("forgejo/repos.json", &servers.github)],
    )
    .await;
    mount_status(
        &servers.forgejo,
        "DELETE",
        "/api/v1/repos/mirror-org/alpha",
        403,
    )
    .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/orgs/mirror-org"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&servers.forgejo)
        .await;
    let cmd = DeleteOrg {
        forgejo: no_forgejo_args(),
        yes: true,
        forgejo_organisation_name: "mirror-org".into(),
    };

    let err = commands::delete_organisation(cmd, &servers.config())
        .await
        .unwrap_err();

    assert!(
        err.to_string().contains("Forgejo API returned 403"),
        "{err}"
    );
}
