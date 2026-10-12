use std::time::Duration;

use anyhow::{Context, Result, bail};
use bytes::Bytes;
use reqwest::{Client, Method, RequestBuilder, Response, StatusCode, Url, redirect};
use serde::Serialize;
use serde::de::DeserializeOwned;

const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Thin token-authenticated JSON client bound to one API base URL.
///
/// Redirects are disabled so a bearer token or request body is never replayed to another host.
pub struct RestClient {
    client: Client,
    base: Url,
    token: String,
    service: &'static str,
}

impl RestClient {
    pub fn new(service: &'static str, base: Url, token: String) -> Result<Self> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(CONNECT_TIMEOUT)
            .redirect(redirect::Policy::none())
            .build()?;

        Ok(Self {
            client,
            base,
            token,
            service,
        })
    }

    pub fn base(&self) -> &Url {
        &self.base
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    /// Builds an authenticated request for `path` segments appended to the base URL.
    /// Segments are percent-encoded, so names containing `/` or `..` cannot escape the endpoint.
    pub fn request(&self, method: Method, path: &[&str]) -> RequestBuilder {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .expect("base url is not cannot-be-a-base")
            .pop_if_empty()
            .extend(path);

        self.client
            .request(method, url)
            .bearer_auth(&self.token)
            .timeout(REQUEST_TIMEOUT)
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &[&str]) -> Result<T> {
        self.send(self.request(Method::GET, path))
            .await?
            .json()
            .await
            .with_context(|| format!("{} API returned an unexpected body", self.service))
    }

    /// Fetches every page of a list endpoint; `query` must contain the page size parameter.
    pub async fn get_all<T: DeserializeOwned>(
        &self,
        path: &[&str],
        query: &[(&str, &str)],
    ) -> Result<Vec<T>> {
        let mut all = Vec::new();

        for page in 1u32.. {
            let items: Vec<T> = self
                .send(
                    self.request(Method::GET, path)
                        .query(query)
                        .query(&[("page", page)]),
                )
                .await?
                .json()
                .await
                .with_context(|| format!("{} API returned an unexpected body", self.service))?;

            if items.is_empty() {
                break;
            }

            all.extend(items);
        }

        Ok(all)
    }

    pub async fn exists(&self, path: &[&str]) -> Result<bool> {
        let res = self.request(Method::GET, path).send().await?;

        if res.status() == StatusCode::NOT_FOUND {
            return Ok(false);
        }

        self.check(res).await?;

        Ok(true)
    }

    pub async fn post<B: Serialize + ?Sized>(&self, path: &[&str], body: &B) -> Result<Response> {
        self.send(self.request(Method::POST, path).json(body)).await
    }

    pub async fn delete(&self, path: &[&str]) -> Result<()> {
        self.send(self.request(Method::DELETE, path)).await?;

        Ok(())
    }

    /// Downloads an arbitrary URL without credentials.
    pub async fn download(&self, url: &str) -> Result<Bytes> {
        let res = self
            .send(self.client.get(url).timeout(REQUEST_TIMEOUT))
            .await?;

        Ok(res.bytes().await?)
    }

    pub async fn send(&self, request: RequestBuilder) -> Result<Response> {
        self.check(request.send().await?).await
    }

    async fn check(&self, res: Response) -> Result<Response> {
        let status = res.status();

        if status.is_success() {
            return Ok(res);
        }

        let body = res.text().await.unwrap_or_default();

        bail!("{} API returned {status}: {body}", self.service)
    }
}
