//! Status dispatch and the documented error envelope.
use serde::Deserialize;
use std::fmt;

/// Parsed from `X-RateLimit-*`. Documented in the API reference prose; these
/// headers are not declared by any operation in the published OpenAPI artifact.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RateLimit {
    pub limit: Option<u64>,
    pub remaining: Option<u64>,
    /// Unix seconds at which the window resets. Not a duration.
    pub reset: Option<u64>,
}

impl RateLimit {
    pub fn from_headers(headers: &dyn HeaderLookup) -> Self {
        let get = |name: &str| headers.get(name).and_then(|v| v.trim().parse::<u64>().ok());
        Self {
            limit: get("x-ratelimit-limit"),
            remaining: get("x-ratelimit-remaining"),
            reset: get("x-ratelimit-reset"),
        }
    }
    /// True when every header was absent or unparsable.
    pub fn is_empty(&self) -> bool {
        self.limit.is_none() && self.remaining.is_none() && self.reset.is_none()
    }
}

/// Header access without an `http` crate dependency in the default build.
/// Name matching is ASCII-case-insensitive; implementations receive lowercase.
pub trait HeaderLookup {
    fn get(&self, name: &str) -> Option<&str>;
}

/// No headers at all. Useful when decoding a stored fixture.
pub struct NoHeaders;
impl HeaderLookup for NoHeaders {
    fn get(&self, _name: &str) -> Option<&str> {
        None
    }
}

/// Borrowed header pairs, matched case-insensitively.
pub struct SliceHeaders<'a>(pub &'a [(&'a str, &'a str)]);
impl HeaderLookup for SliceHeaders<'_> {
    fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| *v)
    }
}

impl HeaderLookup for Vec<(String, String)> {
    fn get(&self, name: &str) -> Option<&str> {
        self.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// The documented error envelope. Decoding only.
#[derive(Debug, Deserialize)]
struct ErrorBody {
    #[allow(dead_code)]
    #[serde(rename = "statusCode")]
    status_code: Option<u16>,
    error: String,
    message: String,
}

/// Every failure this crate can report.
#[derive(Debug)]
pub enum Error {
    /// 400/401/403/404 carrying the documented `{statusCode, error, message}`.
    Api {
        status: u16,
        kind: String,
        message: String,
    },
    /// 429. Documented in prose only.
    RateLimited(RateLimit),
    /// Any other status, and any status whose body did not match the envelope.
    Unexpected { status: u16, body: Vec<u8> },
    /// A 200 body that did not match the pinned schema.
    ///
    /// Carries no path: `serde_json`'s own message already names the offending
    /// location, and the field that used to sit here was never populated.
    Decode { message: String },
    /// A scene payload rejected by `excalidraw_document::plus`, before sending
    /// or on decode. Retains the lower crate's JSON pointer.
    Content(excalidraw_document::Error),
    /// Caller-side misuse caught before any request is built.
    Invalid { what: &'static str, detail: String },
    /// Transport failure. Only constructible with a client feature enabled.
    #[cfg(feature = "client-core")]
    Transport(Box<dyn std::error::Error + Send + Sync>),
}

impl Error {
    pub(crate) fn invalid(what: &'static str, detail: impl Into<String>) -> Self {
        Self::Invalid {
            what,
            detail: detail.into(),
        }
    }
    pub(crate) fn decode(error: serde_json::Error) -> Self {
        Self::Decode {
            message: error.to_string(),
        }
    }
    /// The HTTP status this error came from, when it came from one.
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Api { status, .. } | Self::Unexpected { status, .. } => Some(*status),
            Self::RateLimited(_) => Some(429),
            _ => None,
        }
    }
    /// True for statuses where a later identical request may succeed.
    ///
    /// Server errors qualify however their body was shaped: a `503` carrying the
    /// documented envelope is as retryable as one that does not.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::RateLimited(_) => true,
            Self::Api { status, .. } | Self::Unexpected { status, .. } => *status >= 500,
            _ => false,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Api {
                status,
                kind,
                message,
            } => write!(f, "{status} {kind}: {message}"),
            Self::RateLimited(limit) => match limit.reset {
                Some(reset) => write!(f, "429 Too Many Requests: rate limit resets at {reset}"),
                None => write!(f, "429 Too Many Requests"),
            },
            Self::Unexpected { status, body } => {
                write!(f, "unexpected HTTP {status}")?;
                if !body.is_empty() {
                    let text = String::from_utf8_lossy(body);
                    let text: String = text.chars().take(200).collect();
                    write!(f, ": {text}")?;
                }
                Ok(())
            }
            Self::Decode { message } => {
                write!(f, "response did not match the pinned schema: {message}")
            }
            Self::Content(error) => write!(f, "scene content rejected: {error}"),
            Self::Invalid { what, detail } => write!(f, "{what}: {detail}"),
            #[cfg(feature = "client-core")]
            Self::Transport(error) => write!(f, "transport failure: {error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Content(error) => Some(error),
            #[cfg(feature = "client-core")]
            Self::Transport(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

impl From<excalidraw_document::Error> for Error {
    fn from(error: excalidraw_document::Error) -> Self {
        Self::Content(error)
    }
}

/// Shared status dispatch for every operation. Returns the body only for 200.
pub(crate) fn expect_ok<'b>(
    status: u16,
    headers: &dyn HeaderLookup,
    body: &'b [u8],
) -> Result<&'b [u8], Error> {
    match status {
        200 => Ok(body),
        429 => Err(Error::RateLimited(RateLimit::from_headers(headers))),
        // Any status carrying the documented envelope becomes a typed error, not
        // only the four the artifact declares per operation. Statuses reachable
        // in practice but absent from the artifact include 413 (a request body
        // limit, observed as FST_ERR_CTP_BODY_TOO_LARGE) and 503.
        _ => match serde_json::from_slice::<ErrorBody>(body) {
            Ok(parsed) => Err(Error::Api {
                status,
                kind: parsed.error,
                message: parsed.message,
            }),
            // A body that is not the documented envelope is never given a
            // fabricated message.
            Err(_) => Err(Error::Unexpected {
                status,
                body: body.to_vec(),
            }),
        },
    }
}

/// Decode a 200 body as a documented JSON shape.
pub(crate) fn json<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(body).map_err(Error::decode)
}
