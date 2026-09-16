//! Select / Dropdown component for generated GPUI starters.
//!
//! [`Select<T>`] is a GPUI entity (`View`) that renders a labelled trigger
//! control with a floating options dropdown.  It supports:
//!
//! - An options list (`Vec<SelectOption<T>>`).
//! - A selected value (`Option<T>`).
//! - A placeholder shown when nothing is selected.
//! - An `on_change` callback invoked on every new selection.
//! - Keyboard navigation: Arrow Down / Up moves the highlight; Enter confirms;
//!   Escape closes without changing the selection.
//!
//! # Usage
//!
//! ```rust,ignore
//! use zoid_gpui::select::{Select, SelectOption};
//!
//! let select = cx.new(|cx| {
//!     Select::<String>::new(cx)
//!         .with_options(vec![
//!             SelectOption::new("alpha".to_owned(), "Alpha"),
//!             SelectOption::new("beta".to_owned(),  "Beta"),
//!         ])
//!         .with_placeholder("Pick a value")
//!         .on_change(|val, _window, _cx| println!("selected: {val}"))
//! });
//!
//! // In a parent view's render:
//! cx.new(|_| ()).into_any() // placeholder — render the entity normally
//! ```
//!
//! # Keyboard bindings
//!
//! Register key bindings at startup via `cx.bind_keys`:
//!
//! ```rust,ignore
//! use zoid_gpui::select::{SelectCancel, SelectConfirm, SelectDown, SelectUp,
//!                         KEY_CONTEXT_SELECT};
//!
//! cx.bind_keys([
//!     gpui::KeyBinding::new("down",   SelectDown,    Some(KEY_CONTEXT_SELECT)),
//!     gpui::KeyBinding::new("up",     SelectUp,      Some(KEY_CONTEXT_SELECT)),
//!     gpui::KeyBinding::new("enter",  SelectConfirm, Some(KEY_CONTEXT_SELECT)),
//!     gpui::KeyBinding::new("escape", SelectCancel,  Some(KEY_CONTEXT_SELECT)),
//! ]);
//! ```
//!
//! # GPUI version
//!
//! Implemented against `gpui 0.2.2`.

use std::rc::Rc;

use gpui::{
    App, Context, DismissEvent, ElementId, EventEmitter, FocusHandle, Focusable, IntoElement,
    Render, SharedString, Window, actions, anchored, deferred, div, prelude::*, px,
};

use crate::ui_tokens::{FocusRingMetrics, RadiusScale, SpacingScale, TypographyScale};
use gpui_kit::component::{Theme, ThemeColor};

// ── key context ───────────────────────────────────────────────────────────────

/// Key context identifier for [`Select`] controls.
///
/// Set on the trigger element with `.key_context(KEY_CONTEXT_SELECT)` so that
/// keyboard navigation actions are scoped to the focused Select and do not
/// interfere with other focusable elements.
pub const KEY_CONTEXT_SELECT: &str = "ZoidSelect";

// ── keyboard actions ──────────────────────────────────────────────────────────

actions!(
    zoid_select,
    [
        /// Move keyboard highlight to the next enabled option (Arrow Down).
        SelectDown,
        /// Move keyboard highlight to the previous enabled option (Arrow Up).
        SelectUp,
        /// Confirm the highlighted option and close the dropdown (Enter).
        SelectConfirm,
        /// Close the dropdown without changing the selection (Escape).
        SelectCancel,
    ]
);

// ── handler type ──────────────────────────────────────────────────────────────

/// Callback type for [`Select`] value changes.
///
/// Called with the newly selected value, a mutable window reference, and a
/// mutable app context reference whenever the user confirms a new selection.
pub type SelectHandler<T> = Rc<dyn Fn(&T, &mut Window, &mut App)>;

// ── SelectOption ──────────────────────────────────────────────────────────────

/// A single option in a [`Select`] dropdown.
#[derive(Clone)]
pub struct SelectOption<T: Clone> {
    /// The value carried by this option.
    pub value: T,
    /// The visible label shown in the trigger and dropdown.
    pub label: SharedString,
    /// Whether this option can be keyboard-selected or clicked.
    pub enabled: bool,
}

impl<T: Clone> SelectOption<T> {
    /// Creates an enabled option with the given value and label.
    pub fn new(value: T, label: impl Into<SharedString>) -> Self {
        Self {
            value,
            label: label.into(),
            enabled: true,
        }
    }

