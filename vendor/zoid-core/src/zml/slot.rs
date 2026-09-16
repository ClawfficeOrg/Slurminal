//! ZML named slot and data binding declarations.

use serde::{Deserialize, Serialize};

use crate::zml::error::{ZmlError, ZmlErrorKind, ZmlResult};
use crate::zml::node::ZmlNode;
use crate::zml::prop::ZmlValue;

/// A named slot declared in a ZML document.
///
/// Slots are insertion points where parent components may inject child content
/// at code-generation time.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ZmlSlot {
    /// Slot identifier referenced by `slot_name` on child nodes.
    pub name: String,

    /// Human-readable description of the slot's purpose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Default content rendered when no content is injected into this slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_content: Option<Vec<ZmlNode>>,
}

impl ZmlSlot {
    /// Creates a new `ZmlSlot`, returning an error if `name` is empty.
    pub fn new(
        name: impl Into<String>,
        description: Option<String>,
        default_content: Option<Vec<ZmlNode>>,
    ) -> ZmlResult<Self> {
        let name: String = name.into().trim().to_owned();
        if name.is_empty() {
            return Err(ZmlError::new(
                ZmlErrorKind::EmptyName,
                "slot name cannot be empty",
            ));
        }
        Ok(Self {
            name,
            description,
            default_content,
        })
    }
}

/// A named data binding declared in a ZML document.
///
/// Bindings wire reactive values from the host application into the component
/// tree via `ZmlValue::BindingRef`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ZmlBinding {
    /// Binding name used in `BindingRef` values (alphanumeric + underscore).
    pub name: String,

    /// The Rust type name of the bound value (e.g. `"String"`, `"u32"`).
    ///
    /// When [`Self::is_list`] is set this is the *element* type `T`, not
    /// `Vec<T>` — see [`Self::rust_type`].
    pub value_type: String,

    /// Optional default value emitted when no runtime value is provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<ZmlValue>,

    /// Whether the host supplies a `Vec<T>` rather than a single `T`.
    ///
    /// This is the shape marker a `Repeat` node's `item_binding` requires: a
    /// repeat may only iterate a binding that carries a collection, and the
    /// editor validates that before codegen ever sees the document.  Absent
    /// from serialized output when `false`, so existing documents round-trip
    /// unchanged.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_list: bool,
}

