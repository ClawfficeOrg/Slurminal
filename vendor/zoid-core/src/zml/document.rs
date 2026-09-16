//! ZML document root type and TOML round-trip helpers.

use serde::{Deserialize, Serialize};

use crate::zml::error::{ZmlError, ZmlErrorKind, ZmlResult};
use crate::zml::node::ZmlNode;
use crate::zml::overlay::{ZmlOverlay, validate_overlays};
use crate::zml::pattern::{ZmlPattern, validate_patterns};
use crate::zml::slot::{ZmlBinding, ZmlSlot};
use crate::zml::theme_override::ThemeOverride;

/// The schema version written into every ZML document.
pub const ZML_VERSION: &str = "1";

fn default_zml_version() -> String {
    ZML_VERSION.to_owned()
}

/// Optional window geometry configuration within a ZML document.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WindowBoundsConfig {
    /// Initial window width in logical pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    /// Initial window height in logical pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    /// Whether the window should start maximized.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximized: Option<bool>,
}

/// A complete ZML document describing a GPUI component tree.
///
/// ZML documents are stored as TOML with a `.zml` extension.  The
/// `zml_version` field enables forward-compatibility: parsers that encounter
/// an unknown version should emit a warning rather than fail hard.
///
/// # Example (TOML)
///
/// ```toml
/// zml_version = "1"
/// window_title = "My App"
///
/// [root]
/// component_type = "Window"
///
/// [[root.children]]
/// component_type = "Button"
///
/// [[root.children.props]]
/// name = "label"
/// type = "String"
/// value = "Click me"
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ZmlDocument {
    /// ZML schema version for forward-compatibility.
    #[serde(default = "default_zml_version")]
    pub zml_version: String,

    /// Optional window title displayed in the OS title bar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_title: Option<String>,

    /// Optional initial window geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_bounds: Option<WindowBoundsConfig>,

    /// The root component node of the UI tree.
    pub root: ZmlNode,

    /// Design-token overrides scoped to this document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_overrides: Option<Vec<ThemeOverride>>,

    /// Named slots declared for use by child components.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slots: Option<Vec<ZmlSlot>>,

    /// Data bindings wired from the host application into the component tree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bindings: Option<Vec<ZmlBinding>>,

    /// Reusable node sub-trees referenced by `pattern_id` (e.g. from `Repeat`).
    ///
    /// Definitions live in the document rather than in the editor's pattern
    /// library so that codegen stays a pure function of one document — see
    /// [`crate::zml::pattern`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patterns: Option<Vec<ZmlPattern>>,

    /// Overlay definitions (Modal / Drawer) opened by event handlers'
    /// `open_overlay` / `close_overlay` ids.
    ///
    /// Overlays are not layout-tree children — they are opened imperatively —
    /// so they live in a document-level table, parallel to `patterns` (task
    /// 8.15.6). See [`crate::zml::overlay`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlays: Option<Vec<ZmlOverlay>>,
}

impl ZmlDocument {
    /// Deserializes a `ZmlDocument` from a TOML string.
    ///
    /// # Errors
    ///
    /// Returns [`ZmlError`] with kind `TomlParse` if the input is not valid
    /// TOML or does not match the `ZmlDocument` schema, or one of the pattern
    /// kinds if the `patterns` table fails [`Self::validate_patterns`].
    pub fn from_toml(input: &str) -> ZmlResult<Self> {
        let mut doc: Self = toml::from_str(input).map_err(ZmlError::from)?;
        migrate_legacy_props(&mut doc.root);
        doc.validate_patterns()?;
        doc.validate_overlays()?;
        Ok(doc)
    }

    /// Serializes this document to a TOML string.
    ///
    /// # Errors
    ///
    /// Returns [`ZmlError`] with kind `TomlSerialize` if serialization fails.
    pub fn to_toml(&self) -> ZmlResult<String> {
        toml::to_string(self).map_err(ZmlError::from)
    }

    /// Deserializes a `ZmlDocument` from a JSON string (task 8.12.1).
    ///
    /// JSON is the primary UI-source representation for new projects; the
    /// schema is identical to the TOML form.
    ///
    /// # Errors
    ///
    /// Returns [`ZmlError`] with kind `JsonParse` if the input is not valid
    /// JSON or does not match the `ZmlDocument` schema, or one of the pattern
    /// kinds if the `patterns` table fails [`Self::validate_patterns`].
    pub fn from_json(input: &str) -> ZmlResult<Self> {
        let mut doc: Self = serde_json::from_str(input)
            .map_err(|e| ZmlError::with_source(ZmlErrorKind::JsonParse, e.to_string(), e))?;
        migrate_legacy_props(&mut doc.root);
        doc.validate_patterns()?;
        doc.validate_overlays()?;
        Ok(doc)
    }

