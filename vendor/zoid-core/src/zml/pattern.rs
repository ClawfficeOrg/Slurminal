//! Reusable node sub-trees (`ZmlPattern`) and the `Repeat` node's contract.
//!
//! A `Repeat` node iterates its per-item template over a collection binding.
//! The template is either a single inline child (Mode A) or a reference by
//! `pattern_id` into the document's own [`ZmlDocument::patterns`] table
//! (Mode B).
//!
//! # Why patterns live in the document
//!
//! `zoid_template::generate_zml_layout` is a pure function of one
//! [`ZmlDocument`], and `zoid_template` does not depend on `zoid_editor` — the
//! dependency runs the other way.  Resolving `pattern_id` against the editor's
//! `.zoid-patterns.json` library would therefore require both a new crate
//! dependency and a project directory at codegen time.  Instead the definitions
//! are copied *into* the document on use, so a `pattern_id` always resolves
//! within the document that carries it:
//!
//! > A `pattern_id` in a document always resolves within that document's own
//! > `patterns` table.  Documents never reference the global library.
//!
//! [`ZmlPattern::source_id`] and [`ZmlPattern::source_version`] record where a
//! copy came from so the editor can offer "the global version changed —
//! update?".  That is the relationship a lockfile has to a registry, and it is
//! never resolved at codegen time.
//!
//! See `docs/repeat-node-design.md` (task 8.15.5).

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::zml::error::{ZmlError, ZmlErrorKind, ZmlResult};
use crate::zml::node::ZmlNode;
use crate::zml::prop::ZmlValue;

/// The `component_type` of an iterating container node.
pub const REPEAT_COMPONENT_TYPE: &str = "Repeat";

/// Names the `Vec<T>` binding a `Repeat` iterates over.
pub const PROP_ITEM_BINDING: &str = "item_binding";

/// References a [`ZmlPattern`] in the document's `patterns` table (Mode B).
pub const PROP_PATTERN_ID: &str = "pattern_id";

/// Names the per-item field used as the row's stable element id.
pub const PROP_KEY: &str = "key";

/// Fixed iteration count for a `Repeat` with no `item_binding`.
pub const PROP_COUNT: &str = "count";

/// Editor-only ghost-preview count.  Stripped by codegen like `_display_name`.
pub const PROP_PREVIEW_COUNT: &str = "_preview_count";

/// Maximum length of a `pattern_id` value.
///
/// Prop *names* go through identifier validation; prop *values* do not, and
/// `pattern_id` is the first prop value used as a lookup key.
pub const MAX_PATTERN_ID_LEN: usize = 64;

/// Maximum node depth explored while expanding pattern references.
///
/// A `patterns` table plus `pattern_id` refs forms a graph supplied by
/// user-controlled JSON.  Cycle detection catches the repeating case; this cap
/// bounds the merely very deep one.
pub const MAX_PATTERN_DEPTH: usize = 64;

/// A reusable node sub-tree referenced by `pattern_id`.
///
/// The root is a [`ZmlNode`] — the already-serializable node form — rather than
/// a separate stored shape, so the editor's converters (`zml_sync`) move it
/// to and from the canvas without a third representation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ZmlPattern {
    /// Machine-readable id referenced by a `pattern_id` prop.
    pub id: String,

    /// Human-readable name shown in the palette.
    pub name: String,

    /// SVG icon path for the palette card.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,

    /// The per-item template root.
    pub root: ZmlNode,

    /// Version of this pattern's definition.
    ///
    /// Library patterns start at 1 and are bumped every time their root is
    /// re-saved; `0` means "no version tracked" (built-in patterns, documents
    /// written before the field existed). A document copy records the library
    /// version it was copied at in [`ZmlPattern::source_version`], so
    /// `library.version > copy.source_version` is the "update?" signal.
    #[serde(default, skip_serializing_if = "version_is_zero")]
    pub version: u32,

    /// Id of the global-library pattern this was copied from, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,

    /// Version of the global-library pattern this was copied from, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_version: Option<u32>,
}

/// `#[serde(skip_serializing_if)]` predicate for [`ZmlPattern::version`].
#[must_use]
fn version_is_zero(version: &u32) -> bool {
    *version == 0
}

