//! Overlay definitions (`ZmlOverlay`) and their validation.
//!
//! `Modal` and `Drawer` were removed from the component palette in 8.15.2
//! because they map to `gpui_component::dialog::Dialog` and
//! `gpui_component::sheet::Sheet` — overlays opened *imperatively*, not children
//! of a layout tree. A canvas node cannot express *when* an overlay opens, so
//! the emitted code would be a permanently-open dialog nested in the layout.
//!
//! Overlays therefore live in a document-level table, parallel to `patterns`
//! (task 8.15.5). The trigger is an event handler: a node's `open_overlay` /
//! `close_overlay` names an overlay id, and codegen turns that into a real
//! `handle.open(..)` / visibility toggle call.
//!
//! # Why overlays live in the document
//!
//! Same reasoning as `patterns`: `zoid_template::generate_zml_layout` is a pure
//! function of one [`ZmlDocument`], and the emitted `ZmlOverlays` struct the
//! host holds must be derivable from the document alone. Overlays carry their
//! own content subtree, so a document is self-contained.
//!
//! See `docs/todo-v8.md` task 8.15.6.

use serde::{Deserialize, Serialize};

use crate::zml::error::{ZmlError, ZmlErrorKind, ZmlResult};
use crate::zml::node::ZmlNode;
use crate::zml::prop::ZmlProp;

/// The overlay kind for a modal dialog (`gpui_component::dialog::Dialog`).
pub const OVERLAY_MODAL: &str = "Modal";

/// The overlay kind for a side sheet / drawer (`gpui_component::sheet::Sheet`).
pub const OVERLAY_DRAWER: &str = "Drawer";

/// The overlay kinds codegen knows how to emit.
///
/// `Toast` is deliberately absent: gpui-component's `ToastManager` +
/// `ToastStack` are clock-driven (the host must `advance(now)` on a loop),
/// which is a different contract from `Dialog`/`Sheet`. Toast stays on the
/// window-level toggle; see todo-v8 8.15.6.
pub const OVERLAY_KINDS: &[&str] = &[OVERLAY_MODAL, OVERLAY_DRAWER];

/// Maximum length of an overlay `id`.
///
/// The id becomes a Rust field name in the generated `ZmlOverlays` struct, so
/// it is validated as a Rust identifier rather than just a charset.
pub const MAX_OVERLAY_ID_LEN: usize = 64;

/// A document-level overlay definition.
///
/// Overlays are opened by an event handler's `open_overlay` id and closed by
/// `close_overlay`. Codegen emits a generated `ZmlOverlays` struct the host
/// view holds, with one handle per overlay:
///
/// - `Modal` → a `gpui_component::dialog::DialogHandle` (imperative
///   `open`/`close`).
/// - `Drawer` → an `Rc<Cell<bool>>` visibility flag, because `Sheet` has no
///   handle and is only rendered when the flag is set.
///
/// `content` is the overlay's body subtree — the dialog popup / sheet surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ZmlOverlay {
    /// Machine-readable id referenced by `open_overlay` / `close_overlay`.
    ///
    /// Validated as a Rust identifier so it can be a generated field name.
    pub id: String,

    /// `Modal` or `Drawer`. Anything else is rejected by
    /// [`validate_overlays`].
    pub kind: String,

    /// Overlay-level configuration (title, width, edge, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub props: Option<Vec<ZmlProp>>,

    /// The overlay body subtree.
    pub content: ZmlNode,
}

impl ZmlOverlay {
    /// Creates a new `ZmlOverlay`.
    ///
    /// # Errors
    ///
    /// Returns [`ZmlErrorKind::InvalidOverlayId`] when `id` is not a valid Rust
    /// identifier (empty, over-length, or containing characters other than
    /// ASCII alphanumerics and `_`).
    pub fn new(
        id: impl Into<String>,
        kind: impl Into<String>,
        props: Option<Vec<ZmlProp>>,
        content: ZmlNode,
    ) -> ZmlResult<Self> {
        let id = id.into();
        validate_overlay_id(&id)?;
        Ok(Self {
            id,
            kind: kind.into(),
            props,
            content,
        })
    }
}

/// Validates an overlay `id`.
///
/// The id becomes a Rust field name in the generated `ZmlOverlays` struct, so
/// unlike a `pattern_id` it must be a valid Rust identifier — not merely a
/// bounded charset.
///
/// # Errors
///
/// Returns [`ZmlErrorKind::InvalidOverlayId`] when `id` is empty, longer than
/// [`MAX_OVERLAY_ID_LEN`], or contains a character outside
/// `[A-Za-z0-9_]` (or starts with a digit).
pub fn validate_overlay_id(id: &str) -> ZmlResult<()> {
    if id.is_empty() {
        return Err(ZmlError::new(
            ZmlErrorKind::InvalidOverlayId,
            "overlay id cannot be empty",
        ));
    }
    if id.len() > MAX_OVERLAY_ID_LEN {
        return Err(ZmlError::new(
            ZmlErrorKind::InvalidOverlayId,
            format!("overlay id exceeds {MAX_OVERLAY_ID_LEN} characters"),
        ));
    }
    let mut chars = id.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => {
            return Err(ZmlError::new(
                ZmlErrorKind::InvalidOverlayId,
                "overlay id must start with an ASCII letter or underscore",
            ));
        }
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(ZmlError::new(
            ZmlErrorKind::InvalidOverlayId,
            "overlay id may only contain ASCII alphanumerics and underscores",
        ));
    }
    Ok(())
}

