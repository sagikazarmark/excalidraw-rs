//! The transport seam: a resolved request plus the operation trait.
use crate::{
    Error,
    error::{HeaderLookup, expect_ok, json},
};
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};

/// Everything outside the unreserved set is encoded, so `/`, `?`, `#` and `%`
/// inside an opaque identifier cannot escape their path segment.
const SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'!')
    .add(b'"')
    .add(b'#')
    .add(b'$')
    .add(b'%')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

/// Percent-encode one path segment.
pub(crate) fn segment(value: &str) -> String {
    utf8_percent_encode(value, SEGMENT).to_string()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
    /// True when repeating the request has the same effect as sending it once.
    /// `POST` is excluded: the API publishes no idempotency key.
    ///
    /// This is the method's default; an operation can still refuse replay
    /// through [`Operation::replayable`].
    pub fn is_idempotent(self) -> bool {
        !matches!(self, Self::Post)
    }
}

/// A resolved request, relative to the configured base URL.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    pub method: Method,
    /// Percent-encoded, with a leading slash, never containing the base URL.
    pub path: String,
    /// Emitted in declaration order. An omitted parameter is absent, not empty.
    pub query: Vec<(&'static str, String)>,
    /// `Some` implies `Content-Type: application/json`.
    pub body: Option<Vec<u8>>,
}

impl Request {
    pub(crate) fn new(method: Method, path: impl Into<String>) -> Self {
        Self {
            method,
            path: path.into(),
            query: Vec::new(),
            body: None,
        }
    }
    pub(crate) fn query(mut self, key: &'static str, value: impl std::fmt::Display) -> Self {
        self.query.push((key, value.to_string()));
        self
    }
    pub(crate) fn maybe_query(
        self,
        key: &'static str,
        value: Option<impl std::fmt::Display>,
    ) -> Self {
        match value {
            Some(value) => self.query(key, value),
            None => self,
        }
    }
    pub(crate) fn json(mut self, body: &impl serde::Serialize) -> Result<Self, Error> {
        // Not `Error::decode`: that names a *response* that failed the pinned
        // schema. This is our own body failing to serialize, which is a defect
        // in this crate rather than something the server said.
        self.body = Some(
            serde_json::to_vec(body)
                .map_err(|error| Error::invalid("request body", error.to_string()))?,
        );
        Ok(self)
    }
    pub(crate) fn bytes(mut self, body: Vec<u8>) -> Self {
        self.body = Some(body);
        self
    }

    /// The path and query as one relative URL, for transports that want it.
    pub fn relative_url(&self) -> String {
        if self.query.is_empty() {
            return self.path.clone();
        }
        let mut url = self.path.clone();
        url.push('?');
        for (i, (key, value)) in self.query.iter().enumerate() {
            if i > 0 {
                url.push('&');
            }
            url.push_str(key);
            url.push('=');
            url.push_str(&utf8_percent_encode(value, SEGMENT).to_string());
        }
        url
    }
}

/// One documented API operation: how to build its request, how to read its
/// response. Implementations own status dispatch so that every transport,
/// including a test harness with no network, gets identical error mapping.
pub trait Operation {
    type Output;

    fn request(&self) -> Result<Request, Error>;

    fn decode(
        &self,
        status: u16,
        headers: &dyn HeaderLookup,
        body: &[u8],
    ) -> Result<Self::Output, Error>;

    /// May the opt-in retry replay `request` after a `429` or `5xx`?
    ///
    /// Defaults to [`Method::is_idempotent`]. Override it to `false` when a
    /// repeat has an effect the first attempt did not, even though the method
    /// is idempotent in HTTP terms: a `5xx` is ambiguous, and the first attempt
    /// may already have been applied. [`crate::op::ReplaceSceneContent`] is the
    /// worked example.
    fn replayable(&self, request: &Request) -> bool {
        request.method.is_idempotent()
    }
}

/// An operation whose whole response contract is "expect a 2xx, then parse the
/// body as JSON". Implementing it supplies [`Operation`] through a blanket impl,
/// so that status dispatch is written once instead of once per operation.
///
/// Do **not** implement it when `decode` has a decision to make: a non-JSON or
/// hand-parsed body, a status other than 2xx that carries a usable result, or
/// headers that change how the body is read. Implement [`Operation`] directly in
/// that case — [`crate::op::GetSceneContent`] is the worked example.
pub trait JsonOperation {
    type Output: serde::de::DeserializeOwned;

    fn request(&self) -> Result<Request, Error>;
}

impl<T: JsonOperation> Operation for T {
    type Output = <T as JsonOperation>::Output;

    fn request(&self) -> Result<Request, Error> {
        JsonOperation::request(self)
    }

    fn decode(
        &self,
        status: u16,
        headers: &dyn HeaderLookup,
        body: &[u8],
    ) -> Result<Self::Output, Error> {
        json(expect_ok(status, headers, body)?)
    }
}
