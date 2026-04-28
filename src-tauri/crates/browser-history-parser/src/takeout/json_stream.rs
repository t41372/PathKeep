//! JSON record streaming helpers for Takeout payloads.
//!
//! ## Responsibilities
//! - Walk top-level arrays or named object arrays without materializing the
//!   whole payload as one `Value`.
//! - Hand each raw record to the Takeout payload parsers in stable ordinal
//!   order.
//! - Keep Takeout JSON-shape quirks localized away from the higher-level
//!   payload parsers.
//!
//! ## Not responsible for
//! - Interpreting records as canonical visits, URLs, or native entities.
//! - Building schema observations or capability snapshots.

use crate::ParseError;
use serde::de::{self, DeserializeSeed, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use std::fmt;

/// Wraps JSON parse failures and callback failures during record streaming.
#[derive(Debug, thiserror::Error)]
pub(super) enum JsonRecordStreamError<E> {
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error(transparent)]
    Callback(E),
}

/// Streams raw records from a Takeout payload into a caller-provided callback.
///
/// The payload may either be a top-level array or an object that stores the
/// relevant rows under one of `array_keys`.
pub(super) fn stream_payload_records<F, E>(
    bytes: &[u8],
    source_path: &str,
    array_keys: &[&str],
    mut on_record: F,
) -> Result<usize, JsonRecordStreamError<E>>
where
    F: FnMut(Value, usize) -> Result<(), E>,
{
    let mut callback_error = None;
    let mut ordinal = 0usize;
    let mut callback = |record: Value| -> Result<(), CallbackAbort> {
        if let Err(error) = on_record(record, ordinal) {
            callback_error = Some(error);
            return Err(CallbackAbort);
        }
        ordinal += 1;
        Ok(())
    };
    let visitor = RootRecordVisitor { array_keys, on_record: &mut callback };
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    if let Err(source) = deserializer.deserialize_any(visitor) {
        if let Some(error) = callback_error {
            return Err(JsonRecordStreamError::Callback(error));
        }
        return Err(JsonRecordStreamError::Parse(ParseError::Json {
            path: source_path.to_string(),
            source,
        }));
    }
    Ok(ordinal)
}

#[derive(Debug, Clone, Copy)]
struct CallbackAbort;

impl fmt::Display for CallbackAbort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("stream callback aborted")
    }
}

struct RootRecordVisitor<'a, F> {
    array_keys: &'a [&'a str],
    on_record: &'a mut F,
}

impl<'de, F> Visitor<'de> for RootRecordVisitor<'_, F>
where
    F: FnMut(Value) -> Result<(), CallbackAbort>,
{
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a Takeout payload array or object")
    }

    fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        RecordArraySeed { on_record: self.on_record }.visit_seq(seq)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        while let Some(key) = map.next_key::<String>()? {
            if self.array_keys.iter().any(|candidate| key == *candidate) {
                map.next_value_seed(RecordArraySeed { on_record: self.on_record })?;
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(())
    }
}

struct RecordArraySeed<'a, F> {
    on_record: &'a mut F,
}

impl<'de, F> DeserializeSeed<'de> for RecordArraySeed<'_, F>
where
    F: FnMut(Value) -> Result<(), CallbackAbort>,
{
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }
}

impl<'de, F> Visitor<'de> for RecordArraySeed<'_, F>
where
    F: FnMut(Value) -> Result<(), CallbackAbort>,
{
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an array of Takeout payload records")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while let Some(record) = seq.next_element::<Value>()? {
            (self.on_record)(record).map_err(de::Error::custom)?;
        }
        Ok(())
    }
}
