mod common;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use bytes::Bytes;
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use common::{FORGEJO_TOKEN, fixture, mount_pages, mount_status, mount_version, ok, requests};
use forgejo_import::forgejo::{CreateOrganisation, ForgejoApi, MigrateRepository, Visibility};

fn api(server: &MockServer) -> ForgejoApi {
    ForgejoApi::new(&server.uri(), FORGEJO_TOKEN.to_string()).unwrap()
}

/// Version parsing itself is unit-tested in src/forgejo.rs; this only covers the connect wiring.
#[tokio::test]
async fn connect_reads_the_version_and_gates_the_avatar_api() {
    for (version, supports_avatar) in [
        ("1.20.6+0", false),
        ("7.0.16+gitea-1.21.11", true),
        ("not-a-version", true),
    ] {
        let server = MockServer::start().await;
        mount_version(&server, version).await;

        let api = ForgejoApi::connect(&server.uri(), FORGEJO_TOKEN.to_string())
            .await
            .unwrap();

        assert_eq!(api.version(), version);
        assert_eq!(
            api.supports_organisation_avatar(),
            supports_avatar,
            "{version}"
        );
    }
}

#[tokio::test]
async fn base_url_keeps_sub_paths_and_tolerates_trailing_slashes_and_api_suffix() {
    for (suffix, expected) in [
        ("", "/api/v1/version"),
        ("/", "/api/v1/version"),
        ("/forge", "/forge/api/v1/version"),
        ("/forge/", "/forge/api/v1/version"),
        ("/api/v1", "/api/v1/version"),
        ("/forge/api/v1/", "/forge/api/v1/version"),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(expected))
            .and(header("authorization", "Bearer fj-token"))
            .respond_with(ok(json!({ "version": "7.0.0+gitea-1.22.0" })))
            .expect(1)
            .mount(&server)
            .await;

        ForgejoApi::connect(
            &format!("{}{suffix}", server.uri()),
            FORGEJO_TOKEN.to_string(),
        )
        .await
        .unwrap_or_else(|err| panic!("{suffix:?}: {err}"));
    }
}

#[tokio::test]
async fn urls_without_a_scheme_are_rejected_up_front() {
    let err = ForgejoApi::new("forgejo.example.org", "t".to_string())
        .err()
        .expect("url without scheme must be rejected");

    assert!(err.to_string().contains("invalid forgejo url"), "{err}");
}

#[tokio::test]
async fn names_are_encoded_as_single_path_segments() {
    let server = MockServer::start().await;
    mount_status(&server, "GET", "/api/v1/orgs/a%2F..%2Fb", 404).await;

    let exists = api(&server).organisation_exists("a/../b").await.unwrap();

    assert!(!exists);
    let paths: Vec<_> = requests(&server)
        .await
        .iter()
        .map(|r| r.url.path().to_string())
        .collect();
    assert_eq!(paths, ["/api/v1/orgs/a%2F..%2Fb"]);
}

#[tokio::test]
async fn exists_distinguishes_not_found_from_failures() {
    let server = MockServer::start().await;
    mount_status(&server, "GET", "/api/v1/repos/o/missing", 404).await;
    mount_status(&server, "GET", "/api/v1/repos/o/present", 200).await;
    mount_status(&server, "GET", "/api/v1/repos/o/broken", 500).await;
    let api = api(&server);

    assert!(!api.repository_exists("o", "missing").await.unwrap());
    assert!(api.repository_exists("o", "present").await.unwrap());
    let err = api.repository_exists("o", "broken").await.unwrap_err();
    assert!(
        err.to_string().contains("Forgejo API returned 500"),
        "{err}"
    );
    let paths: Vec<_> = requests(&server)
        .await
        .iter()
        .map(|r| r.url.path().to_string())
        .collect();
    assert_eq!(
        paths,
        [
            "/api/v1/repos/o/missing",
            "/api/v1/repos/o/present",
            "/api/v1/repos/o/broken"
        ]
    );
}