    /// Marks this option as non-interactive (shown dimmed, not selectable).
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

// ── SelectColors ──────────────────────────────────────────────────────────────

/// Colors for the [`Select`] component, resolved from the active theme.
///
/// Exposed so unit tests can verify token binding without constructing a full
/// GPUI render context.
#[derive(Clone, Copy, Debug)]
pub struct SelectColors {
    /// Trigger control background.
    pub trigger_bg: gpui::Hsla,
    /// Trigger control border (idle).
    pub trigger_border: gpui::Hsla,
    /// Trigger selected-value text.
    pub trigger_text: gpui::Hsla,
    /// Trigger placeholder text shown when no value is selected.
    pub trigger_placeholder: gpui::Hsla,
    /// Dropdown panel background.
    pub dropdown_bg: gpui::Hsla,
    /// Dropdown panel border.
    pub dropdown_border: gpui::Hsla,
    /// Standard (non-highlighted) option text.
    pub option_text: gpui::Hsla,
    /// Background of the keyboard-highlighted option.
    pub option_highlighted_bg: gpui::Hsla,
    /// Text of the keyboard-highlighted option.
    pub option_highlighted_text: gpui::Hsla,
    /// Text of disabled options.
    pub option_disabled_text: gpui::Hsla,
    /// Focus ring / open-state border color.
    pub focus_ring: gpui::Hsla,
}

impl SelectColors {
    /// Resolves select colors from the given token set.
    #[must_use]
    pub fn resolve(theme: &Theme) -> Self {
        Self {
            trigger_bg: theme.secondary,
            trigger_border: theme.border,
            trigger_text: theme.foreground,
            trigger_placeholder: theme.muted_foreground,
            dropdown_bg: theme.secondary,
            dropdown_border: theme.border,
            option_text: theme.foreground,
            option_highlighted_bg: theme.accent.opacity(0.18),
            option_highlighted_text: theme.accent,
            option_disabled_text: theme.muted_foreground,
            focus_ring: FocusRingMetrics::dark().color,
        }
    }
}

// ── Select entity ─────────────────────────────────────────────────────────────

/// Generic dropdown select component.
///
/// Create via `cx.new(|cx| Select::new(cx))` and configure with builder
/// setters.  Place the entity in the element tree; the trigger button and
/// floating dropdown are rendered inline.
///
/// Emits [`gpui::DismissEvent`] whenever the dropdown closes — on selection,
/// Escape, or programmatic close via [`Select::close_dropdown`].
pub struct Select<T: Clone + PartialEq + 'static> {
    /// Stable element id for the trigger.
    id: ElementId,
    /// Options list.
    options: Vec<SelectOption<T>>,
    /// Currently confirmed selection.
    selected: Option<T>,
    /// Placeholder shown when `selected` is `None`.
    placeholder: Option<SharedString>,
    /// Callback invoked on each new selection.
    on_change: Option<SelectHandler<T>>,
    /// Whether the dropdown is currently open.
    open: bool,
    /// Keyboard-highlighted option index when the dropdown is open.
    highlighted_index: Option<usize>,
    /// When `true`, the control is non-interactive.
    disabled: bool,
    /// When `true`, use the dark token palette; `false` uses light.
    dark: bool,
    /// Focus handle for keyboard routing and focus management.
    focus_handle: FocusHandle,
}

