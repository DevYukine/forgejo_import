use std::time::Duration;

use anyhow::{anyhow, Error};
use base64::{engine::general_purpose, Engine as _};
use bytes::Bytes;
use general_purpose::STANDARD;
use reqwest::header::HeaderMap;
use reqwest::{Client, Method, Request, Response, StatusCode, Url};
use serde::de::DeserializeOwned;
use tower::limit::RateLimit;
use tower::timeout::Timeout;
use tower::{Service, ServiceExt};

use crate::forgejo::error::ForgejoApiError;
use crate::forgejo::models::{
    ForgejoCreateOrganisationRequest, ForgejoGetOrganisationRepositoriesResponse,
    ForgejoMigrateRepositoryRequest, ForgejoRepository, ForgejoUpdateUserAvatarRequest,
};
use crate::util::http::{CLIENT, USER_AGENT};

const API_VERSION: &str = "1";

pub(crate) struct ForgejoApi {
    client: Client,
    headers: HeaderMap,
    service: Timeout<RateLimit<Client>>,
    base_url: String,
}

impl ForgejoApi {
    pub fn new(base_url: String, api_key: String) -> anyhow::Result<Self> {
        let client = CLIENT.clone();

        let mut headers = HeaderMap::new();

        headers.insert("User-Agent", USER_AGENT.parse()?);
        headers.insert("Accept", "application/json".parse()?);
        headers.insert("Authorization", format!("token {}", api_key).parse()?);

        let service = tower::ServiceBuilder::new()
            .timeout(Duration::from_secs(300))
            .rate_limit(15, Duration::from_secs(5))
            .service(client.clone());

        Ok(ForgejoApi {
            client,
            headers,
            service,
            base_url,
        })
    }

    pub async fn create_organization(
        &mut self,
        options: &ForgejoCreateOrganisationRequest,
    ) -> anyhow::Result<()> {
        let req = self
            .client
            .request(
                Method::POST,
                Url::parse(&format!("{}/api/v{}/orgs", &self.base_url, API_VERSION))?,
            )
            .headers(self.headers.clone())
            .json(options)
            .build()?;

        self.do_request_handle_status(req).await?;

        Ok(())
    }

    pub async fn organisation_exists(&mut self, name: &str) -> anyhow::Result<bool> {
        let req = self
            .client
            .request(
                Method::GET,
                Url::parse(&format!(
                    "{}/api/v{}/orgs/{}",
                    &self.base_url, API_VERSION, name
                ))?,
            )
            .headers(self.headers.clone())
            .build()?;

        let res = self.do_request(req).await?;

        let status = res.status();

        if !status.is_success() && status != StatusCode::NOT_FOUND {
            return Err(Error::from(ForgejoApiError::NoSuccessStatusCodeError(
                status,
                res.text().await?,
            )));
        }

        let status = res.status();

        Ok(status != StatusCode::NOT_FOUND)
    }

    pub async fn set_organisation_avatar(
        &mut self,
        name: &str,
        avatar_bytes: Bytes,
    ) -> anyhow::Result<()> {
        let req = self
            .client
            .request(
                Method::POST,
                Url::parse(&format!(
                    "{}/api/v{}/orgs/{}/avatar",
                    &self.base_url, API_VERSION, name
                ))?,
            )
            .headers(self.headers.clone())
            .json(&ForgejoUpdateUserAvatarRequest {
                image: STANDARD.encode(avatar_bytes),
            })
            .build()?;

        self.do_request_handle_status(req).await?;

        Ok(())
    }

    pub async fn delete_organisation(&mut self, name: &str) -> anyhow::Result<()> {
        let req = self
            .client
            .request(
                Method::DELETE,
                Url::parse(&format!(
                    "{}/api/v{}/orgs/{}",
                    &self.base_url, API_VERSION, name
                ))?,
            )
            .headers(self.headers.clone())
            .build()?;

        self.do_request_handle_status(req).await?;

        Ok(())
    }

    pub async fn get_organisation_repositories(
        &mut self,
        name: &str,
    ) -> anyhow::Result<Vec<ForgejoRepository>> {
        let mut repos = Vec::new();

        let mut index = 1;
        let mut last_response_count = 0;

        while index == 1 || last_response_count > 0 {
            let req = self
                .client
                .request(
                    Method::GET,
                    Url::parse(&format!(
                        "{}/api/v{}/orgs/{}/repos",
                        &self.base_url, API_VERSION, name
                    ))?,
                )
                .query(&[("page", index), ("per_page", 25)])
                .headers(self.headers.clone())
                .build()?;

            let res = self
                .do_request_handle_status_parsed::<ForgejoGetOrganisationRepositoriesResponse>(req)
                .await?;

            index += 1;
            last_response_count = res.len();

            repos.extend(res);
        }

        Ok(repos)
    }

    pub async fn mirror_repository(
        &mut self,
        options: &ForgejoMigrateRepositoryRequest,
    ) -> anyhow::Result<()> {
        let req = self
            .client
            .request(
                Method::POST,
                Url::parse(&format!(
                    "{}/api/v{}/repos/migrate",
                    &self.base_url, API_VERSION
                ))?,
            )
            .headers(self.headers.clone())
            .json(options)
            .build()?;

        self.do_request_handle_status(req).await?;

        Ok(())
    }

    pub async fn repository_exists(&mut self, owner: &str, name: &str) -> anyhow::Result<bool> {
        let req = self
            .client
            .request(
                Method::GET,
                Url::parse(&format!(
                    "{}/api/v{}/repos/{}/{}",
                    &self.base_url, API_VERSION, owner, name
                ))?,
            )
            .headers(self.headers.clone())
            .build()?;

        let res = self.do_request(req).await?;

        let status = res.status();

        if !status.is_success() && status != StatusCode::NOT_FOUND {
            return Err(Error::from(ForgejoApiError::NoSuccessStatusCodeError(
                status,
                res.text().await?,
            )));
        }

        Ok(status != StatusCode::NOT_FOUND)
    }

    pub async fn delete_repository(&mut self, owner: &str, name: &str) -> anyhow::Result<()> {
        let req = self
            .client
            .request(
                Method::DELETE,
                Url::parse(&format!(
                    "{}/api/v{}/repos/{}/{}",
                    &self.base_url, API_VERSION, owner, name
                ))?,
            )
            .headers(self.headers.clone())
            .build()?;

        self.do_request_handle_status(req).await?;

        Ok(())
    }

    async fn do_request_handle_status_parsed<T: DeserializeOwned>(
        &mut self,
        req: Request,
    ) -> anyhow::Result<T> {
        let res = self.do_request_handle_status(req).await?;

        Ok(res.json::<T>().await?)
    }

    async fn do_request_handle_status(&mut self, request: Request) -> anyhow::Result<Response> {
        let res = self.do_request(request).await?;

        let status = res.status();

        return if status.is_success() {
            Ok(res)
        } else {
            let response_text = res.text().await?;

            Err(Error::from(ForgejoApiError::NoSuccessStatusCodeError(
                status,
                response_text,
            )))
        };
    }

    async fn do_request(&mut self, req: Request) -> anyhow::Result<Response> {
        let res = self
            .service
            .ready()
            .await
            .map_err(|err| anyhow!(err))?
            .call(req)
            .await
            .map_err(|err| anyhow!(err))?;

        Ok(res)
    }
}
