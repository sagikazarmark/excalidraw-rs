//! Optional HTTP clients.
//!
//! Both are thin: build the [`Request`], attach one `Authorization` header, hand
//! the status, headers and body to [`Operation::decode`]. Neither holds
//! per-operation knowledge, so error mapping is identical to a hand-rolled
//! transport's.
#[cfg(feature = "blocking")]
pub mod blocking;
#[cfg(feature = "retry")]
pub mod retry;

use crate::{
    API_BASE_URL, ApiKey, Error, HeaderLookup, Method, Operation, Page, PageRequest, RateLimit,
    Request,
};

/// Transport settings. `Default` uses the documented base URL and no timeout.
#[derive(Clone, Debug, PartialEq)]
pub struct ClientConfig {
    /// Defaults to [`API_BASE_URL`]. A trailing slash is normalised away.
    pub base_url: String,
    /// Appended after `excalidraw-api/<version>` in the `User-Agent`.
    pub user_agent_suffix: Option<String>,
    pub timeout: Option<std::time::Duration>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            base_url: API_BASE_URL.to_owned(),
            user_agent_suffix: None,
            timeout: None,
        }
    }
}

impl ClientConfig {
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }
    pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
    pub fn user_agent_suffix(mut self, suffix: impl Into<String>) -> Self {
        self.user_agent_suffix = Some(suffix.into());
        self
    }
    fn user_agent(&self) -> String {
        let base = concat!("excalidraw-api/", env!("CARGO_PKG_VERSION"));
        match &self.user_agent_suffix {
            Some(suffix) => format!("{base} {suffix}"),
            None => base.to_owned(),
        }
    }
    fn url(&self, request: &Request) -> String {
        format!(
            "{}{}",
            self.base_url.trim_end_matches('/'),
            request.relative_url()
        )
    }
}

/// Header lookup over `reqwest`'s map.
pub(crate) struct ReqwestHeaders<'a>(pub(crate) &'a reqwest::header::HeaderMap);

impl HeaderLookup for ReqwestHeaders<'_> {
    fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).and_then(|value| value.to_str().ok())
    }
}

pub(crate) fn reqwest_method(method: Method) -> reqwest::Method {
    match method {
        Method::Get => reqwest::Method::GET,
        Method::Post => reqwest::Method::POST,
        Method::Put => reqwest::Method::PUT,
        Method::Patch => reqwest::Method::PATCH,
        Method::Delete => reqwest::Method::DELETE,
    }
}

pub(crate) fn transport(error: impl std::error::Error + Send + Sync + 'static) -> Error {
    Error::Transport(Box::new(error))
}

/// Guarantee a rustls cryptography provider exists before a client is built.
///
/// `reqwest` is built with `rustls-no-provider`, which keeps `aws-lc-sys` and its
/// cmake build out of the tree. Without a provider, reqwest panics inside its own
/// runtime thread on the first request, so the `provider-ring` feature installs
/// one here instead.
#[cfg(feature = "provider-ring")]
pub(crate) fn ensure_provider() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        // An error means another crate installed one first, which is equally
        // good: the guarantee is that *a* provider exists, not that it is ours.
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// Without `provider-ring` the caller owns this. See the crate docs.
#[cfg(not(feature = "provider-ring"))]
pub(crate) fn ensure_provider() {}

/// Async client for the documented API.
///
/// The `client` feature installs a `ring` cryptography provider on construction,
/// so this works without setup. `reqwest` is still built with
/// `rustls-no-provider`, which keeps `aws-lc-sys` and its cmake build out of the
/// tree.
///
/// Callers who want to choose their own provider can depend on `client-core`
/// instead and install one before constructing a client. Nothing installs a
/// provider in that configuration, and reqwest panics on the first request
/// without one.
pub struct Client {
    http: reqwest::Client,
    key: ApiKey,
    config: ClientConfig,
    last_rate_limit: std::sync::Mutex<Option<RateLimit>>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.config.base_url)
            .finish_non_exhaustive()
    }
}

impl Client {
    pub fn new(key: ApiKey) -> Result<Self, Error> {
        Self::with_config(key, ClientConfig::default())
    }

    pub fn with_config(key: ApiKey, config: ClientConfig) -> Result<Self, Error> {
        ensure_provider();
        let mut builder = reqwest::Client::builder().user_agent(config.user_agent());
        if let Some(timeout) = config.timeout {
            builder = builder.timeout(timeout);
        }
        Ok(Self {
            http: builder.build().map_err(transport)?,
            key,
            config,
            last_rate_limit: std::sync::Mutex::new(None),
        })
    }

    /// Rate-limit headers from the most recent response that carried them.
    pub fn last_rate_limit(&self) -> Option<RateLimit> {
        self.last_rate_limit.lock().ok().and_then(|seen| *seen)
    }

    pub async fn send<O: Operation>(&self, op: O) -> Result<O::Output, Error> {
        self.send_ref(&op).await
    }

    /// `send` by reference, so a retry loop can resend one operation without
    /// cloning it: a scene body may be several megabytes.
    pub(crate) async fn send_ref<O: Operation>(&self, op: &O) -> Result<O::Output, Error> {
        let mut request = op.request()?;
        let mut http = self
            .http
            .request(reqwest_method(request.method), self.config.url(&request))
            .header("authorization", self.key.header_value());
        // Move the body rather than cloning it: a scene may carry several
        // megabytes of data URLs, and it is already serialised once.
        if let Some(body) = request.body.take() {
            http = http.header("content-type", "application/json").body(body);
        }
        let response = http.send().await.map_err(transport)?;
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let body = response.bytes().await.map_err(transport)?;

        let seen = RateLimit::from_headers(&ReqwestHeaders(&headers));
        if !seen.is_empty()
            && let Ok(mut slot) = self.last_rate_limit.lock()
        {
            *slot = Some(seen);
        }
        op.decode(status, &ReqwestHeaders(&headers), &body)
    }

    /// Walk an offset-paginated operation until it reports no next page, or
    /// `max_items` is reached. A `max_items` of zero returns an empty result
    /// without sending a request.
    ///
    /// Offset pagination over a mutating collection can repeat or skip items:
    /// consecutive pages are not a snapshot. `max_items` is required rather than
    /// defaulted, so a runaway walk is always a caller decision.
    pub async fn collect<O, T, F>(
        &self,
        mut make: F,
        start: PageRequest,
        max_items: usize,
    ) -> Result<Vec<T>, Error>
    where
        O: Operation<Output = Page<T>>,
        F: FnMut(PageRequest) -> O,
    {
        let mut collected = Vec::new();
        let mut next = crate::page::first(start, max_items);
        while let Some(request) = next {
            let page = self.send(make(request)).await?;
            next = crate::page::absorb(page, &mut collected, max_items);
        }
        Ok(collected)
    }
}
