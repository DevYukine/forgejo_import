use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use bytes::Bytes;
use clap::ValueEnum;
use log::warn;
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};

use crate::http::RestClient;

/// Forgejo caps `limit` at `[api] MAX_RESPONSE_ITEMS` (50 by default), so paging stops on an empty page.
const PAGE_SIZE: &str = "50";
/// `POST /repos/migrate` blocks until the clone is done, which can take long for big repositories.
const MIGRATE_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    Limited,
    Private,
}

#[derive(Debug, Serialize)]
pub struct CreateOrganisation {
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,
    pub visibility: Visibility,
}

/// Body of `POST /repos/migrate`. For mirrors Forgejo ignores issues, labels, pull requests,
/// releases and milestones, so only wiki and LFS are exposed.
#[derive(Debug, Serialize)]
pub struct MigrateRepository {
    pub auth_token: String,
    pub clone_addr: String,
    pub description: String,
    pub lfs: bool,
    pub mirror: bool,
    pub private: bool,
    pub repo_name: String,
    pub repo_owner: String,
    pub service: &'static str,
    pub wiki: bool,
}

#[derive(Debug, Deserialize)]
pub struct Repository {
    pub name: String,
}

#[derive(Serialize)]
struct Avatar {
    image: String,
}

pub struct ForgejoApi {
    rest: RestClient,
}

impl ForgejoApi {
    pub fn new(url: &str, token: String) -> Result<Self> {
        let mut base = Url::parse(url).with_context(|| format!("invalid forgejo url: {url}"))?;
        base.path_segments_mut()
            .map_err(|()| anyhow!("invalid forgejo url: {url}"))?
            .pop_if_empty()
            .extend(["api", "v1"]);

        if base.scheme() == "http" {
            warn!("Forgejo url uses plain http, tokens are sent unencrypted");
        }

        Ok(Self {
            rest: RestClient::new("Forgejo", base, token)?,
        })
    }

    pub async fn organisation_exists(&self, name: &str) -> Result<bool> {
        self.rest.exists(&["orgs", name]).await
    }

    pub async fn create_organisation(&self, org: &CreateOrganisation) -> Result<()> {
        self.rest.post(&["orgs"], org).await?;

        Ok(())
    }

    pub async fn set_organisation_avatar(&self, name: &str, image: Bytes) -> Result<()> {
        let body = Avatar {
            image: STANDARD.encode(image),
        };
        self.rest.post(&["orgs", name, "avatar"], &body).await?;

        Ok(())
    }

    pub async fn delete_organisation(&self, name: &str) -> Result<()> {
        self.rest.delete(&["orgs", name]).await
    }

    pub async fn organisation_repositories(&self, name: &str) -> Result<Vec<Repository>> {
        self.rest
            .get_all(&["orgs", name, "repos"], &[("limit", PAGE_SIZE)])
            .await
    }

    pub async fn repository_exists(&self, owner: &str, name: &str) -> Result<bool> {
        self.rest.exists(&["repos", owner, name]).await
    }

    pub async fn migrate_repository(&self, request: &MigrateRepository) -> Result<()> {
        self.rest
            .send(
                self.rest
                    .request(Method::POST, &["repos", "migrate"])
                    .json(request)
                    .timeout(MIGRATE_TIMEOUT),
            )
            .await?;

        Ok(())
    }

    pub async fn delete_repository(&self, owner: &str, name: &str) -> Result<()> {
        self.rest.delete(&["repos", owner, name]).await
    }
}
