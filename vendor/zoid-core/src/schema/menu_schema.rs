//! Menu data model types for the `zoid-project.json` schema v2.x.
//!
//! Describes menu structure as plain serde data: menu sections each contain a
//! list of items (separators, headers, or interactive entries with optional
//! action-ids and shortcuts).  No GPUI types appear in this module — it is a
//! pure data crate.
//!
//! Runtime state (dynamic enable/disable, check toggles) is not covered by
//! this static data model.  The values here represent static defaults;
//! callers should override at runtime via entity methods.
//!
//! # JSON shape
//!
//! ```json
//! {
//!   "menus": [
//!     {
//!       "name": "File",
//!       "items": [
//!         { "type": "entry", "label": "New Project", "action_id": "NewProject",
//!           "shortcut": { "key": "n", "control": true } },
//!         { "type": "separator" },
//!         { "type": "entry", "label": "Quit", "action_id": "QuitApp",
//!           "shortcut": { "key": "q", "control": true } }
//!       ]
//!     }
//!   ]
//! }
//! ```

use serde::{Deserialize, Serialize};

use crate::schema::validation::{SchemaValidationError, SchemaValidationResult};

// ── Shortcut ───────────────────────────────────────────────────────────────────

/// A keyboard shortcut assigned to a menu item.
///
/// Stored as a data record — no GPUI `Keystroke` dependency.  Conversion to
/// `Keystroke` is performed by the adapter in `zoid_gpui::menu::menu_schema_adapter`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MenuShortcut {
    /// Key identifier (e.g. `"n"`, `"q"`, `"f1"`).  Must be non-empty.
    pub key: String,

    /// `Ctrl` modifier (or `Cmd` on macOS — `platform` is the cross-platform
    /// meta key).
    #[serde(default)]
    pub control: bool,

    /// `Shift` modifier.
    #[serde(default)]
    pub shift: bool,

    /// `Alt` / `Option` modifier.
    #[serde(default)]
    pub alt: bool,

    /// Platform meta key (`Cmd` on macOS, `Win` on Windows, `Super` on Linux).
    #[serde(default)]
    pub platform: bool,
}

impl MenuShortcut {
    /// Creates a shortcut with only a key (no modifiers).
    pub fn key_only(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            control: false,
            shift: false,
            alt: false,
            platform: false,
        }
    }

    /// Creates a shortcut with a key and the control modifier.
    pub fn ctrl_key(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            control: true,
            shift: false,
            alt: false,
            platform: false,
        }
    }

    /// Creates a shortcut with a key and the platform (Cmd/Win) modifier.
    pub fn platform_key(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            control: false,
            shift: false,
            alt: false,
            platform: true,
        }
    }

    /// Returns `true` if the key is non-empty.
    pub fn is_valid(&self) -> bool {
        !self.key.trim().is_empty()
    }
}

// ── Menu item data ─────────────────────────────────────────────────────────────

/// Static data describing a single interactive menu entry.
///
/// `action_id` is a string identifier (e.g. `"NewProject"`, `"QuitApp"`).
/// Callers provide a `HashMap<String, Box<dyn Action>>` to map these ids to
/// concrete GPUI action types at runtime.  Items whose `action_id` is
/// missing from the map are rendered without a dispatchable action.
///
/// `shortcut` is static display-only metadata — the actual key binding
/// registration lives in the GPUI key context system.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MenuItemEntry {
    /// Visible label displayed in the menu.
    pub label: String,

    /// Optional string action identifier for runtime action lookup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,

    /// Whether the entry is initially enabled.  Defaults to `true`.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Optional initial check state (`None` = no check column, `Some(v)` =
    /// checked/unchecked).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,

    /// Optional keyboard shortcut (display-only metadata).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<MenuShortcut>,
}

fn default_enabled() -> bool {
    true
}

impl MenuItemEntry {
    /// Creates an entry with the given label and no action or shortcut.
    pub fn simple(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            action_id: None,
            enabled: true,
            checked: None,
            shortcut: None,
        }
    }

    /// Validates this entry.
    pub fn validate(&self) -> SchemaValidationResult<()> {
        if self.label.trim().is_empty() {
            return Err(SchemaValidationError::empty_field("menu_item.label"));
        }
        if let Some(ref shortcut) = self.shortcut {
            shortcut.validate()?;
        }
        Ok(())
    }
}

