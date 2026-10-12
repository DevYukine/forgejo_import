use std::io::Write;

use tempfile::NamedTempFile;

use forgejo_import::config::Config;

fn file(contents: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(contents.as_bytes()).unwrap();
    file
}

#[test]
fn parses_camel_case_keys_and_leaves_the_rest_unset() {
    let file = file(
        r#"{ "forgejoUrl": "https://forgejo.example", "githubToken": "gh", "githubApiUrl": "https://ghe.example/api/v3", "migrateWiki": true }"#,
    );

    let config = Config::load_from(file.path()).unwrap();

    assert_eq!(
        config.forgejo_url.as_deref(),
        Some("https://forgejo.example")
    );
    assert_eq!(config.forgejo_token, None);
    assert_eq!(config.github_token.as_deref(), Some("gh"));
    assert_eq!(
        config.github_api_url.as_deref(),
        Some("https://ghe.example/api/v3")
    );
    assert_eq!(config.migrate_wiki, Some(true));
    assert_eq!(config.migrate_lfs, None);
}

#[test]
fn rejects_unknown_keys_so_typos_do_not_pass_silently() {
    let file = file(r#"{ "forgejo_url": "https://forgejo.example" }"#);

    let err = Config::load_from(file.path())
        .err()
        .expect("unknown key must fail");

    let message = format!("{err:#}");
    assert!(message.contains("invalid config file"), "{message}");
    assert!(message.contains("unknown field `forgejo_url`"), "{message}");
}

#[test]
fn reports_the_path_of_an_unreadable_file() {
    let err = Config::load_from("/definitely/not/here.json".as_ref())
        .err()
        .expect("missing file must fail");

    assert!(
        format!("{err:#}").contains("cannot read config file /definitely/not/here.json"),
        "{err:#}"
    );
}
