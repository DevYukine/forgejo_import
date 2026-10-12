mod common;

use reqwest::Url;
use serde_json::json;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use common::{AVATAR_BYTES, GITHUB_TOKEN, fixture, mount_avatar, mount_pages, ok, requests};
use forgejo_import::github::GithubApi;

async fn api(server: &MockServer) -> GithubApi {
    GithubApi::with_base_url(Url::parse(&server.uri()).unwrap(), GITHUB_TOKEN.to_string()).unwrap()
}

#[tokio::test]
async fn requests_carry_the_bearer_token_and_parse_accounts() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/orgs/octo-org"))
        .and(header("authorization", "Bearer gh-token"))
        .and(header(
            "user-agent",
            concat!("forgejo_import/", env!("CARGO_PKG_VERSION")),
        ))
        .respond_with(ok(fixture("github/org.json", &server)))
        .expect(1)
        .mount(&server)
        .await;

    let org = api(&server).await.organisation("octo-org").await.unwrap();

    assert_eq!(org.login, "octo-org");
    assert_eq!(org.name.as_deref(), Some("Octo Org"));
    assert_eq!(org.description.as_deref(), Some(""));
    assert_eq!(org.blog.as_deref(), Some("octo.example"));
    assert_eq!(org.html_url, "https://github.com/octo-org");
    assert_eq!(
        org.avatar_url,
        format!("{}/avatars/u/583231?v=4", server.uri())
    );
}

#[tokio::test]
async fn users_without_org_only_fields_still_parse() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/octocat"))
        .respond_with(ok(fixture("github/user.json", &server)))
        .mount(&server)
        .await;

    let user = api(&server).await.user("octocat").await.unwrap();

    assert_eq!(user.login, "octocat");
    assert_eq!(user.name.as_deref(), Some("The Octocat"));
    assert_eq!(user.description, None);
    assert_eq!(user.blog.as_deref(), Some("https://github.blog"));
}

#[tokio::test]
async fn organisation_repositories_follow_pages_and_sort_by_size() {
    let server = MockServer::start().await;
    let repos = fixture("github/repos.json", &server);
    let (first, rest) = (
        json!([repos[0].clone(), repos[1].clone()]),
        json!([repos[2].clone()]),
    );
    mount_pages(&server, "/orgs/octo-org/repos", "page", &[first, rest]).await;

    let repos = api(&server)
        .await
        .organisation_repositories("octo-org")
        .await
        .unwrap();

    let names: Vec<&str> = repos.iter().map(|repo| repo.name.as_str()).collect();
    assert_eq!(names, ["empty", "beta", "alpha"]);
    assert!(repos[1].private, "beta is private on GitHub");
    assert_eq!(repos[0].description.as_deref(), Some(""));
    assert_eq!(repos[2].clone_url, "https://github.com/octo-org/alpha.git");

    let page_sizes: Vec<_> = requests(&server)
        .await
        .iter()
        .map(|req| req.url.query().unwrap().to_string())
        .collect();
    assert!(
        page_sizes
            .iter()
            .all(|query| query.contains("per_page=100")),
        "{page_sizes:?}"
    );
}

