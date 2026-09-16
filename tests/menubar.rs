#[path = "../src/menu_actions.rs"]
mod menu_actions;

#[path = "../src/menu_bar.rs"]
mod menu_bar;

#[path = "../src/settings.rs"]
mod settings;

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::MenuItem;
use menu_actions::{
    CloseWindow, MenuNavLeft, MenuNavRight, NewWindow, OpenAppMenu, OpenPreferences, Quit,
    ToggleTheme,
};
use gpui_kit::component::ThemeMode;
use settings::{AppSettings, AppearanceSettings, SettingsError};

// ── Menu action tests ──────────────────────────────────────────────────────────

#[test]
fn build_menus_contains_expected_top_level_menus() {
    let menus = menu_actions::build_menus();

    // App menu + File + View at minimum
    assert!(menus.len() >= 3, "expected at least 3 menus, got {}", menus.len());
    assert_eq!(menus[0].name.to_string(), "Slurminal");
    assert_eq!(menus[1].name.to_string(), "File");
    assert_eq!(menus[2].name.to_string(), "View");
}

#[test]
fn app_menu_contains_quit_action() {
    let menus = menu_actions::build_menus();
    let app_menu = &menus[0];

    let has_quit = app_menu.items.iter().any(|item| {
        matches!(item, MenuItem::Action { .. })
    });
    assert!(has_quit, "app menu must contain at least one Action item (Quit)");
}

#[test]
fn file_menu_contains_new_window_and_close() {
    let menus = menu_actions::build_menus();
    let file_menu = &menus[1];

    let action_count = file_menu
        .items
        .iter()
        .filter(|item| matches!(item, MenuItem::Action { .. }))
        .count();
    assert!(action_count >= 2, "File menu needs NewWindow and CloseWindow actions");
}

#[test]
fn view_menu_contains_toggle_theme() {
    let menus = menu_actions::build_menus();
    let view_menu = &menus[2];

    let action_count = view_menu
        .items
        .iter()
        .filter(|item| matches!(item, MenuItem::Action { .. }))
        .count();
    assert!(action_count >= 1, "View menu needs ToggleTheme action");
}

#[cfg(not(target_os = "macos"))]
#[test]
fn in_window_menu_entries_match_top_level_menus() {
    let entries = menu_actions::build_in_window_menus();
    let names: Vec<&str> = entries.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, ["Slurminal", "File", "View"]);
    assert_eq!(entries[0].1.len(), 3);
    assert_eq!(entries[1].1.len(), 3);
    assert_eq!(entries[2].1.len(), 1);
}

// ── Action type smoke tests ────────────────────────────────────────────────────

#[test]
fn action_types_are_debug() {
    // GPUI `actions!`-generated structs derive Debug but not Copy.
    fn assert_debug<T: std::fmt::Debug>(_: &T) {}
    assert_debug(&Quit);
    assert_debug(&OpenPreferences);
    assert_debug(&ToggleTheme);
    assert_debug(&NewWindow);
    assert_debug(&CloseWindow);
    assert_debug(&OpenAppMenu);
    assert_debug(&MenuNavLeft);
    assert_debug(&MenuNavRight);
}

// ── Theme tests ────────────────────────────────────────────────────────────────
//
// The palette itself belongs to gpui-component and is exercised upstream; what
// this app owns is the mapping between its settings file and `ThemeMode`. The
// live switch is covered by the `#[gpui::test]` cases in `main.rs`, which need
// an `App` to read `cx.theme()`.

#[test]
fn theme_mode_serialises_to_its_settings_spelling() {
    assert_eq!(ThemeMode::Light.name(), "light");
    assert_eq!(ThemeMode::Dark.name(), "dark");
}

#[test]
fn theme_mode_reports_darkness() {
    assert!(ThemeMode::Dark.is_dark());
    assert!(!ThemeMode::Light.is_dark());
}

// ── Settings tests ─────────────────────────────────────────────────────────────

#[test]
fn default_settings_are_valid() {
    let settings = AppSettings::default();
    settings.validate().expect("default settings should validate");
    assert_eq!(settings.appearance.theme, ThemeMode::Dark);
}

#[test]
fn parses_and_serialises_settings() {
    let source = "schema_version=1\ntheme=light\n";
    let settings = AppSettings::parse(source).expect("parse settings");
    assert_eq!(settings.appearance.theme, ThemeMode::Light);
    assert_eq!(settings.to_file_string(), source);
}

#[test]
fn round_trips_settings_to_disk() {
    let root = unique_temp_dir("menubar-settings-round-trip");
    fs::create_dir_all(&root).expect("create temp root");
    let path = root.join("settings.toml");

    let settings = AppSettings {
        appearance: AppearanceSettings {
            theme: ThemeMode::Light,
        },
        ..AppSettings::default()
    };

    settings.save_to_file(&path).expect("save settings");
    let loaded = AppSettings::load_from_file(&path).expect("load settings");
    assert_eq!(loaded, settings);

    fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn missing_settings_file_returns_default() {
    let path = unique_temp_dir("menubar-missing-settings").join("settings.toml");
    let settings = AppSettings::load_from_file(&path).expect("missing file should return default");
    assert_eq!(settings, AppSettings::default());
}

#[test]
fn rejects_parent_traversal_path() {
    let error = AppSettings::default()
        .save_to_file("../settings.toml")
        .expect_err("path traversal should be rejected");
    assert!(matches!(error, SettingsError::UnsafePath(_)));
}

#[test]
fn rejects_unknown_settings_keys() {
    let error = AppSettings::parse("unknown_key=value\n").expect_err("unknown key should fail");
    assert!(error.to_string().contains("unknown settings key"));
}

#[test]
fn rejects_duplicate_settings_keys() {
    let error =
        AppSettings::parse("theme=dark\ntheme=light\n").expect_err("duplicate key should fail");
    assert!(error.to_string().contains("duplicate settings key"));
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("zoid-{label}-{unique}"))
}
