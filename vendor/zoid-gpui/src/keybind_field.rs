//! `KeybindField` component for generated GPUI starters.
//!
//! [`KeybindField`] is a focusable `View` that captures keyboard combinations.
//! When focused, it listens for the next key event and records the full
//! keystroke (modifiers + key).  The captured binding is rendered as a
//! pill-shaped label (e.g. `⌘K` on macOS or `Ctrl+K` on Windows/Linux).
//! Pressing **Escape** while focused clears the captured binding.
//!
//! ## Capture rules
//!
//! - **Modifier-only presses** (Shift, Ctrl, Alt, Cmd/Win/Super, Fn) are
//!   ignored — a real key must be pressed to complete the capture.
//! - **Escape** clears the captured binding and fires `on_change` with a
//!   default (empty) `Keystroke`.
//! - Any other key (optionally combined with one or more modifiers) is
//!   captured immediately, and `on_change` fires with the new [`Keystroke`].
//!
//! ## Pill display
//!
//! The component delegates formatting to GPUI's `Display` implementation on
//! [`Keystroke`], which is already platform-aware:
//!
//! | Platform | Example output |
//! |----------|---------------|
//! | macOS    | `⌘K`, `⇧⌃P`   |
//! | Windows  | `ctrl-k`, `shift-ctrl-p` |
//! | Linux    | `ctrl-k`, `shift-ctrl-p` |
//!
//! ## Headless-test helpers
//!
//! Because `on_change` requires `&mut Window`, full callback invocation
//! requires a windowed context.  Use [`KeybindField::apply_keystroke_silent`]
//! to drive state transitions in headless [`gpui::TestAppContext`] tests.
//!
//! ## Example
//!
//! ```rust,ignore
//! let field = cx.new(|cx| {
//!     KeybindField::new(cx)
//!         .with_value(Keystroke::parse("cmd-k").unwrap())
//!         .on_change(|ks, _window, _cx| println!("new binding: {ks}"))
//!         .dark(true)
//! });
//! ```
//!
//! ## Theming
//!
//! Colors resolve from the active `gpui_kit::component::Theme` via
//! [`KeybindFieldColors::resolve`]; the focus ring uses `theme.ring`.

use std::rc::Rc;

use gpui::{
    App, Context, FocusHandle, Focusable, IntoElement, KeyDownEvent, Keystroke, Render,
    SharedString, Subscription, Window, div, prelude::*, px,
};

use crate::ui_tokens::{RadiusScale, SpacingScale, TypographyScale};
use gpui_kit::component::{ActiveTheme, Theme};

// ── handler type ──────────────────────────────────────────────────────────────

/// Callback type for [`KeybindField`] binding changes.
///
/// Invoked with the newly captured [`Keystroke`] (or `Keystroke::default()`
/// when the binding is cleared), a mutable window reference, and a mutable
/// app context.
pub type KeybindFieldHandler = Rc<dyn Fn(Keystroke, &mut Window, &mut App)>;

// ── KeybindFieldColors ────────────────────────────────────────────────────────

/// Token-resolved colors for a [`KeybindField`] component.
///
/// Exposed so unit tests can verify token binding without constructing a full
/// GPUI render context.
#[derive(Clone, Copy, Debug)]
pub struct KeybindFieldColors {
    /// Background of the field container at rest.
    pub field_bg: gpui::Hsla,
    /// Border color at rest.
    pub field_border: gpui::Hsla,
    /// Border color when focused.
    pub field_border_focus: gpui::Hsla,
    /// Placeholder text color (shown when no binding is captured).
    pub placeholder: gpui::Hsla,
    /// Pill background.
    pub pill_bg: gpui::Hsla,
    /// Pill text color.
    pub pill_text: gpui::Hsla,
    /// Pill border color.
    pub pill_border: gpui::Hsla,
}

impl KeybindFieldColors {
    /// Resolves [`KeybindField`] colors from the given token set.
    #[must_use]
    pub fn resolve(theme: &Theme) -> Self {
        Self {
            field_bg: theme.secondary,
            field_border: theme.border,
            field_border_focus: theme.ring,
            placeholder: theme.muted_foreground,
            pill_bg: theme.secondary,
            pill_text: theme.foreground,
            pill_border: theme.border,
        }
    }
}