impl<T: Clone + PartialEq + 'static> Select<T> {
    /// Creates a Select with defaults: dark palette, empty options list, no
    /// placeholder, no `on_change` handler.
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            id: ElementId::Name("zoid-select".into()),
            options: Vec::new(),
            selected: None,
            placeholder: None,
            on_change: None,
            open: false,
            highlighted_index: None,
            disabled: false,
            dark: true,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Sets the element id.
    ///
    /// Required when multiple `Select` instances coexist in the element tree.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// Replaces the full options list.
    #[must_use]
    pub fn with_options(mut self, options: Vec<SelectOption<T>>) -> Self {
        self.options = options;
        self
    }

    /// Sets the currently selected value.
    ///
    /// Pass `None` to clear the selection and show the placeholder.
    #[must_use]
    pub fn with_selected(mut self, value: Option<T>) -> Self {
        self.selected = value;
        self
    }

    /// Sets the placeholder text shown when no value is selected.
    #[must_use]
    pub fn with_placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Registers a callback invoked when the user confirms a new selection.
    #[must_use]
    pub fn on_change(mut self, handler: impl Fn(&T, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Marks the select as disabled (non-interactive; keyboard events ignored).
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Selects the dark (`true`) or light (`false`) token palette.
    #[must_use]
    pub fn dark(mut self, dark: bool) -> Self {
        self.dark = dark;
        self
    }

    /// Updates the palette in-place without consuming `self`.
    ///
    /// Useful when an owning `View` (e.g. a gallery) needs to sync the palette
    /// after construction.  Call `cx.notify()` on the entity after this to
    /// trigger a re-render.
    pub fn set_dark(&mut self, dark: bool) {
        self.dark = dark;
    }

    // ── read accessors ────────────────────────────────────────────────────────

    /// Returns a reference to the currently selected value, if any.
    pub fn selected(&self) -> Option<&T> {
        self.selected.as_ref()
    }

    /// Sets the selected value programmatically.
    ///
    /// This does not invoke the `on_change` callback.  Use it for
    /// initial setup or controlled-component patterns where the caller
    /// manages selection externally.
    /// Sets the selected value and notifies subscribers.
    ///
    /// This does not invoke the `on_change` callback.  Use it for
    /// initial setup or controlled-component patterns where the caller
    /// manages selection externally.
    pub fn set_selected(&mut self, value: Option<T>, cx: &mut Context<Self>) {
        self.selected = value;
        cx.notify();
    }

    /// Returns `true` if the dropdown is currently open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Returns the currently keyboard-highlighted option index, if any.
    pub fn highlighted_index(&self) -> Option<usize> {
        self.highlighted_index
    }

    /// Returns a slice of all options.
    pub fn options(&self) -> &[SelectOption<T>] {
        &self.options
    }

    /// Returns `true` if an `on_change` handler has been registered.
    #[must_use]
    pub fn has_on_change(&self) -> bool {
        self.on_change.is_some()
    }

    // ── state mutations ───────────────────────────────────────────────────────

    /// Opens the dropdown.
    ///
    /// Pre-highlights the currently selected option when possible.
    /// Does nothing when the select is disabled.
    pub fn open_dropdown(&mut self) {
        if self.disabled {
            return;
        }
        self.highlighted_index = self.selected.as_ref().and_then(|sel| {
            self.options
                .iter()
                .position(|o| o.enabled && &o.value == sel)
        });
        self.open = true;
    }

    /// Closes the dropdown and emits [`gpui::DismissEvent`].
    ///
    /// Does not change the current selection.
    pub fn close_dropdown(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        self.highlighted_index = None;
        cx.notify();
        cx.emit(DismissEvent);
    }

    /// Returns the value at `index` if the option exists and is enabled.
    fn option_value_at(&self, index: usize) -> Option<T> {
        self.options
            .get(index)
            .filter(|o| o.enabled)
            .map(|o| o.value.clone())
    }

    /// Confirms the highlighted option: updates `selected`, invokes
    /// `on_change`, then closes the dropdown.
    ///
    /// When no option is highlighted, only closes the dropdown.
    pub fn confirm_highlighted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(idx) = self.highlighted_index else {
            self.close_dropdown(cx);
            return;
        };
        // Clone the value first to release the borrow on `self.options`
        // before mutating `self.selected`.
        let maybe_value: Option<T> = self.option_value_at(idx);

        if let Some(value) = maybe_value {
            self.selected = Some(value.clone());
            if let Some(handler) = self.on_change.clone() {
                handler(&value, window, &mut *cx);
            }
        }
        self.close_dropdown(cx);
    }

    /// Moves the keyboard highlight to the next enabled option, wrapping at
    /// the end of the list.
    pub fn move_highlight_next(&mut self) {
        let len = self.options.len();
        if len == 0 {
            return;
        }
        let start = self.highlighted_index.map(|i| i + 1).unwrap_or(0);
        for offset in 0..len {
            let idx = (start + offset) % len;
            if self.options[idx].enabled {
                self.highlighted_index = Some(idx);
                return;
            }
        }
    }

    /// Moves the keyboard highlight to the previous enabled option, wrapping
    /// at the start of the list.
    pub fn move_highlight_previous(&mut self) {
        let len = self.options.len();
        if len == 0 {
            return;
        }
        let start = self
            .highlighted_index
            .map(|i| if i == 0 { len - 1 } else { i - 1 })
            .unwrap_or(len - 1);
        for offset in 0..len {
            let idx = if start >= offset {
                start - offset
            } else {
                len - (offset - start)
            };
            if self.options[idx].enabled {
                self.highlighted_index = Some(idx);
                return;
            }
        }
    }
}

