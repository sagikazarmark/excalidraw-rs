use crate::Error;
use serde::Serialize;
use serde_json::Value;

/// Validated absolute HTTPS report destination. Construct with [`Self::new`].
#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct ReportUrl(String);

impl ReportUrl {
    /// Accept at most 2048 UTF-8 bytes, with explicit `https://` authority and no
    /// credentials, whitespace, controls, backslashes or malformed percent escapes.
    /// Store the URL parser's canonical form (including host/port normalization).
    pub fn new(value: &str) -> Result<Self, Error> {
        if value.len() > 2048
            || !value
                .get(..8)
                .is_some_and(|s| s.eq_ignore_ascii_case("https://"))
            || value
                .chars()
                .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
        {
            return Err(Error::Invalid("invalid report URL"));
        }
        let authority = value[8..].split(['/', '?', '#']).next().unwrap_or_default();
        if authority.is_empty() || authority.contains('@') {
            return Err(Error::Invalid(
                "report URL requires a host without credentials",
            ));
        }
        for (i, byte) in value.bytes().enumerate() {
            if byte == b'%'
                && !value
                    .as_bytes()
                    .get(i + 1..i + 3)
                    .is_some_and(|pair| pair.iter().all(u8::is_ascii_hexdigit))
            {
                return Err(Error::Invalid("invalid report URL percent escape"));
            }
        }
        let parsed = url::Url::parse(value).map_err(|_| Error::Invalid("invalid report URL"))?;
        if parsed.scheme() != "https" || parsed.host_str().is_none() || parsed.as_str().len() > 2048
        {
            return Err(Error::Invalid("report URL must be absolute HTTPS"));
        }
        Ok(Self(parsed.into()))
    }
}

/// Opt-in provenance stored under native `customData.excaliplot`. Source keys
/// describe sources, not element instances, and intentionally survive copying.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    schema_version: u8,
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_key: Option<String>,
    attributes: Value,
}

impl Provenance {
    /// `role` is 1–128 UTF-8 bytes; an optional source key is 1–256 bytes.
    /// Both must be nonblank and control-free. `attributes` must be a JSON object
    /// with at most 16 nested containers (root included). The complete compact
    /// `customData` envelope, including escaped strings, must fit in 4096 bytes.
    /// Caller attributes are literal provenance, never data for regeneration.
    pub fn new(role: &str, source_key: Option<&str>, attributes: Value) -> Result<Self, Error> {
        let valid_label = |s: &str, max| {
            !s.trim().is_empty() && s.len() <= max && !s.chars().any(char::is_control)
        };
        if !valid_label(role, 128) || source_key.is_some_and(|s| !valid_label(s, 256)) {
            return Err(Error::Invalid("invalid provenance role or source key"));
        }
        if !attributes.is_object() || !bounded_depth(&attributes, 0) {
            return Err(Error::Invalid(
                "provenance attributes must be an object of depth at most 16",
            ));
        }
        let provenance = Self {
            schema_version: 1,
            role: role.into(),
            source_key: source_key.map(str::to_owned),
            attributes,
        };
        let envelope = std::collections::BTreeMap::from([("excaliplot", &provenance)]);
        serde_json::to_writer(SizeLimit(4096), &envelope)
            .map_err(|_| Error::Invalid("provenance customData exceeds 4096 bytes"))?;
        Ok(provenance)
    }
}

fn bounded_depth(value: &Value, depth: usize) -> bool {
    match value {
        Value::Array(values) => depth < 16 && values.iter().all(|v| bounded_depth(v, depth + 1)),
        Value::Object(values) => depth < 16 && values.values().all(|v| bounded_depth(v, depth + 1)),
        _ => true,
    }
}

// Count compact serialized bytes without allocating an unbounded intermediate.
struct SizeLimit(usize);
impl std::io::Write for SizeLimit {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self
            .0
            .checked_sub(bytes.len())
            .ok_or_else(|| std::io::Error::other("metadata size limit"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Native report link, optionally accompanied by structured provenance.
#[derive(Clone, Debug)]
pub struct SourceLink {
    pub(crate) url: ReportUrl,
    pub(crate) provenance: Option<Provenance>,
}

impl SourceLink {
    pub fn new(url: ReportUrl) -> Self {
        Self {
            url,
            provenance: None,
        }
    }

    pub fn with_provenance(mut self, provenance: Provenance) -> Self {
        self.provenance = Some(provenance);
        self
    }
}

#[derive(Serialize)]
pub(crate) struct CustomData {
    pub(crate) excaliplot: Provenance,
}