// ── KeybindField ──────────────────────────────────────────────────────────────

/// A key-binding capture field.
///
/// Click or tab-focus the field, then press any modifier+key combination to
/// record a binding.  The binding is displayed as a pill label.  Press Escape
/// to clear.
///
/// # Example
///
/// ```rust,ignore
/// let field = cx.new(|cx| {
///     KeybindField::new(cx)
///         .with_value(Keystroke::parse("cmd-k").unwrap())
///         .on_change(|ks, _window, _cx| println!("binding changed: {ks}"))
/// });
/// ```
pub struct KeybindField {
    /// The currently captured key binding, if any.
    captured: Option<Keystroke>,
    /// Whether to use dark-mode styling.
    dark: bool,
    /// Focus handle for keyboard input.
    focus_handle: FocusHandle,
    /// Optional callback invoked when the binding changes (capture or clear).
    on_change: Option<KeybindFieldHandler>,
    /// Blur subscription; kept alive until the entity is dropped.
    _blur_sub: Option<Subscription>,
}

impl KeybindField {
    /// Creates a new `KeybindField` with no captured binding and dark styling.
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        Self {
            captured: None,
            dark: true,
            focus_handle,
            on_change: None,
            _blur_sub: None,
        }
    }

    // ── builder setters ───────────────────────────────────────────────────────

    /// Sets an initial captured binding.
    ///
    /// A binding with an empty `key` is treated as "no binding" and displays
    /// the placeholder instead of a pill.
    #[must_use]
    pub fn with_value(mut self, ks: Keystroke) -> Self {
        self.captured = if ks.key.is_empty() { None } else { Some(ks) };
        self
    }

    /// Selects dark (`true`) or light (`false`) styling.
    #[must_use]
    pub fn dark(mut self, dark: bool) -> Self {
        self.dark = dark;
        self
    }

    /// Registers a callback invoked whenever the captured binding changes.
    ///
    /// The callback receives the new [`Keystroke`] (or `Keystroke::default()`
    /// when cleared), a mutable [`Window`] reference, and a mutable [`App`]
    /// context.
    #[must_use]
    pub fn on_change(
        mut self,
        handler: impl Fn(Keystroke, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    // ── query methods ─────────────────────────────────────────────────────────

    /// Returns the current captured binding, or `None` if cleared.
    #[must_use]
    pub fn captured(&self) -> Option<&Keystroke> {
        self.captured.as_ref()
    }

    /// Returns `true` if a binding is currently captured.
    #[must_use]
    pub fn has_binding(&self) -> bool {
        self.captured.is_some()
    }

    /// Returns `true` if an `on_change` callback has been registered.
    #[must_use]
    pub fn has_on_change(&self) -> bool {
        self.on_change.is_some()
    }

    /// Returns `true` if dark-mode styling is active.
    #[must_use]
    pub fn is_dark(&self) -> bool {
        self.dark
    }

    // ── update methods ────────────────────────────────────────────────────────

    /// Switches between dark and light styling and schedules a re-render.
    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        self.dark = dark;
        cx.notify();
    }

    /// Clears the captured binding and schedules a re-render.
    ///
    /// Does NOT fire `on_change`.  Exposed so integration tests can reset
    /// state without a window context.
    pub fn clear_silent(&mut self, cx: &mut Context<Self>) {
        self.captured = None;
        cx.notify();
    }

    /// Clears the captured binding and fires `on_change` with the default
    /// (empty) keystroke.
    ///
    /// Requires `&mut Window` because `on_change` callbacks receive one.
    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.captured = None;
        cx.notify();
        self.fire_on_change(Keystroke::default(), window, cx);
    }

    /// Captures `ks` as the new binding and schedules a re-render.
    ///
    /// Does NOT fire `on_change`.  Exposed as `pub` for headless integration
    /// tests that cannot provide `&mut Window`.
    pub fn apply_keystroke_silent(&mut self, ks: Keystroke, cx: &mut Context<Self>) {
        self.captured = Some(ks);
        cx.notify();
    }

    // ── internal helpers ──────────────────────────────────────────────────────

    /// Processes a raw `KeyDownEvent` from GPUI:
    ///
    /// - Modifier-only presses are ignored.
    /// - Escape clears the binding and fires `on_change(Keystroke::default())`.
    /// - Any other key captures the full keystroke and fires `on_change`.
    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();

        if is_modifier_key(key) {
            // Modifier-only press: do nothing until a real key arrives.
            return;
        }

        if key == "escape" {
            self.clear(window, cx);
            return;
        }

        let ks = event.keystroke.clone();
        self.captured = Some(ks.clone());
        cx.notify();
        self.fire_on_change(ks, window, cx);
    }

    /// Clones and invokes the `on_change` handler with `ks`.
    fn fire_on_change(&self, ks: Keystroke, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(handler) = self.on_change.clone() {
            handler(ks, window, cx);
        }
    }
}

