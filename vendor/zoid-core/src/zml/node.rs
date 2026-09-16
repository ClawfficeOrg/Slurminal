//! ZML component node types.

use serde::{Deserialize, Serialize};

use crate::zml::event_handler::ZmlEventHandler;
use crate::zml::prop::ZmlProp;

/// A single component node in a ZML component tree.
///
/// Nodes are recursive: a `ZmlNode` may contain `children`, each of which is
/// itself a `ZmlNode`.  Trees must be finite and acyclic; depth is bounded
/// in practice by TOML parser stack limits.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ZmlNode {
    /// The component type identifier (e.g. `"Window"`, `"Button"`, `"Card"`).
    pub component_type: String,

    /// Props passed to the component at instantiation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub props: Option<Vec<ZmlProp>>,

    /// Child nodes nested inside this component.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<ZmlNode>>,

    /// Event handlers attached to this node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_handlers: Option<Vec<ZmlEventHandler>>,

    /// Names the slot this node should be injected into when used as a slot
    /// filler in a parent component.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot_name: Option<String>,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::zml::prop::ZmlValue;

    fn roundtrip(node: &ZmlNode) -> ZmlNode {
        let s = toml::to_string(node).expect("serialize");
        toml::from_str(&s).expect("deserialize")
    }

    #[test]
    fn leaf_node_roundtrips() {
        let node = ZmlNode {
            component_type: "Text".to_owned(),
            props: Some(vec![
                ZmlProp::new("content", ZmlValue::String("Hello".to_owned())).expect("valid"),
            ]),
            children: None,
            event_handlers: None,
            slot_name: None,
        };
        assert_eq!(roundtrip(&node), node);
    }

    #[test]
    fn three_level_tree_roundtrips() {
        let leaf = ZmlNode {
            component_type: "Icon".to_owned(),
            props: Some(vec![
                ZmlProp::new("name", ZmlValue::String("star".to_owned())).expect("valid"),
            ]),
            children: None,
            event_handlers: None,
            slot_name: None,
        };
        let middle = ZmlNode {
            component_type: "Button".to_owned(),
            props: Some(vec![
                ZmlProp::new("disabled", ZmlValue::Bool(false)).expect("valid"),
            ]),
            children: Some(vec![leaf]),
            event_handlers: None,
            slot_name: None,
        };
        let root = ZmlNode {
            component_type: "Card".to_owned(),
            props: None,
            children: Some(vec![middle]),
            event_handlers: None,
            slot_name: None,
        };
        assert_eq!(roundtrip(&root), root);
    }

    #[test]
    fn node_with_two_children_roundtrips() {
        let child_a = ZmlNode {
            component_type: "Label".to_owned(),
            props: Some(vec![
                ZmlProp::new("text", ZmlValue::String("A".to_owned())).expect("valid"),
            ]),
            children: None,
            event_handlers: None,
            slot_name: None,
        };
        let child_b = ZmlNode {
            component_type: "Label".to_owned(),
            props: Some(vec![
                ZmlProp::new("text", ZmlValue::String("B".to_owned())).expect("valid"),
            ]),
            children: None,
            event_handlers: None,
            slot_name: None,
        };
        let parent = ZmlNode {
            component_type: "Row".to_owned(),
            props: None,
            children: Some(vec![child_a, child_b]),
            event_handlers: None,
            slot_name: None,
        };
        assert_eq!(roundtrip(&parent), parent);
    }
}
