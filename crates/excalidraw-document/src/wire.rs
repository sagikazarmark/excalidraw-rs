use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, MapAccess, Visitor},
};
use serde_json::{Map, Value, value::RawValue};
use std::{fmt, marker::PhantomData};

pub type Object = Map<String, Value>;

/// A JSON-path-addressed failure. Input documents are never modified on failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub path: String,
    pub message: String,
}
impl Error {
    pub(crate) fn at(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}
impl std::error::Error for Error {}
impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::at("", error.to_string())
    }
}

/// Missing, explicit null and a typed value remain distinct.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Field<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}

/// Exact JSON number; conversion into editor geometry is explicit and checked.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Number(pub serde_json::Number);
impl Number {
    pub(crate) fn check_dimension(&self, path: &str) -> Result<(), Error> {
        self.as_f64().map_err(|e| Error::at(path, e.message))?;
        if self.is_negative_nonzero() {
            return Err(Error::at(path, "dimension must be nonnegative"));
        }
        Ok(())
    }
    /// Whether the stored decimal is negative and not exactly zero.
    ///
    /// `as_f64` underflows a tiny negative magnitude such as `-1e-400` to
    /// `-0.0`, which is neither less than a zero floor nor distinguishable
    /// from `0.0` by comparison, so a range check that consults f64 alone
    /// admits it. The stored decimal still carries the sign, so ask that.
    /// Exact `-0` is permitted: it is zero, spelled with a sign.
    ///
    /// Both the authoring path ([`Self::check_dimension`]) and the validating
    /// range table judge nonnegative fields through this, so the two cannot
    /// disagree about which numbers are admissible.
    pub(crate) fn is_negative_nonzero(&self) -> bool {
        self.as_f64().is_ok_and(f64::is_sign_negative) && self.as_safe_integer() != Ok(0)
    }
    pub fn from_f64(value: f64) -> Result<Self, Error> {
        serde_json::Number::from_f64(value)
            .map(Self)
            .ok_or_else(|| Error::at("", "number must be finite"))
    }
    pub fn as_f64(&self) -> Result<f64, Error> {
        self.0
            .as_f64()
            .filter(|v| v.is_finite())
            .ok_or_else(|| Error::at("", "number is outside finite f64 range"))
    }
    pub fn as_safe_integer(&self) -> Result<i64, Error> {
        let Some((negative, digits, exponent)) = crate::projection::decimal(&self.0.to_string())
        else {
            return Err(Error::at("", "expected JavaScript safe integer"));
        };
        if !(0..=16).contains(&exponent) || digits.len() + exponent as usize > 16 {
            return Err(Error::at("", "expected JavaScript safe integer"));
        }
        let magnitude: i64 = format!("{digits}{}", "0".repeat(exponent as usize))
            .parse()
            .map_err(|_| Error::at("", "expected JavaScript safe integer"))?;
        if magnitude > 9_007_199_254_740_991 {
            return Err(Error::at("", "expected JavaScript safe integer"));
        }
        Ok(if negative { -magnitude } else { magnitude })
    }
}
impl std::str::FromStr for Number {
    type Err = Error;
    fn from_str(value: &str) -> Result<Self, Error> {
        Self::read_wire(&parse(value.as_bytes())?)
    }
}
impl From<i64> for Number {
    fn from(value: i64) -> Self {
        Self(value.into())
    }
}
impl From<u64> for Number {
    fn from(value: u64) -> Self {
        Self(value.into())
    }
}

/// A field descriptor scoped to its owning record. Prefer the constants in
/// [`crate::element`], [`crate::app_state`] and the other model modules.
pub struct Key<O, T> {
    pub name: &'static str,
    marker: PhantomData<fn() -> (O, T)>,
}
impl<O, T> Copy for Key<O, T> {}
impl<O, T> Clone for Key<O, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<O, T> Key<O, T> {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            marker: PhantomData,
        }
    }
}

/// One authoritative JSON object with typed views. Unknown keys and malformed
/// known values survive inspection and edits to other fields.
#[derive(Clone, Debug, PartialEq)]
pub struct Record<O> {
    values: Object,
    marker: PhantomData<O>,
}
impl<O> Default for Record<O> {
    fn default() -> Self {
        Self::from_object(Object::new())
    }
}
impl<O> Record<O> {
    pub fn from_object(values: Object) -> Self {
        Self {
            values,
            marker: PhantomData,
        }
    }
    pub fn as_object(&self) -> &Object {
        &self.values
    }
    pub fn into_object(self) -> Object {
        self.values
    }
    pub fn get<T: WireValue>(&self, key: Key<O, T>) -> Result<Field<T>, Error> {
        match self.values.get(key.name) {
            None => Ok(Field::Missing),
            Some(Value::Null) => Ok(Field::Null),
            Some(value) => T::read_wire(value)
                .map(Field::Value)
                .map_err(|e| Error::at(format!("{}{}", pointer(key.name), e.path), e.message)),
        }
    }
    pub fn set<T: WireValue>(&mut self, key: Key<O, T>, value: T) -> Result<(), Error> {
        let value = value.write_wire();
        self.values.insert(key.name.into(), value);
        Ok(())
    }
    pub fn set_null<T>(&mut self, key: Key<O, T>) {
        self.values.insert(key.name.into(), Value::Null);
    }
    pub fn remove<T>(&mut self, key: Key<O, T>) {
        self.values.remove(key.name);
    }
    /// Explicit untyped edit, also usable for future fields. No duplicate keys are emitted.
    pub fn set_raw(&mut self, name: impl Into<String>, value: Value) {
        self.values.insert(name.into(), value);
    }
}