impl Default for MenuItemEntry {
    fn default() -> Self {
        Self {
            label: String::new(),
            action_id: None,
            enabled: true,
            checked: None,
            shortcut: None,
        }
    }
}

// ── Item enum ──────────────────────────────────────────────────────────────────

/// An item in a menu section.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MenuItemSchema {
    /// A horizontal visual separator.
    #[serde(rename = "separator")]
    Separator,

    /// A non-interactive section header.
    #[serde(rename = "header")]
    Header {
        /// Header label text.  Must be non-empty.
        label: String,
    },

    /// An interactive menu entry.
    #[serde(rename = "entry")]
    Entry(MenuItemEntry),
}

impl MenuItemSchema {
    /// Creates a separator item.
    pub fn separator() -> Self {
        Self::Separator
    }

    /// Creates a header item.
    pub fn header(label: impl Into<String>) -> Self {
        Self::Header {
            label: label.into(),
        }
    }

    /// Creates an entry item.
    pub fn entry(entry: MenuItemEntry) -> Self {
        Self::Entry(entry)
    }

    /// Validates this item.
    pub fn validate(&self) -> SchemaValidationResult<()> {
        match self {
            Self::Separator => {}
            Self::Header { label } => {
                if label.trim().is_empty() {
                    return Err(SchemaValidationError::empty_field("menu_item.header.label"));
                }
            }
            Self::Entry(entry) => {
                entry.validate()?;
            }
        }
        Ok(())
    }
}

// ── MenuBar ─────────────────────────────────────────────────────────────────────

/// A menubar definition: a collection of menus tied to a parent window.
///
/// On macOS this becomes the native menu bar; on Windows/Linux it is rendered
/// as an in-window menu bar.  The `icon` field specifies the SVG icon used as
/// the menu title (forced on for tray/menubar apps).  `parent_window` names the
/// window this menubar belongs to (the first menubar without a `parent_window`
/// is the default / primary-window menubar).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MenuBarDef {
    /// Display name for the menubar in the editor tree.
    pub name: String,

    /// Optional SVG icon path used as the menu title (forced for tray/menubar
    /// app types).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,

    /// Optional window name this menubar belongs to.  `None` means the default
    /// / primary-window menubar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_window: Option<String>,

    /// The menus in this menubar.
    #[serde(default)]
    pub menus: Vec<MenuSection>,
}

impl MenuBarDef {
    /// Creates a new menubar with the given name and no icon or parent window.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            icon: None,
            parent_window: None,
            menus: Vec::new(),
        }
    }

    /// Sets the icon for this menubar.
    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Sets the parent window for this menubar.
    #[must_use]
    pub fn with_parent_window(mut self, window: impl Into<String>) -> Self {
        self.parent_window = Some(window.into());
        self
    }

    /// Adds a menu to this menubar.
    #[must_use]
    pub fn with_menu(mut self, menu: MenuSection) -> Self {
        self.menus.push(menu);
        self
    }

    /// Validates this menubar definition.
    pub fn validate(&self) -> SchemaValidationResult<()> {
        if self.name.trim().is_empty() {
            return Err(SchemaValidationError::empty_field("menubar.name"));
        }
        for menu in &self.menus {
            menu.validate()?;
        }
        Ok(())
    }
}

// ── Section ────────────────────────────────────────────────────────────────────

/// A named menu section (e.g. "File", "Edit", "Help").
///
/// Each section maps to one top-level menu.  On macOS this becomes a native
/// `NSMenu`; on Windows/Linux it becomes an `InWindowMenuBar` entry.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MenuSection {
    /// Menu name (e.g. `"File"`, `"Edit"`).  Must be non-empty.
    pub name: String,

    /// Items in this menu.
    #[serde(default)]
    pub items: Vec<MenuItemSchema>,
}

impl MenuSection {
    /// Creates a new menu section with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            items: Vec::new(),
        }
    }

    /// Adds an item to the section.
    #[must_use]
    pub fn with_item(mut self, item: MenuItemSchema) -> Self {
        self.items.push(item);
        self
    }

    /// Validates this section.
    pub fn validate(&self) -> SchemaValidationResult<()> {
        if self.name.trim().is_empty() {
            return Err(SchemaValidationError::empty_field("menu_section.name"));
        }
        for item in &self.items {
            item.validate()?;
        }
        Ok(())
    }
}

