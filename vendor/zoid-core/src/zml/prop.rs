//! ZML property value types and prop key-value pairs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::zml::error::{ZmlError, ZmlErrorKind, ZmlResult};

/// A typed value for a ZML component prop.
///
/// Uses adjacent TOML tagging: `{ type = "String", value = "hello" }`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum ZmlValue {
    /// A plain string literal.
    String(String),
    /// A numeric value (integer or floating-point).
    Number(f64),
    /// A boolean flag.
    Bool(bool),
    /// A reference to a named data binding, e.g. `$counter`.
    BindingRef(String),
    /// A reference to a design-token path, e.g. `$color.accent`.
    TokenRef(String),
    /// An ordered list of values. Used for collection props such as the
    /// per-component tooltip set (`_tooltips`).
    List(Vec<ZmlValue>),
    /// A string-keyed map of values. Used for structured records such as a
    /// single tooltip entry (`target`/`label`/`icon`/`enabled`). Keys are
    /// sorted for deterministic serialization.
    Map(BTreeMap<String, ZmlValue>),
}

/// A single named property on a ZML component node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ZmlProp {
    /// Property name. Must be non-empty.
    pub name: String,
    /// Property value. Flattened so `type`/`value` sit alongside `name`.
    #[serde(flatten)]
    pub value: ZmlValue,
}

impl ZmlProp {
    /// Creates a new `ZmlProp`, returning an error if `name` is empty.
    pub fn new(name: impl Into<String>, value: ZmlValue) -> ZmlResult<Self> {
        let name: String = name.into().trim().to_owned();
        if name.is_empty() {
            return Err(ZmlError::new(
                ZmlErrorKind::EmptyName,
                "prop name cannot be empty",
            ));
        }
        Ok(Self { name, value })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn roundtrip(prop: &ZmlProp) -> ZmlProp {
        let s = toml::to_string(prop).expect("serialize");
        toml::from_str(&s).expect("deserialize")
    }

    #[test]
    fn string_value_roundtrips() {
        let prop = ZmlProp::new("label", ZmlValue::String("Hello".to_owned())).expect("valid");
        assert_eq!(roundtrip(&prop), prop);
    }

    #[test]
    fn number_value_roundtrips() {
        let prop = ZmlProp::new("size", ZmlValue::Number(16.0)).expect("valid");
        assert_eq!(roundtrip(&prop), prop);
    }

    #[test]
    fn bool_value_roundtrips() {
        let prop = ZmlProp::new("disabled", ZmlValue::Bool(false)).expect("valid");
        assert_eq!(roundtrip(&prop), prop);
    }

    #[test]
    fn binding_ref_roundtrips() {
        let prop = ZmlProp::new("text", ZmlValue::BindingRef("counter".to_owned())).expect("valid");
        assert_eq!(roundtrip(&prop), prop);
    }

    #[test]
    fn token_ref_roundtrips() {
        let prop =
            ZmlProp::new("color", ZmlValue::TokenRef("color.accent".to_owned())).expect("valid");
        assert_eq!(roundtrip(&prop), prop);
    }

    #[test]
    fn list_value_roundtrips() {
        let prop = ZmlProp::new(
            "items",
            ZmlValue::List(vec![
                ZmlValue::String("a".to_owned()),
                ZmlValue::Number(2.0),
            ]),
        )
        .expect("valid");
        assert_eq!(roundtrip(&prop), prop);
    }

    #[test]
    fn map_value_roundtrips() {
        let mut entry = BTreeMap::new();
        entry.insert("target".to_owned(), ZmlValue::String("Save".to_owned()));
        entry.insert("label".to_owned(), ZmlValue::String("Save file".to_owned()));
        entry.insert("enabled".to_owned(), ZmlValue::Bool(true));
        let prop =
            ZmlProp::new("_tooltips", ZmlValue::List(vec![ZmlValue::Map(entry)])).expect("valid");
        assert_eq!(roundtrip(&prop), prop);
    }

    #[test]
    fn empty_name_is_rejected() {
        let err = ZmlProp::new("", ZmlValue::Bool(true)).expect_err("empty name should fail");
        assert_eq!(err.kind(), ZmlErrorKind::EmptyName);
    }

    #[test]
    fn whitespace_only_name_is_rejected() {
        let err = ZmlProp::new("  ", ZmlValue::Bool(true)).expect_err("blank name should fail");
        assert_eq!(err.kind(), ZmlErrorKind::EmptyName);
    }
}