impl ZmlPattern {
    /// Creates a new `ZmlPattern`.
    ///
    /// # Errors
    ///
    /// Returns [`ZmlErrorKind::EmptyName`] if `name` is blank, or
    /// [`ZmlErrorKind::InvalidPatternId`] if `id` is empty, over
    /// [`MAX_PATTERN_ID_LEN`], or contains characters outside ASCII
    /// alphanumerics, `_`, and `-`.
    pub fn new(id: impl Into<String>, name: impl Into<String>, root: ZmlNode) -> ZmlResult<Self> {
        let id: String = id.into().trim().to_owned();
        validate_pattern_id(&id)?;
        let name: String = name.into().trim().to_owned();
        if name.is_empty() {
            return Err(ZmlError::new(
                ZmlErrorKind::EmptyName,
                "pattern name cannot be empty",
            ));
        }
        Ok(Self {
            id,
            name,
            icon: None,
            root,
            version: 0,
            source_id: None,
            source_version: None,
        })
    }
}

/// Validates a `pattern_id` value used as a lookup key.
///
/// `pattern_id` is never emitted as Rust, so this is a narrower check than
/// identifier validation — but it is not skippable: the value indexes a table
/// and comes from user-supplied JSON.
///
/// # Errors
///
/// Returns [`ZmlErrorKind::InvalidPatternId`] when `id` is empty, longer than
/// [`MAX_PATTERN_ID_LEN`], or contains a character outside
/// `[A-Za-z0-9_-]`.
pub fn validate_pattern_id(id: &str) -> ZmlResult<()> {
    if id.is_empty() {
        return Err(ZmlError::new(
            ZmlErrorKind::InvalidPatternId,
            "pattern id cannot be empty",
        ));
    }
    if id.len() > MAX_PATTERN_ID_LEN {
        return Err(ZmlError::new(
            ZmlErrorKind::InvalidPatternId,
            format!("pattern id exceeds {MAX_PATTERN_ID_LEN} characters"),
        ));
    }
    if !id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(ZmlError::new(
            ZmlErrorKind::InvalidPatternId,
            format!(
                "pattern id '{id}' may only contain ASCII alphanumerics, underscores, and hyphens"
            ),
        ));
    }
    Ok(())
}

/// Returns `true` when `node` is a `Repeat` container.
#[must_use]
pub fn is_repeat(node: &ZmlNode) -> bool {
    node.component_type == REPEAT_COMPONENT_TYPE
}

/// Reads a string-valued prop off a node, ignoring props of other value types.
#[must_use]
pub fn string_prop<'a>(node: &'a ZmlNode, name: &str) -> Option<&'a str> {
    node.props
        .iter()
        .flatten()
        .find(|p| p.name == name)
        .and_then(|p| match &p.value {
            ZmlValue::String(s) => Some(s.as_str()),
            _ => None,
        })
}

/// Reads a number-valued prop off a node.
#[must_use]
pub fn number_prop(node: &ZmlNode, name: &str) -> Option<f64> {
    node.props
        .iter()
        .flatten()
        .find(|p| p.name == name)
        .and_then(|p| match p.value {
            ZmlValue::Number(n) => Some(n),
            _ => None,
        })
}

/// Validates every `pattern_id` reference reachable from a document.
///
/// Checks, over the document root *and* every entry in the `patterns` table so
/// that an unused-but-broken definition is still caught:
///
/// - duplicate ids in the table ([`ZmlErrorKind::DuplicatePatternId`]),
/// - malformed id values ([`ZmlErrorKind::InvalidPatternId`]),
/// - references to ids the table does not define
///   ([`ZmlErrorKind::UnknownPatternId`]),
/// - reference cycles, direct or mutual ([`ZmlErrorKind::PatternCycle`]) —
///   without which expansion is unbounded, driven by user-supplied JSON,
/// - trees deeper than [`MAX_PATTERN_DEPTH`] ([`ZmlErrorKind::PatternTooDeep`]).
///
/// # Errors
///
/// Returns the first violation found.
pub fn validate_patterns(root: &ZmlNode, patterns: Option<&Vec<ZmlPattern>>) -> ZmlResult<()> {
    let mut table: BTreeMap<&str, &ZmlPattern> = BTreeMap::new();
    for pattern in patterns.into_iter().flatten() {
        validate_pattern_id(&pattern.id)?;
        if table.insert(pattern.id.as_str(), pattern).is_some() {
            return Err(ZmlError::new(
                ZmlErrorKind::DuplicatePatternId,
                format!("pattern id '{}' is defined more than once", pattern.id),
            ));
        }
    }

    let mut visiting: Vec<&str> = Vec::new();
    walk(root, &table, &mut visiting, 0)?;
    for pattern in patterns.into_iter().flatten() {
        visiting.push(pattern.id.as_str());
        walk(&pattern.root, &table, &mut visiting, 0)?;
        visiting.pop();
    }
    Ok(())
}