#[tokio::test]
async fn user_repositories_include_private_ones_for_the_token_owner() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(ok(
            json!({ "login": "OctoCat", "html_url": "", "avatar_url": "" }),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/user/repos"))
        .and(query_param("affiliation", "owner"))
        .and(query_param("page", "1"))
        .respond_with(ok(fixture("github/repos.json", &server)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/user/repos"))
        .and(query_param("page", "2"))
        .respond_with(ok(json!([])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/users/octocat/repos"))
        .respond_with(ok(json!([])))
        .expect(0)
        .mount(&server)
        .await;

    let repos = api(&server)
        .await
        .user_repositories("octocat")
        .await
        .unwrap();

    assert_eq!(repos.len(), 3);
}

#[tokio::test]
async fn user_repositories_of_other_users_use_the_public_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(ok(
            json!({ "login": "someone-else", "html_url": "", "avatar_url": "" }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/user/repos"))
        .respond_with(ok(json!([])))
        .expect(0)
        .mount(&server)
        .await;
    mount_pages(
        &server,
        "/users/octocat/repos",
        "page",
        &[json!([fixture("github/repos.json", &server)[0].clone()])],
    )
    .await;

    let repos = api(&server)
        .await
        .user_repositories("octocat")
        .await
        .unwrap();

    assert_eq!(repos.len(), 1);
}

#[tokio::test]
async fn avatars_are_downloaded_without_credentials() {
    let server = MockServer::start().await;
    mount_avatar(&server, "/avatars/u/1").await;

    let bytes = api(&server)
        .await
        .avatar(&format!("{}/avatars/u/1?v=4", server.uri()))
        .await
        .unwrap();

    assert_eq!(bytes.as_ref(), AVATAR_BYTES);
    let request = &requests(&server).await[0];
    assert!(
        request.headers.get("authorization").is_none(),
        "token leaked to avatar host"
    );
}

#[tokio::test]
async fn errors_carry_status_and_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octo/missing"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({ "message": "Not Found" })))
        .mount(&server)
        .await;

    let err = api(&server)
        .await
        .repository("octo", "missing")
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("GitHub API returned 404"), "{err}");
    assert!(err.contains("Not Found"), "{err}");
}

#[tokio::test]
async fn same_origin_redirects_are_followed_with_the_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/octo-org/old-name"))
        .respond_with(ResponseTemplate::new(301).insert_header("location", "/repos/octo-org/alpha"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/octo-org/alpha"))
        .and(header("authorization", "Bearer gh-token"))
        .respond_with(ok(fixture("github/repos.json", &server)[0].clone()))
        .expect(1)
        .mount(&server)
        .await;

    let repo = api(&server)
        .await
        .repository("octo-org", "old-name")
        .await
        .unwrap();

    assert_eq!(repo.name, "alpha");
}

#[tokio::test]
async fn cross_origin_redirects_are_reported_instead_of_followed() {
    let server = MockServer::start().await;
    let elsewhere = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/orgs/moved"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("location", format!("{}/orgs/moved", elsewhere.uri())),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/orgs/moved"))
        .respond_with(ok(fixture("github/org.json", &server)))
        .expect(0)
        .mount(&elsewhere)
        .await;

    let err = api(&server).await.organisation("moved").await.unwrap_err();

    let message = err.to_string();
    assert!(message.contains("302"), "{message}");
    assert!(
        message.contains(&format!("redirect to {}/orgs/moved", elsewhere.uri())),
        "{message}"
    );
}

#[tokio::test]
async fn user_repositories_fall_back_to_public_when_the_token_cannot_identify_itself() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(
            ResponseTemplate::new(403)
                .set_body_json(json!({ "message": "Resource not accessible by integration" })),
        )
        .expect(1)
        .mount(&server)
        .await;
    mount_pages(
        &server,
        "/users/octocat/repos",
        "page",
        &[json!([fixture("github/repos.json", &server)[0].clone()])],
    )
    .await;

    let repos = api(&server)
        .await
        .user_repositories("octocat")
        .await
        .unwrap();

    assert_eq!(repos.len(), 1);
}

#[tokio::test]
async fn names_that_would_change_the_endpoint_are_rejected() {
    let server = MockServer::start().await;
    let api = api(&server).await;

    for name in ["..", ".", ""] {
        let err = api.organisation(name).await.unwrap_err();
        assert!(err.to_string().contains("invalid name"), "{name:?}: {err}");
    }
    assert!(requests(&server).await.is_empty());
}

#[tokio::test]
async fn long_error_bodies_are_truncated_on_a_char_boundary() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/orgs/broken"))
        .respond_with(ResponseTemplate::new(502).set_body_string(format!("x{}", "ä".repeat(1000))))
        .mount(&server)
        .await;

    let err = api(&server).await.organisation("broken").await.unwrap_err();

    let message = err.to_string();
    let body = message.split_once(": ").unwrap().1;
    assert_eq!(body.chars().filter(|c| *c == 'ä').count(), 511);
    assert!(body.ends_with('…'), "{message}");
}

#[tokio::test]
async fn base_urls_without_host_or_http_scheme_are_rejected() {
    for url in ["ghe.example.com:443/api/v3", "file:///api/v3"] {
        let err = GithubApi::with_base_url(Url::parse(url).unwrap(), "t".to_string())
            .err()
            .unwrap_or_else(|| panic!("{url} must be rejected"));

        assert!(
            err.to_string()
                .contains("needs an http(s) scheme and a host"),
            "{url}: {err}"
        );
    }
}

#[tokio::test]
async fn user_repositories_propagate_unexpected_failures_of_the_identity_check() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(ResponseTemplate::new(502))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/users/octocat/repos"))
        .respond_with(ok(json!([])))
        .expect(0)
        .mount(&server)
        .await;

    let err = api(&server)
        .await
        .user_repositories("octocat")
        .await
        .unwrap_err();

    assert!(err.to_string().contains("502"), "{err}");
}

#[tokio::test]
async fn redirect_loops_stop_after_ten_hops() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/orgs/loop"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/orgs/loop"))
        .mount(&server)
        .await;

    let err = api(&server).await.organisation("loop").await.unwrap_err();

    assert!(format!("{err:#}").contains("too many redirects"), "{err:#}");
    assert_eq!(requests(&server).await.len(), 11);
}

#[tokio::test]
async fn avatar_downloads_follow_cross_origin_redirects() {
    let server = MockServer::start().await;
    let cdn = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/avatars/u/1"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("location", format!("{}/img.png", cdn.uri())),
        )
        .expect(1)
        .mount(&server)
        .await;
    mount_avatar(&cdn, "/img.png").await;

    let bytes = api(&server)
        .await
        .avatar(&format!("{}/avatars/u/1", server.uri()))
        .await
        .unwrap();

    assert_eq!(bytes.as_ref(), AVATAR_BYTES);
    assert!(
        requests(&cdn).await[0]
            .headers
            .get("authorization")
            .is_none()
    );
}
