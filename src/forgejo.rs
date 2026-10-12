use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use bytes::Bytes;
use clap::ValueEnum;
use log::{info, warn};
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

#[derive(Serialize)]
pub struct CreateOrganisation {
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,
    pub visibility: Visibility,
}

/// Body of `POST /repos/migrate`; see [`crate::cli::MirrorArgs`] for why only wiki and LFS are exposed.
#[derive(Serialize)]
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

#[derive(Debug, Deserialize)]
struct VersionResponse {
    version: String,
}

/// Gitea API level an instance is compatible with, taken from `GET /version`.
///
/// Forgejo 7+ reports `X.Y.Z+gitea-A.B.C` and the Gitea part decides which endpoints exist.
/// Forgejo 1.18 to 1.21 reported `A.B.C+N` and plain Gitea reports `A.B.C`; both use the leading part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ApiVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

/// `POST /orgs/{org}/avatar` was added in Gitea 1.21; Forgejo 1.18 to 1.20 have no API for it.
const ORG_AVATAR_SINCE: ApiVersion = ApiVersion::new(1, 21, 0);

impl ApiVersion {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub fn parse(raw: &str) -> Option<ApiVersion> {
        let compat = match raw.split_once("+gitea-") {
            Some((_, gitea)) => gitea,
            None => raw.split('+').next()?,
        };
        let core = compat.split('-').next()?;
        let mut parts = core.split('.').map(|part| part.parse::<u32>().ok());

        Some(ApiVersion::new(
            parts.next()??,
            parts.next()??,
            parts.next().flatten().unwrap_or(0),
        ))
    }
}

impl std::fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

pub struct ForgejoApi {
    rest: RestClient,
    version: String,
    api_version: Option<ApiVersion>,
}

impl ForgejoApi {
    /// Builds a client without version detection; every endpoint is then assumed to exist.
    pub fn new(url: &str, token: String) -> Result<Self> {
        let mut base = Url::parse(url).with_context(|| format!("invalid forgejo url: {url}"))?;

        if !base.path().trim_end_matches('/').ends_with("/api/v1") {
            base.path_segments_mut()
                .map_err(|()| anyhow!("invalid forgejo url: {url}"))?
                .pop_if_empty()
                .extend(["api", "v1"]);
        }

        Ok(Self {
            rest: RestClient::new("Forgejo", base, token)?,
            version: String::new(),
            api_version: None,
        })
    }

    /// Connects and detects the instance version so callers can skip unsupported endpoints.
    pub async fn connect(url: &str, token: String) -> Result<Self> {
        let mut api = Self::new(url, token)?;
        let VersionResponse { version } = api.rest.get(&["version"]).await?;

        api.api_version = ApiVersion::parse(&version);
        match api.api_version {
            Some(compat) => info!("Connected to Forgejo {version} (Gitea API {compat})"),
            None => warn!(
                "Connected to Forgejo with unrecognised version {version:?}, assuming every API feature is available"
            ),
        }
        api.version = version;

        Ok(api)
    }

    /// Raw version string as reported by the instance; empty before [`ForgejoApi::connect`].
    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn supports_organisation_avatar(&self) -> bool {
        self.api_version.is_none_or(|v| v >= ORG_AVATAR_SINCE)
    }

    pub async fn organisation_exists(&self, name: &str) -> Result<bool> {
        self.rest.exists(&["orgs", name]).await
    }

    pub async fn create_organisation(&self, org: &CreateOrganisation) -> Result<()> {
        self.rest.post(&["orgs"], org).await
    }

    /// Needs Gitea API 1.21 or newer; check [`ForgejoApi::supports_organisation_avatar`] first.
    pub async fn set_organisation_avatar(&self, name: &str, image: Bytes) -> Result<()> {
        let body = Avatar {
            image: STANDARD.encode(image),
        };

        self.rest.post(&["orgs", name, "avatar"], &body).await
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
        let request = self
            .rest
            .request(Method::POST, &["repos", "migrate"])?
            .json(request)
            .timeout(MIGRATE_TIMEOUT);
        self.rest.send(request).await?;

        Ok(())
    }

    pub async fn delete_repository(&self, owner: &str, name: &str) -> Result<()> {
        self.rest.delete(&["repos", owner, name]).await
    }
}

#[cfg(test)]
mod tests {
    use super::{ApiVersion, ORG_AVATAR_SINCE};

    fn parse(raw: &str) -> ApiVersion {
        ApiVersion::parse(raw).unwrap_or_else(|| panic!("{raw}"))
    }

    #[test]
    fn uses_the_gitea_compat_part_of_modern_forgejo_versions() {
        assert_eq!(parse("7.0.16+gitea-1.21.11"), ApiVersion::new(1, 21, 11));
        assert_eq!(parse("16.0.5+gitea-1.22.0"), ApiVersion::new(1, 22, 0));
        assert_eq!(
            parse("15.0.9-6-85b0721bc6+gitea-1.22.0"),
            ApiVersion::new(1, 22, 0)
        );
        assert_eq!(
            parse("12.0.0-dev-abc123+gitea-1.22.0"),
            ApiVersion::new(1, 22, 0)
        );
    }

    #[test]
    fn uses_the_leading_part_of_legacy_and_gitea_versions() {
        assert_eq!(parse("1.21.7+0"), ApiVersion::new(1, 21, 7));
        assert_eq!(parse("1.18.0+1"), ApiVersion::new(1, 18, 0));
        assert_eq!(parse("1.22.0"), ApiVersion::new(1, 22, 0));
        assert_eq!(parse("1.23.0+dev-123-gabcdef"), ApiVersion::new(1, 23, 0));
        assert_eq!(parse("1.24"), ApiVersion::new(1, 24, 0));
    }

    #[test]
    fn rejects_garbage() {
        for raw in ["", "dev", "1", "x.y.z", "+gitea-"] {
            assert!(ApiVersion::parse(raw).is_none(), "{raw}");
        }
    }

    #[test]
    fn org_avatar_boundary() {
        assert!(parse("1.20.6+0") < ORG_AVATAR_SINCE);
        assert!(parse("1.21.0+0") >= ORG_AVATAR_SINCE);
        assert!(parse("7.0.0+gitea-1.22.0") >= ORG_AVATAR_SINCE);
    }
}