/// Depth-first walk resolving `pattern_id` refs, carrying the chain of
/// pattern ids currently being expanded so a repeat of one is a cycle.
fn walk<'a>(
    node: &'a ZmlNode,
    table: &BTreeMap<&'a str, &'a ZmlPattern>,
    visiting: &mut Vec<&'a str>,
    depth: usize,
) -> ZmlResult<()> {
    if depth > MAX_PATTERN_DEPTH {
        return Err(ZmlError::new(
            ZmlErrorKind::PatternTooDeep,
            format!("node tree exceeds the maximum depth of {MAX_PATTERN_DEPTH}"),
        ));
    }

    if is_repeat(node)
        && let Some(id) = string_prop(node, PROP_PATTERN_ID)
    {
        validate_pattern_id(id)?;
        let Some(pattern) = table.get(id) else {
            return Err(ZmlError::new(
                ZmlErrorKind::UnknownPatternId,
                format!("pattern id '{id}' is not defined in this document"),
            ));
        };
        if visiting.contains(&pattern.id.as_str()) {
            return Err(ZmlError::new(
                ZmlErrorKind::PatternCycle,
                format!(
                    "pattern id '{}' references itself through {}",
                    pattern.id,
                    visiting.join(" -> ")
                ),
            ));
        }
        visiting.push(pattern.id.as_str());
        walk(&pattern.root, table, visiting, depth + 1)?;
        visiting.pop();
    }

    for child in node.children.iter().flatten() {
        walk(child, table, visiting, depth + 1)?;
    }
    Ok(())
}

/// Returns the ids of patterns no `Repeat` in the document references.
///
/// Unused definitions are *kept*, not pruned: deleting a `Repeat` must not
/// silently discard the pattern it referenced.  Removal is an explicit editor
/// action, and this is what it lists.
#[must_use]
pub fn unused_pattern_ids(root: &ZmlNode, patterns: Option<&Vec<ZmlPattern>>) -> Vec<String> {
    let mut used: HashSet<&str> = HashSet::new();
    collect_refs(root, &mut used);
    for pattern in patterns.into_iter().flatten() {
        collect_refs(&pattern.root, &mut used);
    }
    patterns
        .into_iter()
        .flatten()
        .filter(|p| !used.contains(p.id.as_str()))
        .map(|p| p.id.clone())
        .collect()
}

