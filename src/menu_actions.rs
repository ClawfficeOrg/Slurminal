use gpui::{actions, App, KeyBinding, Menu, MenuItem as NativeMenuItem};
#[cfg(target_os = "macos")]
use gpui::SystemMenuType;
#[cfg(not(target_os = "macos"))]
use gpui::Window;

/// GPUI actions for the menubar application.
/// Each variant maps to one menu item that users can override or extend.
actions!(
    menubar_app,
    [
        Quit,
        OpenPreferences,
        ToggleTheme,
        NewWindow,
        CloseWindow,
        OpenAppMenu,
        MenuNavLeft,
        MenuNavRight,
    ]
);

/// Installs the application menu and registers all action handlers.
///
/// Call this once at application startup inside the `Application::run` closure
/// before opening any windows.
pub fn install_app_menus(cx: &mut App) {
    cx.on_action(handle_quit);
    cx.bind_keys([
        KeyBinding::new("cmd-,", OpenPreferences, None),
        KeyBinding::new("ctrl-,", OpenPreferences, None),
        KeyBinding::new("cmd-n", NewWindow, None),
        KeyBinding::new("ctrl-n", NewWindow, None),
        KeyBinding::new("cmd-w", CloseWindow, None),
        KeyBinding::new("ctrl-w", CloseWindow, None),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("ctrl-q", Quit, None),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("f10", OpenAppMenu, None),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("left", MenuNavLeft, Some("AppMenuBar")),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("right", MenuNavRight, Some("AppMenuBar")),
    ]);
    #[cfg(target_os = "macos")]
    cx.set_menus(build_menus());
}

/// Returns the full menu tree for the application.
///
/// Adjust the returned `Vec<Menu>` to add or remove top-level menus or items.
pub fn build_menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "Slurminal".into(),
            items: vec![
                #[cfg(target_os = "macos")]
                NativeMenuItem::os_submenu("Services", SystemMenuType::Services),
                #[cfg(target_os = "macos")]
                NativeMenuItem::separator(),
                NativeMenuItem::action("Preferences…", OpenPreferences),
                NativeMenuItem::separator(),
                NativeMenuItem::action("Quit Slurminal", Quit),
            ],
            disabled: false,
        },
        Menu {
            name: "File".into(),
            items: vec![
                NativeMenuItem::action("New Window", NewWindow),
                NativeMenuItem::separator(),
                NativeMenuItem::action("Close Window", CloseWindow),
            ],
            disabled: false,
        },
        Menu {
            name: "View".into(),
            items: vec![NativeMenuItem::action("Toggle Theme", ToggleTheme)],
            disabled: false,
        },
    ]
}

#[cfg(not(target_os = "macos"))]
pub fn build_in_window_menus() -> Vec<(&'static str, Vec<crate::menu_bar::AppMenuItem>)> {
    use crate::menu_bar::AppMenuItem;

    vec![
        (
            "Slurminal",
            vec![
                AppMenuItem::Entry {
                    label: "Preferences…",
                    action: |window: &mut Window, cx: &mut App| {
                        window.dispatch_action(Box::new(OpenPreferences), cx);
                    },
                },
                AppMenuItem::Separator,
                AppMenuItem::Entry {
                    label: "Quit Slurminal",
                    action: |window: &mut Window, cx: &mut App| {
                        window.dispatch_action(Box::new(Quit), cx);
                    },
                },
            ],
        ),
        (
            "File",
            vec![
                AppMenuItem::Entry {
                    label: "New Window",
                    action: |window: &mut Window, cx: &mut App| {
                        window.dispatch_action(Box::new(NewWindow), cx);
                    },
                },
                AppMenuItem::Separator,
                AppMenuItem::Entry {
                    label: "Close Window",
                    action: |window: &mut Window, cx: &mut App| {
                        window.dispatch_action(Box::new(CloseWindow), cx);
                    },
                },
            ],
        ),
        (
            "View",
            vec![AppMenuItem::Entry {
                label: "Toggle Theme",
                action: |window: &mut Window, cx: &mut App| {
                    window.dispatch_action(Box::new(ToggleTheme), cx);
                },
            }],
        ),
    ]
}

/// Quits the application by dispatching to GPUI's quit mechanism.
///
/// Inject a different handler via `cx.on_action` in tests to avoid
/// terminating the test runner.
fn handle_quit(_: &Quit, cx: &mut App) {
    cx.quit();
}
