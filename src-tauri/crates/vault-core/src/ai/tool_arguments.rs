//! Responsibilities: normalize untrusted tool arguments against the advertised schema.
//! Not responsible for: tool execution or replacing the tool's serde validation.
//! Why: repair unambiguous schema names and simple scalar representations before replay.

use anyhow::{Result, bail};
use serde_json::{Map, Value};

/// Returns an object suitable for both dispatch and assistant replay.
/// Unknown fields remain intact so each tool's deny_unknown_fields error lists allowed names.
pub(super) fn normalize(raw: &str, schema: &Value) -> Result<Value> {
    let result = (|| {
        let mut value: Value =
            parse_unique(raw).map_err(|error| anyhow::anyhow!("at $: {error}"))?;
        if let Value::String(encoded) = value {
            value = parse_unique(&encoded).map_err(|error| anyhow::anyhow!("at $: {error}"))?;
        }
        if !value.is_object() {
            bail!("at $: tool arguments must be a JSON object");
        }
        normalize_value(&mut value, schema, "$")?;
        Ok(value)
    })();
    result.map_err(|error: anyhow::Error| anyhow::anyhow!("{error}; received arguments: {raw}"))
}

/// Keeps the path for scalar representation errors that a whole-struct serde decode loses.
pub(super) fn validate_optional_field<T: serde::de::DeserializeOwned>(
    args: &Value,
    field: &str,
) -> Result<()> {
    if let Some(value) = args.get(field) {
        serde_json::from_value::<Option<T>>(value.clone())
            .map_err(|error| anyhow::anyhow!("at $[{field:?}]: {error}"))?;
    }
    Ok(())
}

