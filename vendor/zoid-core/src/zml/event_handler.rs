//! ZML event handler declarations.

use serde::{Deserialize, Serialize};

use crate::zml::error::{ZmlError, ZmlErrorKind, ZmlResult};

/// An event handler attached to a ZML component node.
///
/// Either `action_id` or `handler_code` must be present; both may be present
/// simultaneously (the code generator will prefer `handler_code`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ZmlEventHandler {
    /// The event name that triggers this handler (e.g. `"click"`, `"change"`).
    pub event_type: String,

    /// An opaque action identifier that the host application dispatches.
    ///
    /// Mutually supplementary with `handler_code`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,

    /// Inline closure source used by the code generator.
    ///
    /// Stored as an opaque string; never compiled or evaluated during parsing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handler_code: Option<String>,

    /// Parameter names for the handler closure, in declaration order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handler_params: Option<Vec<String>>,

    /// Id of a document-level overlay (task 8.15.6) this handler opens.
    ///
    /// Overlays are opened imperatively rather than nested in the layout tree,
    /// so a node's `click` or `secondary_click` handler carrying `open_overlay`
    /// is the trigger — codegen emits a `handle.open(..)` call. Mutually
    /// supplementary with `action_id` / `handler_code`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_overlay: Option<String>,

    /// Id of a document-level overlay this handler closes.
    ///
    /// Codegen turns this into a visibility-toggle / `handle.close()` call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close_overlay: Option<String>,
}

impl ZmlEventHandler {
    /// Creates a new `ZmlEventHandler`, returning an error if neither
    /// `action_id` nor `handler_code` is provided.
    pub fn new(
        event_type: impl Into<String>,
        action_id: Option<String>,
        handler_code: Option<String>,
        handler_params: Option<Vec<String>>,
    ) -> ZmlResult<Self> {
        if action_id.is_none() && handler_code.is_none() {
            return Err(ZmlError::new(
                ZmlErrorKind::MissingEventHandler,
                "event handler must have either action_id or handler_code",
            ));
        }
        Ok(Self {
            event_type: event_type.into(),
            action_id,
            handler_code,
            handler_params,
            open_overlay: None,
            close_overlay: None,
        })
    }

    /// Builder: sets the overlay id this handler opens (task 8.15.6).
    #[must_use]
    pub fn opening(mut self, id: impl Into<String>) -> Self {
        self.open_overlay = Some(id.into());
        self
    }

    /// Builder: sets the overlay id this handler closes (task 8.15.6).
    #[must_use]
    pub fn closing(mut self, id: impl Into<String>) -> Self {
        self.close_overlay = Some(id.into());
        self
    }

    /// Creates a handler whose only job is to open an overlay (task 8.15.6).
    ///
    /// Unlike [`Self::new`], no `action_id` / `handler_code` is required — the
    /// overlay trigger *is* the body. Codegen turns `open_overlay` into a
    /// `handle.open(..)` call.
    pub fn opening_overlay(event_type: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            action_id: None,
            handler_code: None,
            handler_params: None,
            open_overlay: Some(id.into()),
            close_overlay: None,
        }
    }

    /// Creates a handler whose only job is to close an overlay (task 8.15.6).
    pub fn closing_overlay(event_type: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            action_id: None,
            handler_code: None,
            handler_params: None,
            open_overlay: None,
            close_overlay: Some(id.into()),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn roundtrip(h: &ZmlEventHandler) -> ZmlEventHandler {
        let s = toml::to_string(h).expect("serialize");
        toml::from_str(&s).expect("deserialize")
    }

    #[test]
    fn action_id_handler_roundtrips() {
        let h =
            ZmlEventHandler::new("click", Some("do_submit".to_owned()), None, None).expect("valid");
        assert_eq!(roundtrip(&h), h);
    }

    #[test]
    fn handler_code_roundtrips() {
        let h = ZmlEventHandler::new(
            "change",
            None,
            Some("|cx, val| cx.emit(CounterEvent::Set(val))".to_owned()),
            Some(vec!["cx".to_owned(), "val".to_owned()]),
        )
        .expect("valid");
        assert_eq!(roundtrip(&h), h);
    }

    #[test]
    fn both_fields_roundtrips() {
        let h = ZmlEventHandler::new(
            "click",
            Some("fallback".to_owned()),
            Some("|cx| cx.emit(Ping)".to_owned()),
            None,
        )
        .expect("valid");
        assert_eq!(roundtrip(&h), h);
    }

    #[test]
    fn missing_both_fields_is_rejected() {
        let err =
            ZmlEventHandler::new("click", None, None, None).expect_err("should fail without body");
        assert_eq!(err.kind(), ZmlErrorKind::MissingEventHandler);
    }

    #[test]
    fn open_overlay_builder_roundtrips() {
        let h = ZmlEventHandler::new("click", Some("do_submit".to_owned()), None, None)
            .expect("valid")
            .opening("confirm");
        assert_eq!(h.open_overlay.as_deref(), Some("confirm"));
        assert!(h.close_overlay.is_none());
        assert_eq!(roundtrip(&h), h);
    }

    #[test]
    fn close_overlay_builder_roundtrips() {
        let h = ZmlEventHandler::new("click", None, Some("cx".to_owned()), None)
            .expect("valid")
            .closing("confirm");
        assert_eq!(h.close_overlay.as_deref(), Some("confirm"));
        assert!(h.open_overlay.is_none());
        assert_eq!(roundtrip(&h), h);
    }

    #[test]
    fn overlay_fields_are_absent_by_default() {
        let h = ZmlEventHandler::new("click", Some("a".to_owned()), None, None).expect("valid");
        assert!(h.open_overlay.is_none());
        assert!(h.close_overlay.is_none());
        assert!(!toml::to_string(&h).expect("serialize").contains("overlay"));
    }
}
