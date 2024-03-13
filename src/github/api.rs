use anyhow::Error;
use bytes::Bytes;
use reqwest::{Client, Method, Request, Response};
use reqwest::header::HeaderMap;
use serde::de::DeserializeOwned;
use tower::{Service, ServiceExt};
use tower::limit::RateLimit;

use crate::github::constants::API_URL;
use crate::github::error::GithubApiError;
use crate::github::models::{
    GithubOrganisation, GithubOrganisationRepositoryResponse, GithubRepository, GithubUser,
};
use crate::util::http::{CLIENT, USER_AGENT};

pub struct GithubApi {
    client: Client,
    headers: HeaderMap,
    service: RateLimit<Client>,
}

impl GithubApi {
    pub fn new(api_key: String) -> anyhow::Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert("User-Agent", USER_AGENT.parse()?);
        headers.insert("Accept", "application/json".parse()?);
        headers.insert("Authorization", format!("Bearer {}", api_key).parse()?);

        let client = CLIENT.clone();

        let service = tower::ServiceBuilder::new()
            .rate_limit(10, std::time::Duration::from_secs(10))
            .service(client.clone());

        Ok(Self {
            client,
            service,
            headers,
        })
    }

    pub async fn get_user(&mut self, user: &str) -> anyhow::Result<GithubUser> {
        let req = self
            .client
            .request(Method::GET, format!("{}/users/{}", API_URL, user))
            .headers(self.headers.clone())
            .build()?;

        self.do_request_handle_status_parsed::<GithubUser>(req)
            .await
    }

    pub async fn get_user_avatar(&mut self, user: &str) -> anyhow::Result<Bytes> {
        let user = self.get_user(user).await?;

        let req = self
            .client
            .request(Method::GET, user.avatar_url)
            .headers(self.headers.clone())
            .build()?;

        let res = self.do_request_handle_status(req).await?;

        Ok(res.bytes().await?)
    }

    pub async fn get_repositories_of_user(
        &mut self,
        user: &str,
    ) -> anyhow::Result<Vec<GithubRepository>> {
        let mut repos = Vec::new();

        let mut last_response_count = 0;
        let mut index = 1;

        while index == 1 || last_response_count > 0 {
            let url = format!(
                "{}/users/{}/repos?page={}&per_page=25",
                API_URL, user, index
            );

            let req = self
                .client
                .request(Method::GET, url)
                .headers(self.headers.clone())
                .build()?;

            let res = self
                .do_request_handle_status_parsed::<Vec<GithubRepository>>(req)
                .await?;

            index += 1;

            if res.len() > 0 {
                last_response_count = res.len();

                repos.extend(res);
            } else {
                last_response_count = 0;
            }
        }

        repos.sort_by(|a, b| a.size.cmp(&b.size));

        Ok(repos)
    }

    pub async fn get_organisation(&mut self, org: &str) -> anyhow::Result<GithubOrganisation> {
        let req = self
            .client
            .request(Method::GET, format!("{}/orgs/{}", API_URL, org))
            .headers(self.headers.clone())
            .build()?;

        self.do_request_handle_status_parsed::<GithubOrganisation>(req)
            .await
    }

    pub async fn get_organisation_avatar(&mut self, org: &str) -> anyhow::Result<Bytes> {
        let org = self.get_organisation(org).await?;

        let req = self
            .client
            .request(Method::GET, org.avatar_url)
            .headers(self.headers.clone())
            .build()?;

        let res = self.do_request_handle_status(req).await?;

        Ok(res.bytes().await?)
    }

    pub async fn get_repositories_of_org(
        &mut self,
        org: &str,
    ) -> anyhow::Result<Vec<GithubRepository>> {
        let mut repos = Vec::new();

        let mut last_response_count = 0;
        let mut index = 1;

        while index == 1 || last_response_count > 0 {
            let url = format!("{}/orgs/{}/repos?page={}&per_page=25", API_URL, org, index);

            let req = self
                .client
                .request(Method::GET, url)
                .headers(self.headers.clone())
                .build()?;

            let res = self
                .do_request_handle_status_parsed::<GithubOrganisationRepositoryResponse>(req)
                .await?;

            index += 1;

            if res.len() > 0 {
                last_response_count = res.len();

                repos.extend(res);
            } else {
                last_response_count = 0;
            }
        }

        repos.sort_by(|a, b| a.size.cmp(&b.size));

        Ok(repos)
    }

    pub async fn get_repository(
        &mut self,
        owner: &str,
        repo: &str,
    ) -> anyhow::Result<GithubRepository> {
        let req = self
            .client
            .request(Method::GET, format!("{}/repos/{}/{}", API_URL, owner, repo))
            .headers(self.headers.clone())
            .build()?;

        self.do_request_handle_status_parsed::<GithubRepository>(req)
            .await
    }

    async fn do_request_handle_status_parsed<T: DeserializeOwned>(
        &mut self,
        req: Request,
    ) -> anyhow::Result<T> {
        let res = self.do_request_handle_status(req).await?;

        Ok(res.json().await?)
    }

    async fn do_request_handle_status(&mut self, request: Request) -> anyhow::Result<Response> {
        let res = self.do_request(request).await?;

        let status = res.status();

        return if status.is_success() {
            Ok(res)
        } else {
            let response_text = res.text().await?;

            Err(Error::from(GithubApiError::NoSuccessStatusCodeError(
                status,
                response_text,
            )))
        };
    }

    async fn do_request(&mut self, req: Request) -> anyhow::Result<Response> {
        let res = self.service.ready().await?.call(req).await?;

        Ok(res)
    }
}
