//! API key handling. The key reaches only the `Authorization` header.
use crate::Error;
use std::fmt;

/// A bearer API key. `Debug` and `Display` both redact the secret, and there is
/// no `Deref`, `AsRef<str>`, or `Serialize`: the value leaves this type only
/// through `header_value`, which the client uses for one header.
#[derive(Clone)]
pub struct ApiKey(#[cfg_attr(not(feature = "client-core"), allow(dead_code))] String);

impl ApiKey {
    /// Rejects empty and whitespace-only keys. No format is validated, because
    /// no key format is published.
    pub fn new(key: impl Into<String>) -> Result<Self, Error> {
        let key = key.into();
        if key.trim().is_empty() {
            return Err(Error::invalid("api key", "must not be empty"));
        }
        if key.bytes().any(|b| b == b'\r' || b == b'\n' || b == 0) {
            return Err(Error::invalid(
                "api key",
                "must not contain control characters",
            ));
        }
        Ok(Self(key))
    }

    /// Reads `EXCALIDRAW_API_KEY`.
    pub fn from_env() -> Result<Self, Error> {
        match std::env::var("EXCALIDRAW_API_KEY") {
            Ok(key) => Self::new(key),
            Err(_) => Err(Error::invalid("api key", "EXCALIDRAW_API_KEY is not set")),
        }
    }

    #[cfg_attr(not(feature = "client-core"), allow(dead_code))]
    pub(crate) fn header_value(&self) -> String {
        format!("Bearer {}", self.0)
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey(<redacted>)")
    }
}

impl fmt::Display for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey(<redacted>)")
    }
}