impl ZmlBinding {
    /// Creates a new `ZmlBinding`, returning an error if `name` is empty or
    /// contains characters other than ASCII alphanumerics and underscores.
    pub fn new(
        name: impl Into<String>,
        value_type: impl Into<String>,
        default: Option<ZmlValue>,
    ) -> ZmlResult<Self> {
        let name = name.into();
        if name.is_empty() {
            return Err(ZmlError::new(
                ZmlErrorKind::EmptyName,
                "binding name cannot be empty",
            ));
        }
        if !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return Err(ZmlError::new(
                ZmlErrorKind::InvalidBindingName,
                "binding name may only contain ASCII alphanumerics and underscores",
            ));
        }
        Ok(Self {
            name,
            value_type: value_type.into(),
            default,
            is_list: false,
        })
    }

    /// Creates a collection binding whose host value is `Vec<element_type>`.
    ///
    /// This is the shape a `Repeat` node's `item_binding` names.
    ///
    /// # Errors
    ///
    /// Same as [`Self::new`].
    pub fn new_list(
        name: impl Into<String>,
        element_type: impl Into<String>,
        default: Option<ZmlValue>,
    ) -> ZmlResult<Self> {
        Ok(Self {
            is_list: true,
            ..Self::new(name, element_type, default)?
        })
    }

    /// The Rust type this binding's `ZmlBindings` field carries.
    ///
    /// `Vec<T>` for a collection binding, `T` otherwise.
    #[must_use]
    pub fn rust_type(&self) -> String {
        if self.is_list {
            format!("Vec<{}>", self.value_type)
        } else {
            self.value_type.clone()
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::zml::prop::ZmlProp;

    fn roundtrip_slot(s: &ZmlSlot) -> ZmlSlot {
        let toml = toml::to_string(s).expect("serialize");
        toml::from_str(&toml).expect("deserialize")
    }

    fn roundtrip_binding(b: &ZmlBinding) -> ZmlBinding {
        let toml = toml::to_string(b).expect("serialize");
        toml::from_str(&toml).expect("deserialize")
    }

    #[test]
    fn empty_slot_roundtrips() {
        let slot = ZmlSlot::new("footer", None, None).expect("valid");
        assert_eq!(roundtrip_slot(&slot), slot);
    }

    #[test]
    fn slot_with_default_content_roundtrips() {
        let child = ZmlNode {
            component_type: "Text".to_owned(),
            props: Some(vec![
                ZmlProp::new("content", ZmlValue::String("Loading…".to_owned())).expect("valid"),
            ]),
            children: None,
            event_handlers: None,
            slot_name: None,
        };
        let slot = ZmlSlot::new(
            "content",
            Some("Main content area".to_owned()),
            Some(vec![child]),
        )
        .expect("valid");
        assert_eq!(roundtrip_slot(&slot), slot);
    }

    #[test]
    fn empty_slot_name_is_rejected() {
        let err = ZmlSlot::new("", None, None).expect_err("empty name should fail");
        assert_eq!(err.kind(), ZmlErrorKind::EmptyName);
    }

    #[test]
    fn whitespace_only_slot_name_is_rejected() {
        let err = ZmlSlot::new("   ", None, None).expect_err("whitespace-only name should fail");
        assert_eq!(err.kind(), ZmlErrorKind::EmptyName);
    }

    #[test]
    fn slot_name_is_trimmed() {
        let slot = ZmlSlot::new("  footer  ", None, None).expect("valid after trim");
        assert_eq!(slot.name, "footer");
    }

    #[test]
    fn binding_with_string_default_roundtrips() {
        let b = ZmlBinding::new(
            "title",
            "String",
            Some(ZmlValue::String("Untitled".to_owned())),
        )
        .expect("valid");
        assert_eq!(roundtrip_binding(&b), b);
    }

    #[test]
    fn binding_with_number_default_roundtrips() {
        let b = ZmlBinding::new("count", "u32", Some(ZmlValue::Number(0.0))).expect("valid");
        assert_eq!(roundtrip_binding(&b), b);
    }

    #[test]
    fn binding_with_bool_default_roundtrips() {
        let b = ZmlBinding::new("visible", "bool", Some(ZmlValue::Bool(true))).expect("valid");
        assert_eq!(roundtrip_binding(&b), b);
    }

    #[test]
    fn binding_empty_name_is_rejected() {
        let err = ZmlBinding::new("", "String", None).expect_err("empty name should fail");
        assert_eq!(err.kind(), ZmlErrorKind::EmptyName);
    }

    #[test]
    fn list_binding_carries_the_element_type() {
        // `value_type` is `T`, not `Vec<T>`; the collection lives in the marker
        // so a `Repeat`'s `item_binding` can be validated against it.
        let b = ZmlBinding::new_list("entries", "HostEntry", None).expect("valid");
        assert_eq!(b.value_type, "HostEntry");
        assert!(b.is_list);
        assert_eq!(b.rust_type(), "Vec<HostEntry>");
        assert_eq!(roundtrip_binding(&b), b);
    }

    #[test]
    fn scalar_binding_omits_the_list_marker_from_output() {
        // Existing documents must round-trip byte-identically.
        let b = ZmlBinding::new("counter", "u32", None).expect("valid");
        let toml = toml::to_string(&b).expect("serialize");
        assert!(!toml.contains("is_list"), "{toml}");
        assert_eq!(b.rust_type(), "u32");
    }

    #[test]
    fn binding_invalid_chars_are_rejected() {
        let err =
            ZmlBinding::new("my-binding", "String", None).expect_err("hyphen should be rejected");
        assert_eq!(err.kind(), ZmlErrorKind::InvalidBindingName);
    }
}