    /// Validates every `pattern_id` reference this document carries.
    ///
    /// Run on both load paths, so a document whose `patterns` table is
    /// cyclic, self-referential, or incomplete fails loudly at parse time
    /// instead of expanding without bound during codegen.
    ///
    /// # Errors
    ///
    /// See [`validate_patterns`].
    pub fn validate_patterns(&self) -> ZmlResult<()> {
        validate_patterns(&self.root, self.patterns.as_ref())
    }

    /// Looks up a pattern by id within this document's own table.
    ///
    /// Documents never reference the editor's global library, so this is the
    /// only resolution path a `pattern_id` has.
    #[must_use]
    pub fn pattern(&self, id: &str) -> Option<&ZmlPattern> {
        self.patterns.iter().flatten().find(|p| p.id == id)
    }

    /// Validates every overlay in the document's `overlays` table.
    ///
    /// Run on both load paths, so an overlay table with duplicate ids, unknown
    /// kinds, or malformed ids fails loudly at parse time.
    ///
    /// # Errors
    ///
    /// See [`validate_overlays`].
    pub fn validate_overlays(&self) -> ZmlResult<()> {
        validate_overlays(self.overlays.as_ref())
    }

    /// Looks up an overlay by id within this document's own table.
    ///
    /// This is the only resolution path an `open_overlay` / `close_overlay`
    /// event-handler id has, mirroring [`Self::pattern`].
    #[must_use]
    pub fn overlay(&self, id: &str) -> Option<&ZmlOverlay> {
        self.overlays.iter().flatten().find(|o| o.id == id)
    }

    /// Serializes this document to a pretty-printed JSON string.
    ///
    /// # Errors
    ///
    /// Returns [`ZmlError`] with kind `JsonSerialize` if serialization fails.
    pub fn to_json(&self) -> ZmlResult<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| ZmlError::with_source(ZmlErrorKind::JsonSerialize, e.to_string(), e))
    }
}

/// Rewrites prop names that older documents carry but codegen cannot emit.
///
/// Applied on every load path (`from_toml`, `from_json`) rather than behind a
/// `zml_version` bump: the changes are renames and one removal, they are
/// idempotent, and a document that has already been migrated is indistinguishable
/// from a freshly written one — so there is nothing for a version gate to
/// decide.
///
/// - `row`/`col` become `_row`/`_col`. They are grid-position metadata that the
///   prop editor offers on every component, but no widget has a `.row()` or
///   `.col()` builder, so emitting them produced code that did not compile. The
///   underscore prefix is the existing convention for editor-only props
///   (`_display_name`, `_tooltips`), and `emit_node` already skips it.
/// - `Label.align` is dropped. `zoid_gpui::Label` has no alignment field and no
///   `.align()` builder, so the prop was wired to nothing.
///
/// See todo-v8 8.15.1.
fn migrate_legacy_props(node: &mut ZmlNode) {
    if let Some(props) = node.props.as_mut() {
        if node.component_type == "Label" {
            props.retain(|p| p.name != "align");
        }
        for (legacy, replacement) in [("row", "_row"), ("col", "_col")] {
            // A document that already carries the underscore form keeps it; the
            // legacy duplicate is dropped rather than renamed on top of it.
            if props.iter().any(|p| p.name == replacement) {
                props.retain(|p| p.name != legacy);
            } else if let Some(prop) = props.iter_mut().find(|p| p.name == legacy) {
                prop.name = replacement.to_owned();
            }
        }
    }
    for child in node.children.iter_mut().flatten() {
        migrate_legacy_props(child);
    }
}

