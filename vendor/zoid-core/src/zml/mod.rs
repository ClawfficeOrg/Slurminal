//! ZML — Zoid Markup Language.
//!
//! ZML is a document format for describing GPUI component trees, stored as
//! either JSON (`ui/app.json`, the primary representation) or TOML
//! (`ui/app.zml`, the legacy representation) — see [`source`]. A ZML document
//! captures:
//!
//! - Window root configuration (title, bounds).
//! - A recursive tree of component [`ZmlNode`]s with typed [`ZmlProp`]s and
//!   [`ZmlEventHandler`]s.
//! - Design-token [`ThemeOverride`]s scoped to the document.
//! - Named [`ZmlSlot`]s for content injection.
//! - Reusable [`ZmlPattern`] sub-trees referenced by `Repeat` nodes.
//! - Overlay [`ZmlOverlay`] definitions opened imperatively by event
//!   handlers' `open_overlay` / `close_overlay` ids.
//! - Reactive data [`ZmlBinding`]s wired from the host application.
//!
//! # Quick start
//!
//! ```
//! use zoid_core::zml::ZmlDocument;
//!
//! let toml = r#"
//! zml_version = "1"
//! window_title = "Hello"
//!
//! [root]
//! component_type = "Window"
//! "#;
//!
//! let doc = ZmlDocument::from_toml(toml).unwrap();
//! assert_eq!(doc.root.component_type, "Window");
//! ```

pub mod document;
pub mod error;
pub mod event_handler;
pub mod node;
pub mod overlay;
pub mod pattern;
pub mod prop;
pub mod slot;
pub mod source;
pub mod theme_override;

pub use document::{WindowBoundsConfig, ZML_VERSION, ZmlDocument};
pub use error::{ZmlError, ZmlErrorKind, ZmlResult};
pub use event_handler::ZmlEventHandler;
pub use node::ZmlNode;
pub use overlay::{
    MAX_OVERLAY_ID_LEN, OVERLAY_DRAWER, OVERLAY_KINDS, OVERLAY_MODAL, ZmlOverlay, overlay_ids,
    validate_overlay_id, validate_overlays,
};
pub use pattern::{
    MAX_PATTERN_DEPTH, MAX_PATTERN_ID_LEN, PROP_COUNT, PROP_ITEM_BINDING, PROP_KEY,
    PROP_PATTERN_ID, PROP_PREVIEW_COUNT, REPEAT_COMPONENT_TYPE, ZmlPattern, is_repeat, number_prop,
    string_prop, unused_pattern_ids, validate_pattern_id, validate_patterns,
};
pub use prop::{ZmlProp, ZmlValue};
pub use slot::{ZmlBinding, ZmlSlot};
pub use source::{APP_JSON_REL, APP_ZML_REL, UiSourceFormat, resolve_ui_source};
pub use theme_override::{MAX_THEME_PATH_DEPTH, ThemeOverride};