// ── Focusable + EventEmitter ──────────────────────────────────────────────────

impl<T: Clone + PartialEq + 'static> Focusable for Select<T> {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl<T: Clone + PartialEq + 'static> EventEmitter<DismissEvent> for Select<T> {}

// ── Render ────────────────────────────────────────────────────────────────────

impl<T: Clone + PartialEq + 'static> Render for Select<T> {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = if self.dark {
            Theme::from(&*ThemeColor::dark())
        } else {
            Theme::from(&*ThemeColor::light())
        };
        let colors = SelectColors::resolve(&theme);

        let focus = self.focus_handle.clone();
        let is_open = self.open;
        let disabled = self.disabled;

        // ── trigger display text ──────────────────────────────────────────────
        let (trigger_text, is_placeholder): (SharedString, bool) = self
            .selected
            .as_ref()
            .and_then(|sel| {
                self.options
                    .iter()
                    .find(|o| &o.value == sel)
                    .map(|o| (o.label.clone(), false))
            })
            .unwrap_or_else(|| {
                (
                    self.placeholder
                        .clone()
                        .unwrap_or_else(|| SharedString::from("Select…")),
                    true,
                )
            });

        let text_color = if is_placeholder {
            colors.trigger_placeholder
        } else {
            colors.trigger_text
        };

        // When open, the border adopts the focus-ring color.
        let border_color = if is_open {
            colors.focus_ring
        } else {
            colors.trigger_border
        };

        // Capture scalar colors for move-closures below.
        let focus_ring = colors.focus_ring;
        let option_highlight_bg = colors.option_highlighted_bg;

        // ── build trigger element ─────────────────────────────────────────────
        let mut trigger = div()
            .id(self.id.clone())
            .key_context(KEY_CONTEXT_SELECT)
            .track_focus(&focus)
            .on_action(cx.listener(|this, _: &SelectDown, _window, cx| {
                if !this.disabled {
                    if !this.open {
                        this.open_dropdown();
                    }
                    this.move_highlight_next();
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &SelectUp, _window, cx| {
                if !this.disabled {
                    if !this.open {
                        this.open_dropdown();
                    }
                    this.move_highlight_previous();
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &SelectConfirm, window, cx| {
                if !this.disabled {
                    if this.open {
                        this.confirm_highlighted(window, cx);
                    } else {
                        this.open_dropdown();
                        cx.notify();
                    }
                }
            }))
            .on_action(cx.listener(|this, _: &SelectCancel, _window, cx| {
                if this.open {
                    this.close_dropdown(cx);
                }
            }))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap_2()
            .px(px(SpacingScale::default().md))
            .py(px(SpacingScale::default().xs))
            .h(px(32.0))
            .min_w(px(120.0))
            .rounded(px(RadiusScale::default().sm))
            .border_1()
            .border_color(border_color)
            .bg(colors.trigger_bg)
            .text_size(px(TypographyScale::default().md))
            .text_color(text_color)
            .child(div().flex_1().child(trigger_text));

        if !disabled {
            trigger = trigger
                .cursor_pointer()
                .hover(move |s| s.border_color(focus_ring))
                .on_click(cx.listener(|this, _, _window, cx| {
                    if this.open {
                        this.close_dropdown(cx);
                    } else {
                        this.open_dropdown();
                        cx.notify();
                    }
                }));
        }

        // Chevron indicator (▾ when closed, ▴ when open).
        trigger = trigger.child(
            div()
                .text_size(px(10.0))
                .text_color(colors.trigger_placeholder)
                .child(if is_open { "▴" } else { "▾" }),
        );

        // ── collect option display data ───────────────────────────────────────
        // Collected as owned values so that cx.listener closures below do not
        // need to hold a borrow on self.options.
        struct OptionRow {
            index: usize,
            label: SharedString,
            enabled: bool,
            is_selected: bool,
        }

        let highlighted = self.highlighted_index;

        let option_rows: Vec<OptionRow> = self
            .options
            .iter()
            .enumerate()
            .map(|(i, o)| {
                let is_selected = self.selected.as_ref() == Some(&o.value);
                OptionRow {
                    index: i,
                    label: o.label.clone(),
                    enabled: o.enabled,
                    is_selected,
                }
            })
            .collect();

        // ── root container ────────────────────────────────────────────────────
        let mut container = div().relative().child(trigger);

        if !is_open {
            return container;
        }

        // ── dropdown panel ────────────────────────────────────────────────────
        let option_elements: Vec<gpui::AnyElement> = option_rows
            .into_iter()
            .map(|row| {
                let is_highlighted = highlighted == Some(row.index);
                let (bg, row_text_color) = if !row.enabled {
                    (colors.dropdown_bg, colors.option_disabled_text)
                } else if is_highlighted {
                    (colors.option_highlighted_bg, colors.option_highlighted_text)
                } else {
                    (colors.dropdown_bg, colors.option_text)
                };

                let mut option_div = div()
                    .id(ElementId::Integer(row.index as u64))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .px(px(SpacingScale::default().md))
                    .py(px(SpacingScale::default().xs))
                    .rounded_sm()
                    .bg(bg)
                    .text_color(row_text_color)
                    .text_size(px(TypographyScale::default().md));

                if row.enabled {
                    let idx = row.index;
                    let focus_clone = focus.clone();
                    option_div = option_div
                        .cursor_pointer()
                        .hover(move |s| s.bg(option_highlight_bg))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            // Clone value before mutating self to avoid borrow conflict.
                            let maybe_value: Option<T> = this.option_value_at(idx);
                            if let Some(value) = maybe_value {
                                this.selected = Some(value.clone());
                                if let Some(handler) = this.on_change.clone() {
                                    handler(&value, window, &mut *cx);
                                }
                            }
                            this.open = false;
                            this.highlighted_index = None;
                            window.focus(&focus_clone, cx);
                            cx.notify();
                            cx.emit(DismissEvent);
                        }));
                }

                // Checkmark column — keeps all labels left-aligned.
                option_div = option_div.child(
                    div()
                        .w(px(14.0))
                        .flex_shrink_0()
                        .text_color(colors.option_highlighted_text)
                        .child(if row.is_selected { "✓" } else { " " }),
                );

                option_div.child(row.label).into_any_element()
            })
            .collect();

        let dropdown = div()
            .occlude()
            .min_w(px(160.0))
            .rounded_md()
            .border_1()
            .border_color(colors.dropdown_border)
            .bg(colors.dropdown_bg)
            .shadow_lg()
            .py_1()
            .children(option_elements);

        container = container.child(
            deferred(
                anchored()
                    .snap_to_window_with_margin(px(8.0))
                    .child(dropdown),
            )
            .with_priority(1),
        );

        container
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_tokens::color_to_hex;
    use gpui_kit::component::{Theme, ThemeColor};

    fn sample_options() -> Vec<SelectOption<&'static str>> {
        vec![
            SelectOption::new("a", "Alpha"),
            SelectOption::new("b", "Beta"),
            SelectOption::new("c", "Gamma"),
            SelectOption::new("d", "Delta").disabled(),
        ]
    }

    // ── SelectColors token tests ──────────────────────────────────────────────

    #[test]
    fn select_colors_resolve_dark() {
        let theme = Theme::from(&*ThemeColor::dark());
        let colors = SelectColors::resolve(&theme);
        assert_eq!(
            color_to_hex(colors.trigger_bg),
            color_to_hex(theme.secondary)
        );
        assert_eq!(
            color_to_hex(colors.trigger_border),
            color_to_hex(theme.border)
        );
        assert_eq!(
            color_to_hex(colors.focus_ring),
            color_to_hex(FocusRingMetrics::dark().color)
        );
    }

    #[test]
    fn select_colors_resolve_light() {
        let theme = Theme::from(&*ThemeColor::light());
        let colors = SelectColors::resolve(&theme);
        assert_eq!(
            color_to_hex(colors.trigger_bg),
            color_to_hex(theme.secondary)
        );
    }

    #[test]
    fn select_colors_differ_between_light_and_dark() {
        let light = SelectColors::resolve(&Theme::from(&*ThemeColor::light()));
        let dark = SelectColors::resolve(&Theme::from(&*ThemeColor::dark()));
        assert_ne!(
            color_to_hex(light.trigger_bg),
            color_to_hex(dark.trigger_bg),
            "trigger_bg must differ between light and dark"
        );
    }

    // ── SelectOption tests ────────────────────────────────────────────────────

    #[test]
    fn select_option_defaults_enabled() {
        let opt = SelectOption::new("x", "X label");
        assert!(opt.enabled);
        assert_eq!(opt.value, "x");
        assert_eq!(opt.label.as_ref(), "X label");
    }

    #[test]
    fn select_option_disabled_builder() {
        let opt = SelectOption::<&str>::new("x", "X").disabled();
        assert!(!opt.enabled);
    }

    // ── KEY_CONTEXT_SELECT ────────────────────────────────────────────────────

    #[test]
    fn key_context_select_is_non_empty() {
        assert!(!KEY_CONTEXT_SELECT.is_empty());
    }

    // ── Highlight navigation (pure logic) ─────────────────────────────────────
    //
    // These tests use a raw Vec<SelectOption> to exercise the navigation logic
    // without a GPUI context.  The same logic is exercised through the entity
    // in the integration tests in tests/select_feature.rs.

    fn dummy_nav_state(options: &[SelectOption<&'static str>]) -> (Vec<bool>, usize) {
        let enabled: Vec<bool> = options.iter().map(|o| o.enabled).collect();
        (enabled, options.len())
    }

    fn move_next(highlighted: &mut Option<usize>, enabled: &[bool]) {
        let len = enabled.len();
        if len == 0 {
            return;
        }
        let start = highlighted.map(|i| i + 1).unwrap_or(0);
        for offset in 0..len {
            let idx = (start + offset) % len;
            if enabled[idx] {
                *highlighted = Some(idx);
                return;
            }
        }
    }

    fn move_prev(highlighted: &mut Option<usize>, enabled: &[bool]) {
        let len = enabled.len();
        if len == 0 {
            return;
        }
        let start = highlighted
            .map(|i| if i == 0 { len - 1 } else { i - 1 })
            .unwrap_or(len - 1);
        for offset in 0..len {
            let idx = if start >= offset {
                start - offset
            } else {
                len - (offset - start)
            };
            if enabled[idx] {
                *highlighted = Some(idx);
                return;
            }
        }
    }

    #[test]
    fn highlight_next_wraps_at_end() {
        let opts = sample_options();
        let (enabled, _) = dummy_nav_state(&opts);
        let mut h: Option<usize> = None;
        // From none → 0 (Alpha)
        move_next(&mut h, &enabled);
        assert_eq!(h, Some(0));
        // 0 → 1 (Beta)
        move_next(&mut h, &enabled);
        assert_eq!(h, Some(1));
        // 1 → 2 (Gamma)
        move_next(&mut h, &enabled);
        assert_eq!(h, Some(2));
        // 2 → wraps past disabled (3) back to 0 (Alpha)
        move_next(&mut h, &enabled);
        assert_eq!(h, Some(0), "must wrap past disabled option to first");
    }

    #[test]
    fn highlight_previous_wraps_at_start() {
        let opts = sample_options();
        let (enabled, _) = dummy_nav_state(&opts);
        let mut h: Option<usize> = Some(0); // at Alpha
        // 0 → wraps past disabled (3) to 2 (Gamma)
        move_prev(&mut h, &enabled);
        assert_eq!(h, Some(2), "must wrap to last enabled option");
    }

    #[test]
    fn highlight_next_skips_disabled() {
        let opts = sample_options();
        let (enabled, _) = dummy_nav_state(&opts);
        let mut h: Option<usize> = Some(2); // at Gamma
        // Gamma (2) → next enabled is Alpha (0); Delta (3) is disabled
        move_next(&mut h, &enabled);
        assert_eq!(h, Some(0), "disabled option must be skipped");
    }

    #[test]
    fn highlight_on_empty_options_is_noop() {
        let enabled: Vec<bool> = vec![];
        let mut h: Option<usize> = None;
        move_next(&mut h, &enabled);
        assert!(h.is_none());
        move_prev(&mut h, &enabled);
        assert!(h.is_none());
    }
}