impl Default for ZmlDocument {
    fn default() -> Self {
        Self {
            zml_version: ZML_VERSION.to_owned(),
            window_title: None,
            window_bounds: None,
            root: ZmlNode {
                component_type: "Window".to_owned(),
                ..ZmlNode::default()
            },
            theme_overrides: None,
            slots: None,
            bindings: None,
            patterns: None,
            overlays: None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::zml::event_handler::ZmlEventHandler;
    use crate::zml::node::ZmlNode;
    use crate::zml::overlay::{OVERLAY_DRAWER, OVERLAY_MODAL, ZmlOverlay};
    use crate::zml::prop::{ZmlProp, ZmlValue};
    use crate::zml::slot::{ZmlBinding, ZmlSlot};
    use crate::zml::theme_override::ThemeOverride;

    fn roundtrip(doc: &ZmlDocument) -> ZmlDocument {
        let toml = doc.to_toml().expect("serialize");
        ZmlDocument::from_toml(&toml).expect("deserialize")
    }

    #[test]
    fn minimal_document_roundtrips() {
        let doc = ZmlDocument::default();
        assert_eq!(roundtrip(&doc), doc);
    }

    #[test]
    fn full_document_roundtrips() {
        let button = ZmlNode {
            component_type: "Button".to_owned(),
            props: Some(vec![
                ZmlProp::new("label", ZmlValue::String("Submit".to_owned())).expect("valid"),
                ZmlProp::new("size", ZmlValue::Number(14.0)).expect("valid"),
            ]),
            children: Some(vec![ZmlNode {
                component_type: "Icon".to_owned(),
                props: Some(vec![
                    ZmlProp::new("name", ZmlValue::String("send".to_owned())).expect("valid"),
                ]),
                children: None,
                event_handlers: None,
                slot_name: Some("icon".to_owned()),
            }]),
            event_handlers: Some(vec![
                ZmlEventHandler::new("click", Some("handle_submit".to_owned()), None, None)
                    .expect("valid"),
            ]),
            slot_name: None,
        };

        let doc = ZmlDocument {
            overlays: None,
            zml_version: ZML_VERSION.to_owned(),
            window_title: Some("Demo App".to_owned()),
            window_bounds: Some(WindowBoundsConfig {
                width: Some(800.0),
                height: Some(600.0),
                maximized: None,
            }),
            root: ZmlNode {
                component_type: "Window".to_owned(),
                props: None,
                children: Some(vec![button]),
                event_handlers: None,
                slot_name: None,
            },
            theme_overrides: Some(vec![
                ThemeOverride::new(
                    vec!["color".to_owned(), "accent".to_owned()],
                    ZmlValue::String("#7c3aed".to_owned()),
                )
                .expect("valid"),
            ]),
            slots: Some(vec![
                ZmlSlot::new("footer", Some("Footer content area".to_owned()), None)
                    .expect("valid"),
            ]),
            bindings: Some(vec![
                ZmlBinding::new("counter", "u32", Some(ZmlValue::Number(0.0))).expect("valid"),
            ]),
            patterns: None,
        };

        assert_eq!(roundtrip(&doc), doc);
    }

    #[test]
    fn version_field_is_preserved() {
        let doc = ZmlDocument::default();
        let back = roundtrip(&doc);
        assert_eq!(back.zml_version, ZML_VERSION);
    }

    fn json_roundtrip(doc: &ZmlDocument) -> ZmlDocument {
        let json = doc.to_json().expect("serialize");
        ZmlDocument::from_json(&json).expect("deserialize")
    }

    #[test]
    fn minimal_document_roundtrips_through_json() {
        let doc = ZmlDocument::default();
        assert_eq!(json_roundtrip(&doc), doc);
    }

    #[test]
    fn full_document_roundtrips_through_json() {
        let button = ZmlNode {
            component_type: "Button".to_owned(),
            props: Some(vec![
                ZmlProp::new("label", ZmlValue::String("Submit".to_owned())).expect("valid"),
                ZmlProp::new("size", ZmlValue::Number(14.0)).expect("valid"),
                ZmlProp::new("enabled", ZmlValue::Bool(true)).expect("valid"),
                ZmlProp::new("target", ZmlValue::BindingRef("state".to_owned())).expect("valid"),
            ]),
            children: Some(vec![ZmlNode {
                component_type: "Icon".to_owned(),
                props: Some(vec![
                    ZmlProp::new("name", ZmlValue::String("send".to_owned())).expect("valid"),
                ]),
                children: None,
                event_handlers: None,
                slot_name: Some("icon".to_owned()),
            }]),
            event_handlers: Some(vec![
                ZmlEventHandler::new("click", Some("handle_submit".to_owned()), None, None)
                    .expect("valid"),
            ]),
            slot_name: None,
        };

        let doc = ZmlDocument {
            overlays: None,
            zml_version: ZML_VERSION.to_owned(),
            window_title: Some("Demo App".to_owned()),
            window_bounds: Some(WindowBoundsConfig {
                width: Some(800.0),
                height: Some(600.0),
                maximized: None,
            }),
            root: ZmlNode {
                component_type: "Window".to_owned(),
                props: None,
                children: Some(vec![button]),
                event_handlers: None,
                slot_name: None,
            },
            theme_overrides: Some(vec![
                ThemeOverride::new(
                    vec!["color".to_owned(), "accent".to_owned()],
                    ZmlValue::String("#7c3aed".to_owned()),
                )
                .expect("valid"),
            ]),
            slots: Some(vec![
                ZmlSlot::new("footer", Some("Footer content area".to_owned()), None)
                    .expect("valid"),
            ]),
            bindings: Some(vec![
                ZmlBinding::new("counter", "u32", Some(ZmlValue::Number(0.0))).expect("valid"),
            ]),
            patterns: None,
        };

        assert_eq!(json_roundtrip(&doc), doc);
    }

    #[test]
    fn patterns_table_roundtrips_through_json() {
        let doc = ZmlDocument {
            overlays: None,
            root: ZmlNode {
                component_type: "Window".to_owned(),
                children: Some(vec![ZmlNode {
                    component_type: "Repeat".to_owned(),
                    props: Some(vec![
                        ZmlProp::new("pattern_id", ZmlValue::String("row".to_owned()))
                            .expect("valid"),
                        ZmlProp::new("item_binding", ZmlValue::String("entries".to_owned()))
                            .expect("valid"),
                    ]),
                    ..ZmlNode::default()
                }]),
                ..ZmlNode::default()
            },
            patterns: Some(vec![
                ZmlPattern::new(
                    "row",
                    "Host row",
                    ZmlNode {
                        component_type: "HStack".to_owned(),
                        ..ZmlNode::default()
                    },
                )
                .expect("valid"),
            ]),
            ..ZmlDocument::default()
        };
        assert_eq!(json_roundtrip(&doc), doc);
    }

    #[test]
    fn a_document_without_patterns_omits_the_field_entirely() {
        // Not `null`: older parsers must see a document indistinguishable from
        // one written before the table existed.
        let json = ZmlDocument::default().to_json().expect("serialize");
        assert!(!json.contains("patterns"), "{json}");
    }

    #[test]
    fn overlays_roundtrip_and_resolve_by_id() {
        let doc = ZmlDocument {
            overlays: Some(vec![
                ZmlOverlay::new(
                    "confirm",
                    OVERLAY_MODAL,
                    None,
                    ZmlNode {
                        component_type: "VStack".to_owned(),
                        ..Default::default()
                    },
                )
                .expect("valid"),
                ZmlOverlay::new(
                    "filters",
                    OVERLAY_DRAWER,
                    None,
                    ZmlNode {
                        component_type: "VStack".to_owned(),
                        ..Default::default()
                    },
                )
                .expect("valid"),
            ]),
            ..ZmlDocument::default()
        };

        assert_eq!(roundtrip(&doc), doc);
        assert_eq!(json_roundtrip(&doc), doc);
        assert_eq!(doc.overlay("confirm").expect("modal").kind, OVERLAY_MODAL);
        assert_eq!(doc.overlay("filters").expect("drawer").kind, OVERLAY_DRAWER);
        assert!(doc.overlay("missing").is_none());
    }

    #[test]
    fn a_document_without_overlays_omits_the_field_entirely() {
        let json = ZmlDocument::default().to_json().expect("serialize");
        assert!(!json.contains("overlays"), "{json}");
    }

    #[test]
    fn duplicate_or_unknown_overlay_fails_at_parse() {
        let dup = ZmlDocument::from_json(
            r#"{"root":{"component_type":"Window"},
                "overlays":[
                  {"id":"confirm","kind":"Modal","content":{"component_type":"VStack"}},
                  {"id":"confirm","kind":"Drawer","content":{"component_type":"VStack"}}]}"#,
        )
        .expect_err("duplicate overlay id");
        assert_eq!(dup.kind(), ZmlErrorKind::DuplicateOverlayId);

        let unknown = ZmlDocument::from_json(
            r#"{"root":{"component_type":"Window"},
                "overlays":[
                  {"id":"toast","kind":"Toast","content":{"component_type":"VStack"}}]}"#,
        )
        .expect_err("unknown overlay kind");
        assert_eq!(unknown.kind(), ZmlErrorKind::UnknownOverlayKind);
    }