fn normalize_value(value: &mut Value, schema: &Value, path: &str) -> Result<()> {
    let types: Vec<&str> = match &schema["type"] {
        Value::String(kind) => vec![kind.as_str()],
        Value::Array(kinds) => kinds.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    if !types.is_empty() && !types.iter().any(|kind| matches_type(value, kind)) {
        let mut candidates = types.iter().filter_map(|kind| coerce(value, kind));
        let repaired = candidates.next();
        if repaired.is_none() || candidates.next().is_some() {
            bail!("at {path}: expected one of {types:?}, without losing information or guessing");
        }
        *value = repaired.expect("one unambiguous conversion");
    }
    if let Some(allowed) = schema["enum"].as_array()
        && !allowed.contains(value)
    {
        bail!("at {path}: expected one of {allowed:?}");
    }
    match value {
        Value::Object(object) => {
            let Some(properties) = schema["properties"].as_object() else {
                return Ok(());
            };
            let mut normalized = Map::new();
            for (key, mut item) in std::mem::take(object) {
                let candidates: Vec<&String> =
                    properties.keys().filter(|name| name.trim() == key.trim()).collect();
                if candidates.len() > 1 {
                    bail!("at {path}: ambiguous property {key:?}");
                }
                let key = candidates.first().map(|name| (*name).clone()).unwrap_or(key);
                if normalized.contains_key(&key) {
                    bail!("at {path}: duplicate property after trimming {key:?}");
                }
                if let Some(property) = properties.get(&key) {
                    normalize_value(&mut item, property, &format!("{path}[{key:?}]"))?;
                }
                normalized.insert(key, item);
            }
            *object = normalized;
            if let Some(required) = schema["required"].as_array() {
                for key in required.iter().filter_map(Value::as_str) {
                    if !object.contains_key(key) {
                        bail!("at {path}[{key:?}]: required property is missing");
                    }
                }
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                normalize_value(item, &schema["items"], &format!("{path}[{index}]"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn matches_type(value: &Value, kind: &str) -> bool {
    match kind {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "null" => value.is_null(),
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        _ => false,
    }
}

fn coerce(value: &Value, kind: &str) -> Option<Value> {
    match (value, kind) {
        (Value::Number(_) | Value::Bool(_), "string") => Some(Value::String(value.to_string())),
        (Value::String(text), "integer") => text
            .parse::<i64>()
            .map(Value::from)
            .ok()
            .or_else(|| text.parse::<u64>().map(Value::from).ok()),
        (Value::String(text), "number") => {
            let number = text.parse::<f64>().ok().filter(|number| number.is_finite())?;
            let json = serde_json::Number::from_f64(number)?;
            (json.to_string() == *text).then_some(Value::Number(json))
        }
        (Value::String(text), "boolean") => match text.as_str() {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        },
        (Value::Number(number), "integer") => {
            let number = number.as_f64()?;
            // Every integer in this range is exactly representable by JSON's f64 fallback.
            (number.fract() == 0.0 && number.abs() <= 9_007_199_254_740_992.0)
                .then(|| Value::from(number as i64))
        }
        _ => None,
    }
}

// One deserializer owns the entire walk, so serde_json's recursion limit still applies.
fn parse_unique(raw: &str) -> Result<Value, serde_json::Error> {
    use serde::de::DeserializeSeed;
    let mut deserializer = serde_json::Deserializer::from_str(raw);
    let value = UniqueValue { path: "$".into() }.deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(value)
}

struct UniqueValue {
    path: String,
}

impl<'de> serde::de::DeserializeSeed<'de> for UniqueValue {
    type Value = Value;

    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> serde::de::Visitor<'de> for UniqueValue {
    type Value = Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("JSON with unique object fields")
    }
    fn visit_map<M: serde::de::MapAccess<'de>>(self, mut map: M) -> Result<Value, M::Error> {
        let mut object = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            let path = format!("{}[{key:?}]", self.path);
            if object.contains_key(&key) {
                return Err(serde::de::Error::custom(format!("at {path}: duplicate field")));
            }
            let value = map.next_value_seed(UniqueValue { path })?;
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }
    fn visit_seq<S: serde::de::SeqAccess<'de>>(self, mut seq: S) -> Result<Value, S::Error> {
        let mut items = Vec::new();
        while let Some(item) =
            seq.next_element_seed(UniqueValue { path: format!("{}[{}]", self.path, items.len()) })?
        {
            items.push(item);
        }
        Ok(Value::Array(items))
    }
    fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
}

#[cfg(test)]
mod tests {
    //! Failure modes: malformed JSON or non-object roots reach dispatch; encoded JSON is
    //! unwrapped repeatedly; duplicate JSON fields disappear; trimmed names overwrite another key or choose an ambiguous
    //! property; unknown names disappear; nested paths are lost; primitive coercion rounds,
    //! overflows or truncates; nullable primitives reject unambiguous repairs; serde recursion protection is bypassed; missing required fields and enum violations are accepted.
    //! Typed integer bounds must report the field instead of only the object.
    //! Errors must include the original arguments and the failing JSON path.
    use super::*;
    use serde_json::json;

    fn schema() -> Value {
        json!({"type":"object","properties":{
            "count":{"type":"integer"},"label":{"type":"string"},
            "ready":{"type":"boolean"},"ratio":{"type":"number"},
            "nested":{"type":"array","items":{"type":"object","required":["kind"],
                "properties":{"kind":{"type":"string","enum":["url","domain"]}}}}
        }})
    }
    #[test]
    fn typed_integer_overflow_reports_the_argument_path() {
        let error = validate_optional_field::<u32>(&json!({"limit":4294967296_u64}), "limit")
            .unwrap_err()
            .to_string();
        assert!(error.contains("$[\"limit\"]"), "{error}");
        assert!(validate_optional_field::<u32>(&json!({"limit":u32::MAX}), "limit").is_ok());
    }
    #[test]
    fn repairs_only_lossless_values_and_known_trimmed_names() {
        assert_eq!(
            normalize(r#"{" count ":"5","label":5,"ready":"true","ratio":"0.5"}"#, &schema())
                .unwrap(),
            json!({"count":5,"label":"5","ready":true,"ratio":0.5})
        );
        assert_eq!(normalize(r#"{"count":"-0"}"#, &schema()).unwrap(), json!({"count":0}));
        let encoded = serde_json::to_string(r#"{"count":"5"}"#).unwrap();
        assert_eq!(normalize(&encoded, &schema()).unwrap(), json!({"count":5}));
        assert_eq!(normalize(r#"{"wrong ":5}"#, &schema()).unwrap(), json!({"wrong ":5}));
    }
    #[test]
    fn refuses_duplicate_json_fields_including_encoded_and_nested_objects() {
        for raw in [r#"{"count":1,"count":2}"#, r#"{"nested":[{"kind":"url","kind":"domain"}]}"#] {
            assert!(normalize(raw, &schema()).is_err());
            assert!(normalize(&serde_json::to_string(raw).unwrap(), &schema()).is_err());
        }
    }
    #[test]
    fn repairs_an_unambiguous_nullable_primitive() {
        let schema = json!({"type":"object","properties":{"cursor":{"type":["string","null"]}}});
        assert_eq!(normalize(r#"{"cursor":5}"#, &schema).unwrap(), json!({"cursor":"5"}));
    }
    #[test]
    fn rejects_excessive_nesting_without_recursing_indefinitely() {
        let raw = format!("{{\"nested\":{}0{}}}", "[".repeat(150), "]".repeat(150));
        assert!(normalize(&raw, &schema()).is_err());
    }
    #[test]
    fn rejects_non_objects_and_more_than_one_encoded_layer() {
        for raw in ["null", "[]", "5", "{", r#""not json""#] {
            let error = normalize(raw, &schema()).unwrap_err().to_string();
            assert!(error.contains('$') && error.contains(raw), "{error}");
        }
        let twice = serde_json::to_string(&serde_json::to_string("{}").unwrap()).unwrap();
        assert!(normalize(&twice, &schema()).is_err());
    }
    #[test]
    fn refuses_collisions_and_ambiguous_property_names() {
        assert!(normalize(r#"{"count":1," count ":2}"#, &schema()).is_err());
        let ambiguous = json!({"type":"object","properties":{"count":{}," count ":{}}});
        assert!(normalize(r#"{" count":1}"#, &ambiguous).is_err());
    }
    #[test]
    fn refuses_rounding_overflow_truncation_and_unrelated_types() {
        for raw in [
            r#"{"count":"5.1"}"#,
            r#"{"count":5.1}"#,
            r#"{"count":true}"#,
            r#"{"count":"18446744073709551616"}"#,
            r#"{"ratio":"0.10000000000000000001"}"#,
            r#"{"count":"5.0"}"#,
            r#"{"ratio":"1e2"}"#,
            r#"{"label":{}}"#,
            r#"{"ready":"yes"}"#,
        ] {
            assert!(normalize(raw, &schema()).is_err(), "{raw}");
        }
    }
    #[test]
    fn validates_nested_required_fields_and_enums_with_paths() {
        for (raw, path) in [
            (r#"{"nested":[{}]}"#, "$[\"nested\"][0][\"kind\"]"),
            (r#"{"nested":[{"kind":"bad"}]}"#, "$[\"nested\"][0][\"kind\"]"),
        ] {
            let error = normalize(raw, &schema()).unwrap_err().to_string();
            assert!(error.contains(path) && error.contains(raw), "{error}");
        }
    }
}
