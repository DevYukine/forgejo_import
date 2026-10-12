#![allow(dead_code)]

use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

use forgejo_import::cli::{ForgejoArgs, GithubArgs, MirrorArgs};
use forgejo_import::config::Config;

pub const GITHUB_TOKEN: &str = "gh-token";
pub const FORGEJO_TOKEN: &str = "fj-token";
pub const AVATAR_BYTES: &[u8] = b"\x89PNG not really";

/// Loads a JSON fixture; `{{github}}` is replaced by the GitHub mock's URL so avatar URLs resolve.
pub fn fixture(name: &str, github: &MockServer) -> Value {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let text = fs::read_to_string(&file).unwrap_or_else(|err| panic!("{}: {err}", file.display()));

    serde_json::from_str(&text.replace("{{github}}", &github.uri())).unwrap()
}

pub fn ok(body: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(body)
}

/// Mounts one page of a list endpoint; the page after the last one answers `[]` so paging stops.
pub async fn mount_pages(server: &MockServer, url_path: &str, page_param: &str, pages: &[Value]) {
    for (index, page) in pages.iter().enumerate() {
        Mock::given(method("GET"))
            .and(path(url_path))
            .and(query_param(page_param, (index + 1).to_string()))
            .respond_with(ok(page.clone()))
            .expect(1)
            .mount(server)
            .await;
    }

    Mock::given(method("GET"))
        .and(path(url_path))
        .and(query_param(page_param, (pages.len() + 1).to_string()))
        .respond_with(ok(json!([])))
        .expect(1)
        .mount(server)
        .await;
}

pub async fn mount_version(forgejo: &MockServer, version: &str) {
    Mock::given(method("GET"))
        .and(path("/api/v1/version"))
        .respond_with(ok(json!({ "version": version })))
        .expect(1)
        .mount(forgejo)
        .await;
}

pub async fn mount_status(server: &MockServer, http_method: &str, url_path: &str, status: u16) {
    Mock::given(method(http_method))
        .and(path(url_path))
        .respond_with(ResponseTemplate::new(status))
        .expect(1)
        .mount(server)
        .await;
}

pub async fn mount_avatar(github: &MockServer, url_path: &str) {
    Mock::given(method("GET"))
        .and(path(url_path))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(AVATAR_BYTES))
        .expect(1)
        .mount(github)
        .await;
}

/// Config pointing both APIs at the mocks, with every credential supplied.
pub fn config(github: &MockServer, forgejo: &MockServer) -> Config {
    Config {
        forgejo_url: Some(forgejo.uri()),
        forgejo_token: Some(FORGEJO_TOKEN.to_string()),
        github_token: Some(GITHUB_TOKEN.to_string()),
        github_api_url: Some(github.uri()),
        migrate_wiki: None,
        migrate_lfs: None,
    }
}

pub fn no_forgejo_args() -> ForgejoArgs {
    ForgejoArgs {
        forgejo_url: None,
        forgejo_token: None,
    }
}

pub fn no_github_args() -> GithubArgs {
    GithubArgs {
        github_token: None,
        github_api_url: None,
    }
}

pub fn no_mirror_args() -> MirrorArgs {
    MirrorArgs {
        migrate_lfs: false,
        migrate_wiki: false,
    }
}

pub async fn requests(server: &MockServer) -> Vec<Request> {
    server.received_requests().await.unwrap_or_default()
}

pub fn body_json(request: &Request) -> Value {
    serde_json::from_slice(&request.body).unwrap()
}