#[tokio::test]
async fn organisation_repositories_follow_pages_with_limit() {
    let server = MockServer::start().await;
    let repos = fixture("forgejo/repos.json", &server);
    mount_pages(
        &server,
        "/api/v1/orgs/mirror-org/repos",
        "page",
        &[json!([repos[0]]), json!([repos[1]])],
    )
    .await;

    let repos = api(&server)
        .organisation_repositories("mirror-org")
        .await
        .unwrap();

    let names: Vec<&str> = repos.iter().map(|repo| repo.name.as_str()).collect();
    assert_eq!(names, ["alpha", "beta"]);
    for request in requests(&server).await {
        assert!(
            request.url.query().unwrap().contains("limit=50"),
            "{}",
            request.url
        );
    }
}

#[tokio::test]
async fn migrate_posts_the_exact_body() {
    let server = MockServer::start().await;
    let request = MigrateRepository {
        auth_token: "gh-token".into(),
        clone_addr: "https://github.com/octo-org/alpha.git".into(),
        description: "[MIRROR] First repository".into(),
        lfs: true,
        mirror: true,
        private: true,
        repo_name: "alpha".into(),
        repo_owner: "mirror-org".into(),
        service: "github",
        wiki: false,
    };
    Mock::given(method("POST"))
        .and(path("/api/v1/repos/migrate"))
        .and(header("content-type", "application/json"))
        .and(body_json(json!({
            "auth_token": "gh-token",
            "clone_addr": "https://github.com/octo-org/alpha.git",
            "description": "[MIRROR] First repository",
            "lfs": true,
            "mirror": true,
            "private": true,
            "repo_name": "alpha",
            "repo_owner": "mirror-org",
            "service": "github",
            "wiki": false
        })))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&server)
        .await;

    api(&server).migrate_repository(&request).await.unwrap();
}

#[tokio::test]
async fn create_organisation_omits_absent_optional_fields() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/orgs"))
        .and(body_json(json!({
            "username": "mirror-org",
            "description": "Mirror of https://github.com/octo-org",
            "visibility": "limited"
        })))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&server)
        .await;

    api(&server)
        .create_organisation(&CreateOrganisation {
            username: "mirror-org".into(),
            full_name: None,
            description: "Mirror of https://github.com/octo-org".into(),
            website: None,
            visibility: Visibility::Limited,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn avatars_are_sent_base64_encoded() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/orgs/mirror-org/avatar"))
        .and(body_json(json!({ "image": STANDARD.encode(b"img") })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    api(&server)
        .set_organisation_avatar("mirror-org", Bytes::from_static(b"img"))
        .await
        .unwrap();
}

#[tokio::test]
async fn deletes_use_the_delete_method() {
    let server = MockServer::start().await;
    mount_status(&server, "DELETE", "/api/v1/repos/mirror-org/alpha", 204).await;
    mount_status(&server, "DELETE", "/api/v1/orgs/mirror-org", 204).await;
    let api = api(&server);

    api.delete_repository("mirror-org", "alpha").await.unwrap();
    api.delete_organisation("mirror-org").await.unwrap();
}

#[tokio::test]
async fn mutating_requests_never_follow_redirects() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/orgs/old-name"))
        .respond_with(ResponseTemplate::new(307).insert_header("location", "/api/v1/orgs/new-name"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/repos/migrate"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/api/v1/repos/x"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/orgs/new-name"))
        .respond_with(ResponseTemplate::new(204))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/repos/x"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let api = api(&server);

    let err = api.delete_organisation("old-name").await.unwrap_err();
    assert!(
        err.to_string()
            .contains("307 Temporary Redirect (redirect to /api/v1/orgs/new-name)"),
        "{err}"
    );

    let request = MigrateRepository {
        auth_token: "t".into(),
        clone_addr: "https://github.com/o/r.git".into(),
        description: String::new(),
        lfs: false,
        mirror: true,
        private: false,
        repo_name: "r".into(),
        repo_owner: "o".into(),
        service: "github",
        wiki: false,
    };
    let err = api.migrate_repository(&request).await.unwrap_err();
    assert!(err.to_string().contains("302"), "{err}");
}

#[tokio::test]
async fn existence_probes_treat_a_redirect_as_an_error_not_as_present() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/repos/o/old"))
        .respond_with(ResponseTemplate::new(307).insert_header("location", "/api/v1/repos/o/new"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/repos/o/new"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let err = api(&server)
        .repository_exists("o", "old")
        .await
        .unwrap_err();

    assert!(err.to_string().contains("307"), "{err}");
}
