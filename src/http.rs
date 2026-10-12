use std::time::Duration;

use anyhow::{Context, Result, bail};
use bytes::Bytes;
use log::warn;
use reqwest::header::LOCATION;
use reqwest::{Client, Method, RequestBuilder, Response, StatusCode, Url, redirect};
use serde::Serialize;
use serde::de::DeserializeOwned;

const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// Generous because Forgejo deletes repositories synchronously.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_REDIRECTS: usize = 10;
/// Enough of an error body for an API message, not a whole HTML page.
const ERROR_BODY_LIMIT: usize = 1024;

/// Thin token-authenticated JSON client bound to one API base URL.
pub struct RestClient {
    client: Client,
    base: Url,
    token: String,
    service: &'static str,
}

impl RestClient {
    pub fn new(service: &'static str, base: Url, token: String) -> Result<Self> {
        if base.cannot_be_a_base() {
            bail!("{service} url needs a scheme and a host: {base}");
        }

        if base.scheme() == "http" {
            warn!("{service} url uses plain http, the token is sent unencrypted");
        }

        let client = Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(CONNECT_TIMEOUT)
            .redirect(same_origin_redirects())
            .build()?;

        Ok(Self {
            client,
            base,
            token,
            service,
        })
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    /// Builds an authenticated request for `path` segments appended to the base URL.
    /// Segments are percent-encoded; `.` and `..` are refused because the url crate would
    /// collapse them and silently target another endpoint.
    pub fn request(&self, method: Method, path: &[&str]) -> Result<RequestBuilder> {
        if let Some(segment) = path.iter().find(|s| matches!(**s, "" | "." | "..")) {
            bail!("invalid name {segment:?}");
        }

        let mut url = self.base.clone();
        url.path_segments_mut()
            .expect("validated in new")
            .pop_if_empty()
            .extend(path);

        Ok(self
            .client
            .request(method, url)
            .bearer_auth(&self.token)
            .timeout(REQUEST_TIMEOUT))
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &[&str]) -> Result<T> {
        self.send(self.request(Method::GET, path)?)
            .await?
            .json()
            .await
            .with_context(|| format!("{} API returned an unexpected body", self.service))
    }

    /// Fetches every page of a list endpoint until an empty page; `query` must contain the page size.
    pub async fn get_all<T: DeserializeOwned>(
        &self,
        path: &[&str],
        query: &[(&str, &str)],
    ) -> Result<Vec<T>> {
        let mut all = Vec::new();

        for page in 1u32.. {
            let request = self
                .request(Method::GET, path)?
                .query(query)
                .query(&[("page", page)]);
            let items: Vec<T> = self
                .send(request)
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
        let res = self.request(Method::GET, path)?.send().await?;

        if res.status() == StatusCode::NOT_FOUND {
            return Ok(false);
        }

        self.check(res).await?;

        Ok(true)
    }

    pub async fn post<B: Serialize + ?Sized>(&self, path: &[&str], body: &B) -> Result<()> {
        self.send(self.request(Method::POST, path)?.json(body))
            .await?;

        Ok(())
    }

    pub async fn delete(&self, path: &[&str]) -> Result<()> {
        self.send(self.request(Method::DELETE, path)?).await?;

        Ok(())
    }

    /// Downloads an arbitrary URL without credentials.
    pub async fn download(&self, url: &str) -> Result<Bytes> {
        let res = self
            .send(self.client.get(url).timeout(REQUEST_TIMEOUT))
            .await
            .with_context(|| format!("downloading {url}"))?;

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

        let location = res
            .headers()
            .get(LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(|location| format!(" (redirect to {location})"))
            .unwrap_or_default();
        let mut body = res.text().await.unwrap_or_default();

        if body.len() > ERROR_BODY_LIMIT {
            let cut = (0..=ERROR_BODY_LIMIT)
                .rev()
                .find(|index| body.is_char_boundary(*index))
                .unwrap_or(0);
            body.truncate(cut);
            body.push('…');
        }

        bail!("{} API returned {status}{location}: {body}", self.service)
    }
}

/// Follows redirects only within the origin the request went to, so neither the bearer token
/// nor a request body (the migrate call carries the GitHub token) can be replayed to another host.
fn same_origin_redirects() -> redirect::Policy {
    redirect::Policy::custom(|attempt| {
        let previous = attempt.previous();

        if previous.len() > MAX_REDIRECTS {
            return attempt.error("too many redirects");
        }

        match previous.last() {
            Some(from) if from.origin() == attempt.url().origin() => attempt.follow(),
            _ => attempt.stop(),
        }
    })
}