impl Focusable for KeybindField {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for KeybindField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Lazily initialise the blur subscription on the first render, where
        // `&mut Window` is available (it is not available in `new()`).
        if self._blur_sub.is_none() {
            let fh = self.focus_handle.clone();
            self._blur_sub = Some(cx.on_blur(&fh, window, |_this: &mut Self, _window, _cx| {
                // No commit action needed on blur for a keybind field.
            }));
        }

        let colors = KeybindFieldColors::resolve(cx.theme());
        let is_focused = self.focus_handle.is_focused(window);

        let border_color = if is_focused {
            colors.field_border_focus
        } else {
            colors.field_border
        };

        let focus_on_click = self.focus_handle.clone();
        let tracked_focus = self.focus_handle.clone();

        div()
            .id("keybind-field")
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .h(px(28.0))
            .min_w(px(80.0))
            .px(px(SpacingScale::default().sm))
            .rounded(px(RadiusScale::default().sm))
            .border_1()
            .border_color(border_color)
            .bg(colors.field_bg)
            .cursor_pointer()
            .track_focus(&tracked_focus)
            .on_click(move |_event, window, cx| {
                window.focus(&focus_on_click, cx);
            })
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            .child(self.render_content(&colors))
    }
}

impl KeybindField {
    /// Renders the pill (if a binding is captured) or the placeholder label.
    fn render_content(&self, colors: &KeybindFieldColors) -> gpui::AnyElement {
        if let Some(ks) = &self.captured {
            let label = format_keystroke_pill(ks);
            div()
                .flex()
                .items_center()
                .px(px(SpacingScale::default().sm))
                .py(px(2.0))
                .rounded(px(RadiusScale::default().sm))
                .bg(colors.pill_bg)
                .border_1()
                .border_color(colors.pill_border)
                .text_color(colors.pill_text)
                .text_size(px(TypographyScale::default().sm))
                .font_weight(gpui::FontWeight::MEDIUM)
                .child(SharedString::from(label))
                .into_any_element()
        } else {
            div()
                .text_color(colors.placeholder)
                .text_size(px(TypographyScale::default().sm))
                .child(SharedString::from("Press a key…"))
                .into_any_element()
        }
    }
}

// ── pure helpers ──────────────────────────────────────────────────────────────

/// Returns `true` if `key` is a modifier key name that should not trigger a
/// capture on its own.
///
/// These key names appear when a modifier key is pressed without any
/// accompanying non-modifier key.
#[must_use]
pub fn is_modifier_key(key: &str) -> bool {
    matches!(
        key,
        "shift" | "control" | "alt" | "platform" | "function" | "caps_lock"
    )
}

/// Formats a [`Keystroke`] as a human-readable pill string.
///
/// Returns an empty string for the default (empty) keystroke.
/// Otherwise delegates to GPUI's `Display` implementation, which is
/// platform-aware (uses Unicode symbols on macOS, text tokens elsewhere).
///
/// # Examples
///
/// ```rust,ignore
/// // macOS
/// let ks = Keystroke::parse("cmd-k").unwrap();
/// assert_eq!(format_keystroke_pill(&ks), "⌘K");
///
/// // Windows / Linux
/// let ks = Keystroke::parse("ctrl-k").unwrap();
/// assert_eq!(format_keystroke_pill(&ks), "ctrl-K");
/// ```
#[must_use]
pub fn format_keystroke_pill(ks: &Keystroke) -> String {
    if ks.key.is_empty() {
        return String::new();
    }
    format!("{ks}")
}
