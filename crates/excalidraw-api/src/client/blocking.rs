//! Synchronous client.
//!
//! `reqwest`'s blocking client is itself a wrapper that runs its own runtime, so
//! this is a thin surface over the same [`Operation`] implementations rather than
//! a separate transport.
use super::{ClientConfig, ReqwestHeaders, ensure_provider, reqwest_method, transport};
use crate::{ApiKey, Error, Operation, Page, PageRequest, RateLimit};

pub struct Client {
    http: reqwest::blocking::Client,
    key: ApiKey,
    config: ClientConfig,
    last_rate_limit: std::sync::Mutex<Option<RateLimit>>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("blocking::Client")
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
        let mut builder = reqwest::blocking::Client::builder().user_agent(config.user_agent());
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

    pub fn last_rate_limit(&self) -> Option<RateLimit> {
        self.last_rate_limit.lock().ok().and_then(|seen| *seen)
    }

    pub fn send<O: Operation>(&self, op: O) -> Result<O::Output, Error> {
        let mut request = op.request()?;
        let mut http = self
            .http
            .request(reqwest_method(request.method), self.config.url(&request))
            .header("authorization", self.key.header_value());
        // Move the body rather than cloning it; see the async client.
        if let Some(body) = request.body.take() {
            http = http.header("content-type", "application/json").body(body);
        }
        let response = http.send().map_err(transport)?;
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let body = response.bytes().map_err(transport)?;

        let seen = RateLimit::from_headers(&ReqwestHeaders(&headers));
        if !seen.is_empty()
            && let Ok(mut slot) = self.last_rate_limit.lock()
        {
            *slot = Some(seen);
        }
        op.decode(status, &ReqwestHeaders(&headers), &body)
    }

    /// Walk an offset-paginated operation. See [`crate::Client::collect`] for the
    /// snapshot caveat.
    pub fn collect<O, T, F>(
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
        let mut next = Some(start);
        while let Some(request) = next {
            let page = self.send(make(request))?;
            next = crate::page::absorb(page, &mut collected, max_items);
        }
        Ok(collected)
    }
}