/// Typed conversion directly from the authoritative JSON tree. Unlike generic
/// Serde buffering, object metadata cannot be mistaken for its number protocol.
pub trait WireValue: Sized {
    fn read_wire(value: &Value) -> Result<Self, Error>;
    fn write_wire(self) -> Value;
}
impl WireValue for Number {
    fn read_wire(value: &Value) -> Result<Self, Error> {
        match value {
            Value::Number(n) => Ok(Self(n.clone())),
            _ => Err(Error::at("", "expected number")),
        }
    }
    fn write_wire(self) -> Value {
        Value::Number(self.0)
    }
}
impl WireValue for String {
    fn read_wire(value: &Value) -> Result<Self, Error> {
        value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| Error::at("", "expected string"))
    }
    fn write_wire(self) -> Value {
        Value::String(self)
    }
}
impl WireValue for bool {
    fn read_wire(value: &Value) -> Result<Self, Error> {
        value
            .as_bool()
            .ok_or_else(|| Error::at("", "expected boolean"))
    }
    fn write_wire(self) -> Value {
        Value::Bool(self)
    }
}
impl WireValue for Object {
    fn read_wire(value: &Value) -> Result<Self, Error> {
        value
            .as_object()
            .cloned()
            .ok_or_else(|| Error::at("", "expected object"))
    }
    fn write_wire(self) -> Value {
        Value::Object(self)
    }
}
impl<O> WireValue for Record<O> {
    fn read_wire(value: &Value) -> Result<Self, Error> {
        Object::read_wire(value).map(Self::from_object)
    }
    fn write_wire(self) -> Value {
        Value::Object(self.values)
    }
}
impl<T: WireValue> WireValue for Vec<T> {
    fn read_wire(value: &Value) -> Result<Self, Error> {
        value
            .as_array()
            .ok_or_else(|| Error::at("", "expected array"))?
            .iter()
            .enumerate()
            .map(|(i, v)| {
                T::read_wire(v).map_err(|e| Error::at(format!("/{i}{}", e.path), e.message))
            })
            .collect()
    }
    fn write_wire(self) -> Value {
        Value::Array(self.into_iter().map(T::write_wire).collect())
    }
}
impl WireValue for [Number; 2] {
    fn read_wire(value: &Value) -> Result<Self, Error> {
        Vec::<Number>::read_wire(value)?
            .try_into()
            .map_err(|_| Error::at("", "expected numeric pair"))
    }
    fn write_wire(self) -> Value {
        Value::Array(self.into_iter().map(WireValue::write_wire).collect())
    }
}
impl WireValue for std::collections::BTreeMap<crate::GroupId, bool> {
    fn read_wire(value: &Value) -> Result<Self, Error> {
        Object::read_wire(value)?
            .into_iter()
            .map(|(k, v)| bool::read_wire(&v).map(|v| (crate::GroupId(k), v)))
            .collect()
    }
    fn write_wire(self) -> Value {
        Value::Object(
            self.into_iter()
                .map(|(k, v)| (k.0, Value::Bool(v)))
                .collect(),
        )
    }
}
impl<O> Serialize for Record<O> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.values.serialize(serializer)
    }
}

pub(crate) fn pointer(key: &str) -> String {
    format!("/{}", key.replace('~', "~0").replace('/', "~1"))
}

// Parse object values as RawValue before dispatch. This distinguishes an actual
// "$serde_json::private::Number" user key from serde_json's number protocol and
// checks duplicates without first collapsing the object into a map.
struct Members(Vec<(String, Box<RawValue>)>);
impl<'de> Deserialize<'de> for Members {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ObjectVisitor;
        impl<'de> Visitor<'de> for ObjectVisitor {
            type Value = Members;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("JSON object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Members, A::Error> {
                let mut entries = Vec::new();
                let mut keys = std::collections::HashSet::new();
                while let Some((key, value)) = map.next_entry::<String, Box<RawValue>>()? {
                    if !keys.insert(key.clone()) {
                        return Err(de::Error::custom(format!("duplicate object key {key:?}")));
                    }
                    entries.push((key, value));
                }
                Ok(Members(entries))
            }
        }
        d.deserialize_map(ObjectVisitor)
    }
}
pub(crate) fn parse(bytes: &[u8]) -> Result<Value, Error> {
    let raw: Box<RawValue> = serde_json::from_slice(bytes)?;
    fn decode(raw: &RawValue, path: &str, depth: usize) -> Result<Value, Error> {
        if depth > 128 {
            return Err(Error::at(path, "JSON nesting exceeds 128"));
        }
        let text = raw.get().trim_start();
        match text.as_bytes()[0] {
            b'{' => {
                let members: Members =
                    serde_json::from_str(text).map_err(|e| Error::at(path, e.to_string()))?;
                let mut object = Object::new();
                for (key, value) in members.0 {
                    object.insert(
                        key.clone(),
                        decode(&value, &format!("{path}{}", pointer(&key)), depth + 1)?,
                    );
                }
                Ok(Value::Object(object))
            }
            b'[' => {
                let entries: Vec<Box<RawValue>> = serde_json::from_str(text)?;
                entries
                    .iter()
                    .enumerate()
                    .map(|(i, v)| decode(v, &format!("{path}/{i}"), depth + 1))
                    .collect::<Result<Vec<_>, _>>()
                    .map(Value::Array)
            }
            _ => serde_json::from_str(text).map_err(|e| Error::at(path, e.to_string())),
        }
    }
    decode(&raw, "", 0)
}