// ── Validation on shortcut ────────────────────────────────────────────────────

impl MenuShortcut {
    /// Validates this shortcut.
    pub fn validate(&self) -> SchemaValidationResult<()> {
        if self.key.trim().is_empty() {
            return Err(SchemaValidationError::empty_field("menu_item.shortcut.key"));
        }
        Ok(())
    }
}

// ── Unit tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    // ── round-trip ──────────────────────────────────────────────────────────

    #[test]
    fn round_trip_menu_section_with_all_item_types() {
        let section = MenuSection::new("File")
            .with_item(MenuItemSchema::entry(MenuItemEntry {
                label: "New Project".into(),
                action_id: Some("NewProject".into()),
                enabled: true,
                checked: None,
                shortcut: Some(MenuShortcut::ctrl_key("n")),
            }))
            .with_item(MenuItemSchema::separator())
            .with_item(MenuItemSchema::header("Recent"))
            .with_item(MenuItemSchema::entry(MenuItemEntry {
                label: "Open".into(),
                action_id: Some("OpenProject".into()),
                enabled: true,
                checked: None,
                shortcut: None,
            }))
            .with_item(MenuItemSchema::entry(MenuItemEntry {
                label: "Quit".into(),
                action_id: Some("QuitApp".into()),
                enabled: true,
                checked: None,
                shortcut: Some(MenuShortcut::platform_key("q")),
            }));

        let json = serde_json::to_string_pretty(&section).expect("serialize");
        let parsed: MenuSection = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(section, parsed);
    }

    #[test]
    fn round_trip_minimal_entry_no_optional_fields() {
        let entry = MenuItemEntry::simple("Save");
        let json = serde_json::to_string(&entry).expect("serialize");
        let parsed: MenuItemEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(entry, parsed);
        // Optional fields should not appear in JSON.
        assert!(!json.contains("action_id"));
        assert!(!json.contains("shortcut"));
        assert!(!json.contains("checked"));
    }

    #[test]
    fn round_trip_entry_with_checked_and_disabled() {
        let entry = MenuItemEntry {
            label: "Toggle".into(),
            action_id: None,
            enabled: false,
            checked: Some(true),
            shortcut: None,
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        let parsed: MenuItemEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(entry, parsed);
    }

    // ── validation ──────────────────────────────────────────────────────────

    #[test]
    fn validate_empty_section_name_fails() {
        let section = MenuSection::new("");
        assert!(section.validate().is_err());
    }

    #[test]
    fn validate_whitespace_only_section_name_fails() {
        let section = MenuSection::new("   ");
        assert!(section.validate().is_err());
    }

    #[test]
    fn validate_empty_entry_label_fails() {
        let section =
            MenuSection::new("File").with_item(MenuItemSchema::entry(MenuItemEntry::simple("")));
        assert!(section.validate().is_err());
    }

    #[test]
    fn validate_empty_header_label_fails() {
        let section = MenuSection::new("File").with_item(MenuItemSchema::header(""));
        assert!(section.validate().is_err());
    }

    #[test]
    fn validate_empty_shortcut_key_fails() {
        let entry = MenuItemEntry {
            label: "Item".into(),
            action_id: None,
            enabled: true,
            checked: None,
            shortcut: Some(MenuShortcut::key_only("")),
        };
        assert!(entry.validate().is_err());
    }

    #[test]
    fn validate_valid_data_passes() {
        let section = MenuSection::new("File")
            .with_item(MenuItemSchema::entry(MenuItemEntry::simple("New")))
            .with_item(MenuItemSchema::separator())
            .with_item(MenuItemSchema::header("Section"))
            .with_item(MenuItemSchema::entry(MenuItemEntry {
                label: "Quit".into(),
                action_id: Some("QuitApp".into()),
                enabled: true,
                checked: None,
                shortcut: Some(MenuShortcut::ctrl_key("q")),
            }));
        assert!(section.validate().is_ok());
    }

    // ── shortcut ────────────────────────────────────────────────────────────

    #[test]
    fn shortcut_ctrl_key_is_valid() {
        let s = MenuShortcut::ctrl_key("n");
        assert!(s.is_valid());
        assert!(s.control);
        assert!(!s.shift);
        assert!(!s.alt);
        assert!(!s.platform);
    }

    #[test]
    fn shortcut_platform_key_is_valid() {
        let s = MenuShortcut::platform_key("q");
        assert!(s.is_valid());
        assert!(s.platform);
        assert!(!s.control);
    }

    #[test]
    fn shortcut_empty_key_is_invalid() {
        let s = MenuShortcut::key_only("");
        assert!(!s.is_valid());
    }

    #[test]
    fn shortcut_default_is_invalid() {
        let s = MenuShortcut::default();
        assert!(!s.is_valid());
    }

    // ── serde default enforcement ───────────────────────────────────────────

    #[test]
    fn deserialize_entry_without_enabled_defaults_to_true() {
        let json = r#"{"type":"entry","label":"Test"}"#;
        let item: MenuItemSchema = serde_json::from_str(json).expect("parse");
        match item {
            MenuItemSchema::Entry(e) => assert!(e.enabled),
            other => panic!("expected entry, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_section_without_items_defaults_to_empty() {
        let json = r#"{"name":"File"}"#;
        let section: MenuSection = serde_json::from_str(json).expect("parse");
        assert!(section.items.is_empty());
    }

    #[test]
    fn deserialize_full_item_type_variants() {
        let json = r#"[
            {"type":"separator"},
            {"type":"header","label":"Recent"},
            {"type":"entry","label":"Open","action_id":"OpenProject"}
        ]"#;
        let items: Vec<MenuItemSchema> = serde_json::from_str(json).expect("parse");
        assert_eq!(items.len(), 3);
        assert!(matches!(items[0], MenuItemSchema::Separator));
        assert!(matches!(items[1], MenuItemSchema::Header { ref label } if label == "Recent"));
        assert!(
            matches!(items[2], MenuItemSchema::Entry(ref e) if e.label == "Open" && e.action_id.as_deref() == Some("OpenProject"))
        );
    }

    // ── builder API ─────────────────────────────────────────────────────────

    #[test]
    fn menu_entry_simple_has_correct_defaults() {
        let e = MenuItemEntry::simple("Save");
        assert!(e.enabled);
        assert!(e.action_id.is_none());
        assert!(e.checked.is_none());
        assert!(e.shortcut.is_none());
    }

    #[test]
    fn menu_section_builders_produce_expected_items() {
        let section = MenuSection::new("Edit")
            .with_item(MenuItemSchema::separator())
            .with_item(MenuItemSchema::entry(MenuItemEntry::simple("Undo")));
        assert_eq!(section.items.len(), 2);
    }

    // ── MenuBarDef tests ─────────────────────────────────────────────────────

    #[test]
    fn menubar_new_has_defaults() {
        let mb = MenuBarDef::new("Main");
        assert_eq!(mb.name, "Main");
        assert!(mb.icon.is_none());
        assert!(mb.parent_window.is_none());
        assert!(mb.menus.is_empty());
    }

    #[test]
    fn menubar_with_icon_sets_icon() {
        let mb = MenuBarDef::new("Tray").with_icon("icons/apple-mac.svg");
        assert_eq!(mb.icon.as_deref(), Some("icons/apple-mac.svg"));
    }

    #[test]
    fn menubar_with_parent_window_sets_window() {
        let mb = MenuBarDef::new("Settings").with_parent_window("SettingsWindow");
        assert_eq!(mb.parent_window.as_deref(), Some("SettingsWindow"));
    }

    #[test]
    fn menubar_with_menu_adds_menu() {
        let mb = MenuBarDef::new("Main")
            .with_menu(MenuSection::new("File"))
            .with_menu(MenuSection::new("Edit"));
        assert_eq!(mb.menus.len(), 2);
    }

    #[test]
    fn menubar_empty_name_fails_validation() {
        let mb = MenuBarDef::new("");
        assert!(mb.validate().is_err());
    }

    #[test]
    fn menubar_valid_passes_validation() {
        let mb = MenuBarDef::new("Main")
            .with_icon("icons/apple-mac.svg")
            .with_menu(
                MenuSection::new("File")
                    .with_item(MenuItemSchema::entry(MenuItemEntry::simple("Quit"))),
            );
        assert!(mb.validate().is_ok());
    }
}