/// Validates every overlay in the document's `overlays` table.
///
/// Checks:
///
/// - malformed ids ([`ZmlErrorKind::InvalidOverlayId`]),
/// - duplicate ids ([`ZmlErrorKind::DuplicateOverlayId`]),
/// - unknown kinds ([`ZmlErrorKind::UnknownOverlayKind`]) — `Toast` is
///   deliberately not a kind (see [`OVERLAY_KINDS`]).
///
/// # Errors
///
/// Returns the first violation found.
pub fn validate_overlays(overlays: Option<&Vec<ZmlOverlay>>) -> ZmlResult<()> {
    let mut seen = std::collections::HashSet::new();
    for overlay in overlays.into_iter().flatten() {
        validate_overlay_id(&overlay.id)?;
        if !seen.insert(overlay.id.as_str()) {
            return Err(ZmlError::new(
                ZmlErrorKind::DuplicateOverlayId,
                format!("overlay id '{}' is defined more than once", overlay.id),
            ));
        }
        if !OVERLAY_KINDS.contains(&overlay.kind.as_str()) {
            return Err(ZmlError::new(
                ZmlErrorKind::UnknownOverlayKind,
                format!(
                    "overlay '{}' has unknown kind '{}' (expected Modal or Drawer)",
                    overlay.id, overlay.kind
                ),
            ));
        }
    }
    Ok(())
}

/// Returns the ids of every overlay in the table, in declaration order.
#[must_use]
pub fn overlay_ids(overlays: Option<&Vec<ZmlOverlay>>) -> Vec<String> {
    overlays
        .into_iter()
        .flatten()
        .map(|o| o.id.clone())
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn content() -> ZmlNode {
        ZmlNode {
            component_type: "VStack".to_owned(),
            ..Default::default()
        }
    }

    fn overlay(id: &str, kind: &str) -> ZmlOverlay {
        ZmlOverlay::new(id, kind, None, content()).expect("valid overlay")
    }

    #[test]
    fn id_validation_accepts_identifiers() {
        assert!(validate_overlay_id("confirm").is_ok());
        assert!(validate_overlay_id("_confirm_delete").is_ok());
        assert!(validate_overlay_id("a_b_c9").is_ok());
    }

    #[test]
    fn id_validation_rejects_bad_ids() {
        assert_eq!(
            validate_overlay_id("").unwrap_err().kind(),
            ZmlErrorKind::InvalidOverlayId
        );
        assert_eq!(
            validate_overlay_id("9confirm").unwrap_err().kind(),
            ZmlErrorKind::InvalidOverlayId
        );
        assert_eq!(
            validate_overlay_id("confirm-delete").unwrap_err().kind(),
            ZmlErrorKind::InvalidOverlayId
        );
        assert_eq!(
            validate_overlay_id(&"x".repeat(MAX_OVERLAY_ID_LEN + 1))
                .unwrap_err()
                .kind(),
            ZmlErrorKind::InvalidOverlayId
        );
    }

    #[test]
    fn overlay_roundtrips() {
        let o = overlay("confirm", OVERLAY_MODAL);
        let toml = toml::to_string(&o).expect("serialize");
        let back: ZmlOverlay = toml::from_str(&toml).expect("deserialize");
        assert_eq!(back, o);
    }

    #[test]
    fn empty_table_validates() {
        assert!(validate_overlays(None).is_ok());
        assert!(validate_overlays(Some(&vec![])).is_ok());
    }

    #[test]
    fn duplicate_ids_rejected() {
        let err = validate_overlays(Some(&vec![
            overlay("confirm", OVERLAY_MODAL),
            overlay("confirm", OVERLAY_DRAWER),
        ]))
        .expect_err("duplicate id");
        assert_eq!(err.kind(), ZmlErrorKind::DuplicateOverlayId);
    }

    #[test]
    fn unknown_kind_rejected() {
        let err = validate_overlays(Some(&vec![overlay("t", "Toast")])).expect_err("unknown kind");
        assert_eq!(err.kind(), ZmlErrorKind::UnknownOverlayKind);
    }

    #[test]
    fn ids_listed_in_order() {
        let table = vec![overlay("a", OVERLAY_MODAL), overlay("b", OVERLAY_DRAWER)];
        assert_eq!(overlay_ids(Some(&table)), vec!["a", "b"]);
        assert!(overlay_ids(None).is_empty());
    }
}