    #[test]
    fn an_open_overlay_handler_wires_through_roundtrip() {
        let button = ZmlNode {
            component_type: "Button".to_owned(),
            props: Some(vec![
                ZmlProp::new("label", ZmlValue::String("Delete".to_owned())).expect("valid"),
            ]),
            children: None,
            event_handlers: Some(vec![ZmlEventHandler::opening_overlay("click", "confirm")]),
            slot_name: None,
        };
        let doc = ZmlDocument {
            overlays: Some(vec![
                ZmlOverlay::new(
                    "confirm",
                    OVERLAY_MODAL,
                    None,
                    ZmlNode {
                        component_type: "VStack".to_owned(),
                        ..Default::default()
                    },
                )
                .expect("valid"),
            ]),
            root: ZmlNode {
                component_type: "Window".to_owned(),
                children: Some(vec![button]),
                ..Default::default()
            },
            ..ZmlDocument::default()
        };

        let back = roundtrip(&doc);
        assert_eq!(back, doc);
        let handler = &back.root.children.as_ref().expect("children")[0]
            .event_handlers
            .as_ref()
            .expect("handlers")[0];
        assert_eq!(handler.open_overlay.as_deref(), Some("confirm"));
        assert!(back.overlay("confirm").is_some());
    }

    #[test]
    fn an_unresolvable_pattern_ref_fails_at_parse() {
        // Loud error, not silently wrong output.
        let err = ZmlDocument::from_json(
            r#"{"root":{"component_type":"Repeat","props":[
                 {"name":"pattern_id","type":"String","value":"missing"}]}}"#,
        )
        .expect_err("unknown pattern id");
        assert_eq!(err.kind(), ZmlErrorKind::UnknownPatternId);
    }

    #[test]
    fn malformed_json_errors_with_json_parse_kind() {
        let err = ZmlDocument::from_json("{ not json").expect_err("malformed");
        assert_eq!(err.kind(), ZmlErrorKind::JsonParse);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod legacy_prop_migration_tests {
    use super::*;
    use crate::zml::prop::ZmlValue;

    fn names(node: &ZmlNode) -> Vec<&str> {
        node.props
            .iter()
            .flatten()
            .map(|p| p.name.as_str())
            .collect()
    }

    #[test]
    fn row_and_col_become_editor_only() {
        // No widget has a `.row()`/`.col()` builder, so the legacy names emitted
        // code that did not compile.
        let doc = ZmlDocument::from_json(
            r#"{"root":{"component_type":"Button","props":[
                 {"name":"row","type":"Number","value":1},
                 {"name":"col","type":"Number","value":2}]}}"#,
        )
        .expect("parse");
        assert_eq!(names(&doc.root), vec!["_row", "_col"]);
    }

    #[test]
    fn label_align_is_dropped() {
        // `zoid_gpui::Label` has no alignment field and no `.align()` builder.
        let doc = ZmlDocument::from_json(
            r#"{"root":{"component_type":"Label","props":[
                 {"name":"text","type":"String","value":"Hi"},
                 {"name":"align","type":"String","value":"left"}]}}"#,
        )
        .expect("parse");
        assert_eq!(names(&doc.root), vec!["text"]);
    }

    #[test]
    fn align_survives_on_other_components() {
        // Only Label loses it here; container `align` is a separate defect with
        // a different fix (see todo-v8 8.15.8).
        let doc = ZmlDocument::from_json(
            r#"{"root":{"component_type":"VStack","props":[
                 {"name":"align","type":"String","value":"center"}]}}"#,
        )
        .expect("parse");
        assert_eq!(names(&doc.root), vec!["align"]);
    }

    #[test]
    fn migration_reaches_nested_children() {
        let doc = ZmlDocument::from_json(
            r#"{"root":{"component_type":"Window","children":[
                 {"component_type":"VStack","children":[
                   {"component_type":"Button","props":[
                     {"name":"row","type":"Number","value":3}]}]}]}}"#,
        )
        .expect("parse");
        let leaf = &doc.root.children.as_ref().expect("l1")[0]
            .children
            .as_ref()
            .expect("l2")[0];
        assert_eq!(names(leaf), vec!["_row"]);
    }

    #[test]
    fn migration_is_idempotent_and_drops_duplicates() {
        // Re-loading an already-migrated document must not produce `__row`, and
        // a document carrying both forms keeps the underscore one.
        let doc = ZmlDocument::from_json(
            r#"{"root":{"component_type":"Button","props":[
                 {"name":"_row","type":"Number","value":1},
                 {"name":"row","type":"Number","value":9}]}}"#,
        )
        .expect("parse");
        assert_eq!(names(&doc.root), vec!["_row"]);
        let again = ZmlDocument::from_json(&doc.to_json().expect("json")).expect("reparse");
        assert_eq!(names(&again.root), vec!["_row"]);
        assert!(matches!(
            again.root.props.as_ref().expect("props")[0].value,
            ZmlValue::Number(n) if (n - 1.0).abs() < f64::EPSILON
        ));
    }
}