fn collect_refs<'a>(node: &'a ZmlNode, out: &mut HashSet<&'a str>) {
    if is_repeat(node)
        && let Some(id) = string_prop(node, PROP_PATTERN_ID)
    {
        out.insert(id);
    }
    for child in node.children.iter().flatten() {
        collect_refs(child, out);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::zml::prop::ZmlProp;

    fn repeat_with(props: Vec<ZmlProp>, children: Option<Vec<ZmlNode>>) -> ZmlNode {
        ZmlNode {
            component_type: REPEAT_COMPONENT_TYPE.to_owned(),
            props: Some(props),
            children,
            event_handlers: None,
            slot_name: None,
        }
    }

    fn pattern_ref(id: &str) -> ZmlNode {
        repeat_with(
            vec![ZmlProp::new(PROP_PATTERN_ID, ZmlValue::String(id.to_owned())).expect("valid")],
            None,
        )
    }

    fn window(children: Vec<ZmlNode>) -> ZmlNode {
        ZmlNode {
            component_type: "Window".to_owned(),
            props: None,
            children: Some(children),
            event_handlers: None,
            slot_name: None,
        }
    }

    #[test]
    fn pattern_roundtrips_through_json() {
        let mut pattern = ZmlPattern::new(
            "host-row",
            "Host row",
            ZmlNode {
                component_type: "HStack".to_owned(),
                ..ZmlNode::default()
            },
        )
        .expect("valid");
        pattern.version = 2;
        let json = serde_json::to_string(&pattern).expect("serialize");
        let back: ZmlPattern = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, pattern);
    }

    #[test]
    fn a_zero_version_is_absent_from_json() {
        // Documents and libraries written before the field existed must stay
        // byte-stable, and new JSON must not grow a `"version": 0` on every
        // pattern.
        let pattern = ZmlPattern::new("row", "Row", ZmlNode::default()).expect("valid");
        let json = serde_json::to_string(&pattern).expect("serialize");
        assert!(!json.contains("version"), "{json}");
    }

    #[test]
    fn legacy_json_without_version_defaults_to_zero() {
        let json = r#"{"id":"row","name":"Row","root":{"component_type":"HStack"}}"#;
        let back: ZmlPattern = serde_json::from_str(json).expect("deserialize");
        assert_eq!(back.version, 0);
    }

    #[test]
    fn pattern_id_charset_is_enforced() {
        assert!(validate_pattern_id("host-row_2").is_ok());
        for bad in ["", "host row", "host.row", "host/../row", "héllo"] {
            let err = validate_pattern_id(bad).expect_err("should reject");
            assert_eq!(err.kind(), ZmlErrorKind::InvalidPatternId, "for {bad:?}");
        }
        let long = "a".repeat(MAX_PATTERN_ID_LEN + 1);
        assert_eq!(
            validate_pattern_id(&long).expect_err("too long").kind(),
            ZmlErrorKind::InvalidPatternId
        );
    }

    #[test]
    fn empty_pattern_name_is_rejected() {
        let err = ZmlPattern::new("id", "  ", ZmlNode::default()).expect_err("blank name");
        assert_eq!(err.kind(), ZmlErrorKind::EmptyName);
    }

    #[test]
    fn known_reference_validates() {
        let patterns = vec![ZmlPattern::new("row", "Row", ZmlNode::default()).expect("valid")];
        validate_patterns(&window(vec![pattern_ref("row")]), Some(&patterns)).expect("valid");
    }

    #[test]
    fn unknown_reference_is_rejected() {
        let err =
            validate_patterns(&window(vec![pattern_ref("missing")]), None).expect_err("unknown id");
        assert_eq!(err.kind(), ZmlErrorKind::UnknownPatternId);
    }

    #[test]
    fn self_referential_pattern_is_a_cycle_not_an_expansion() {
        // Without the cycle check this expands until memory runs out, driven by
        // user-supplied JSON.
        let patterns = vec![ZmlPattern::new("row", "Row", pattern_ref("row")).expect("valid")];
        let err = validate_patterns(&window(vec![pattern_ref("row")]), Some(&patterns))
            .expect_err("cycle");
        assert_eq!(err.kind(), ZmlErrorKind::PatternCycle);
    }

    #[test]
    fn mutually_referential_patterns_are_a_cycle() {
        let patterns = vec![
            ZmlPattern::new("a", "A", pattern_ref("b")).expect("valid"),
            ZmlPattern::new("b", "B", pattern_ref("a")).expect("valid"),
        ];
        let err =
            validate_patterns(&window(vec![pattern_ref("a")]), Some(&patterns)).expect_err("cycle");
        assert_eq!(err.kind(), ZmlErrorKind::PatternCycle);
    }

    #[test]
    fn an_unused_but_cyclic_pattern_is_still_rejected() {
        // The table is validated on its own, not only through references from
        // the root, so a broken definition cannot lie dormant in a document.
        let patterns =
            vec![ZmlPattern::new("orphan", "Orphan", pattern_ref("orphan")).expect("valid")];
        let err = validate_patterns(&window(vec![]), Some(&patterns)).expect_err("cycle");
        assert_eq!(err.kind(), ZmlErrorKind::PatternCycle);
    }

    #[test]
    fn a_pattern_used_twice_is_not_a_cycle() {
        // Two sibling references share an id without nesting through it.
        let patterns = vec![ZmlPattern::new("row", "Row", ZmlNode::default()).expect("valid")];
        validate_patterns(
            &window(vec![pattern_ref("row"), pattern_ref("row")]),
            Some(&patterns),
        )
        .expect("not a cycle");
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let patterns = vec![
            ZmlPattern::new("row", "Row", ZmlNode::default()).expect("valid"),
            ZmlPattern::new("row", "Row again", ZmlNode::default()).expect("valid"),
        ];
        let err = validate_patterns(&window(vec![]), Some(&patterns)).expect_err("duplicate");
        assert_eq!(err.kind(), ZmlErrorKind::DuplicatePatternId);
    }

    #[test]
    fn a_tree_deeper_than_the_cap_is_rejected() {
        let mut node = ZmlNode::default();
        for _ in 0..(MAX_PATTERN_DEPTH + 2) {
            node = ZmlNode {
                component_type: "VStack".to_owned(),
                children: Some(vec![node]),
                ..ZmlNode::default()
            };
        }
        let err = validate_patterns(&node, None).expect_err("too deep");
        assert_eq!(err.kind(), ZmlErrorKind::PatternTooDeep);
    }

    #[test]
    fn unused_patterns_are_listed_not_pruned() {
        let patterns = vec![
            ZmlPattern::new("used", "Used", ZmlNode::default()).expect("valid"),
            ZmlPattern::new("spare", "Spare", ZmlNode::default()).expect("valid"),
        ];
        let unused = unused_pattern_ids(&window(vec![pattern_ref("used")]), Some(&patterns));
        assert_eq!(unused, vec!["spare".to_owned()]);
    }
}
