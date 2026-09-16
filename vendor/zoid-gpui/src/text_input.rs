//! Reusable interactive text input view for GPUI.
//!
//! [`TextInput`] is a focusable `View` that owns its editing state and renders
//! an inline cursor bar. It handles the full US-ASCII keyboard including shift
//! mappings for symbols so callers never need to duplicate keystroke logic.
//!
//! ## Dirty flag
//!
//! Every keystroke sets `dirty = true`. While dirty, external calls to
//! [`set_text`](TextInput::set_text) are silently ignored so that a parent
//! observer pushing model state cannot wipe an in-progress edit. Call
//! [`reset_dirty`](TextInput::reset_dirty) when you want to allow the next
//! `set_text` through (e.g., on focus loss or after a successful submit).

use gpui::{
    App, Context, FocusHandle, Focusable, IntoElement, KeyDownEvent, ParentElement, Render,
    SharedString, Window, div, prelude::*, px,
};

use crate::forms::TextFieldState;
use crate::ui_tokens::{RadiusScale, SpacingScale, TypographyScale};
use gpui_kit::component::ThemeColor;

/// Stable feature id for the interactive text input component.
pub const UI_KIT_TEXT_INPUT_FEATURE_ID: &str = "ui-kit-text-input";

/// An interactive text input widget for GPUI apps.
///
/// `TextInput` wraps a [`TextFieldState`] with focus management, cursor
/// rendering, and keyboard event handling. It supports full US-ASCII
/// shift-symbol mappings so users can type `:`, `_`, `!`, `@`, etc.
///
/// ## Shift symbol mapping (US-ASCII layout)
///
/// | Unshifted | Shifted |
/// |-----------|---------|
/// | `1`–`0`   | `!@#$%^&*()` |
/// | `-`       | `_` |
/// | `=`       | `+` |
/// | `[`       | `{` |
/// | `]`       | `}` |
/// | `\`       | `\|` |
/// | `;`       | `:` |
/// | `'`       | `"` |
/// | `,`       | `<` |
/// | `.`       | `>` |
/// | `/`       | `?` |
/// | `` ` ``   | `~` |
///
/// ## Usage
///
/// ```rust,ignore
/// let input = cx.new(|cx| {
///     TextInput::new(cx).with_placeholder("Enter a value…")
/// });
/// // Read the current text:
/// let value = input.read(cx).text().to_owned();
/// // Programmatically set text (no-op while dirty):
/// input.update(cx, |v, cx| v.set_text("preset", cx));
/// ```
pub struct TextInput {
    field: TextFieldState,
    focus_handle: FocusHandle,
    placeholder: SharedString,
    dirty: bool,
}

impl TextInput {
    /// Creates a new empty `TextInput` and acquires a focus handle from `cx`.
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            field: TextFieldState::new(),
            focus_handle: cx.focus_handle(),
            placeholder: SharedString::default(),
            dirty: false,
        }
    }

    /// Sets the placeholder text shown when the field is empty and unfocused.
    #[must_use]
    pub fn with_placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Returns the current text content.
    #[must_use]
    pub fn text(&self) -> &str {
        self.field.text()
    }

    /// Programmatically sets the field text, placing the cursor at the end.
    ///
    /// This is a no-op when `dirty` is `true` (the user has typed since the
    /// last programmatic update). Call [`reset_dirty`](Self::reset_dirty) first
    /// if you need to force an overwrite.
    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        if self.dirty {
            return;
        }
        self.field = TextFieldState::with_text(text);
        cx.notify();
    }

    /// Clears the dirty flag, re-enabling the next [`set_text`](Self::set_text) call.
    pub fn reset_dirty(&mut self) {
        self.dirty = false;
    }

    /// Returns `true` if the field has unsaved local edits (typed since last
    /// programmatic update).
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Handles a single decoded keystroke.
    ///
    /// Called from the `on_key_down` handler. Exposed as `pub(crate)` so that
    /// unit tests in this module can drive the input without constructing a
    /// full `KeyDownEvent`.
    pub(crate) fn apply_keystroke(
        &mut self,
        key: &str,
        shift: bool,
        ctrl: bool,
        cx: &mut Context<Self>,
    ) {
        // Whether this keystroke edited the text (vs. just moving the caret or
        // changing the selection). Only edits set the dirty flag.
        let mut edited = true;
        match key {
            "backspace" => self.field.backspace(),
            "delete" => self.field.delete(),
            "left" => {
                edited = false;
                if shift {
                    self.field.select_left();
                } else {
                    self.field.move_left();
                }
            }
            "right" => {
                edited = false;
                if shift {
                    self.field.select_right();
                } else {
                    self.field.move_right();
                }
            }
            "space" => self.field.insert_text(" "),
            // Ctrl/Cmd+A selects all without inserting a character.
            "a" if ctrl => {
                edited = false;
                self.field.select_all();
            }
            // enter/return are not consumed here — let parent bindings handle them.
            "enter" | "return" => return,
            // Other ctrl/cmd chords (copy/paste/etc.) are not handled here; let
            // them fall through rather than inserting a literal letter.
            _ if ctrl => return,
            k if k.chars().count() == 1 => {
                let Some(ch) = k.chars().next() else { return };
                let s = if shift { shift_char(ch) } else { k.to_owned() };
                self.field.insert_text(&s);
            }
            _ => return,
        }
        if edited {
            self.dirty = true;
        }
        cx.notify();
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = ThemeColor::default();
        let is_focused = self.focus_handle.is_focused(window);
        let sel = self.field.selected_range();
        let text = self.field.text();
        let before = SharedString::from(text[..sel.start].to_owned());
        let selected = SharedString::from(text[sel.start..sel.end].to_owned());
        let after = SharedString::from(text[sel.end..].to_owned());
        let has_selection = sel.start != sel.end;
        let is_empty = text.is_empty();
        let placeholder = self.placeholder.clone();
        let focus_on_click = self.focus_handle.clone();
        let tracked_focus = self.focus_handle.clone();

        // Extract token colours used inside closures before moving `theme`.
        let text_muted = theme.muted_foreground;
        let border_color = if is_focused {
            theme.accent
        } else {
            theme.border
        };

        div()
            .id("text-input")
            .flex()
            .flex_row()
            .items_center()
            .h(px(32.0))
            .w_full()
            .px(px(SpacingScale::default().sm))
            .rounded(px(RadiusScale::default().md))
            .border_1()
            .border_color(border_color)
            .bg(theme.secondary)
            .text_color(theme.foreground)
            .text_size(px(TypographyScale::default().md))
            .cursor_text()
            .track_focus(&tracked_focus)
            .on_click(move |_event, window, cx| {
                window.focus(&focus_on_click, cx);
            })
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                let mods = event.keystroke.modifiers;
                let shift = mods.shift;
                // Treat Ctrl (Win/Linux) and Cmd (macOS) the same for chords.
                let ctrl = mods.control || mods.platform;
                this.apply_keystroke(key, shift, ctrl, cx);
            }))
            .when(is_empty && !is_focused, |el| {
                el.child(div().text_color(text_muted).child(placeholder))
            })
            .when(!is_empty || is_focused, |el| {
                el.child(div().child(before))
                    // Highlighted selection span (a translucent blue block behind
                    // the selected text). Shown whenever a non-empty selection
                    // exists; otherwise a thin caret bar marks the cursor.
                    .when(has_selection, |el| {
                        el.child(
                            div()
                                .bg(theme.accent.opacity(0.3))
                                .text_color(theme.foreground)
                                .child(selected.clone()),
                        )
                    })
                    .when(!has_selection && is_focused, |el| {
                        // 1.5 px blue cursor bar rendered at the caret.
                        el.child(
                            div()
                                .w(px(1.5))
                                .h(px(14.0))
                                .bg(theme.accent)
                                .flex_shrink_0(),
                        )
                    })
                    .child(div().child(after))
            })
    }
}

// ── US-ASCII shift mapping ────────────────────────────────────────────────────

/// Maps a character to its shifted equivalent on a standard US-ASCII keyboard.
///
/// Lowercase letters are uppercased. Symbol keys are mapped per the US
/// QWERTY layout. Any other character is returned unchanged.
fn shift_char(ch: char) -> String {
    match ch {
        'a'..='z' => ch.to_uppercase().to_string(),
        '1' => "!".to_owned(),
        '2' => "@".to_owned(),
        '3' => "#".to_owned(),
        '4' => "$".to_owned(),
        '5' => "%".to_owned(),
        '6' => "^".to_owned(),
        '7' => "&".to_owned(),
        '8' => "*".to_owned(),
        '9' => "(".to_owned(),
        '0' => ")".to_owned(),
        '-' => "_".to_owned(),
        '=' => "+".to_owned(),
        '[' => "{".to_owned(),
        ']' => "}".to_owned(),
        '\\' => "|".to_owned(),
        ';' => ":".to_owned(),
        '\'' => "\"".to_owned(),
        ',' => "<".to_owned(),
        '.' => ">".to_owned(),
        '/' => "?".to_owned(),
        '`' => "~".to_owned(),
        _ => ch.to_string(),
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    // Helper: type a sequence of characters into the input view.
    fn type_str(view: &gpui::Entity<TextInput>, s: &str, cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            view.update(cx, |v, cx| {
                for ch in s.chars() {
                    v.apply_keystroke(&ch.to_string(), false, false, cx);
                }
            });
        });
    }

    // Helper: send a named key (e.g. "backspace", "left") with optional shift.
    fn send_key(
        view: &gpui::Entity<TextInput>,
        key: &str,
        shift: bool,
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(|cx| {
            view.update(cx, |v, cx| {
                v.apply_keystroke(key, shift, false, cx);
            });
        });
    }

    // Helper: send a key with the ctrl modifier (e.g. ctrl+a).
    fn send_ctrl(view: &gpui::Entity<TextInput>, key: &str, cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            view.update(cx, |v, cx| {
                v.apply_keystroke(key, false, true, cx);
            });
        });
    }

    #[gpui::test]
    fn text_input_typing_hello(cx: &mut gpui::TestAppContext) {
        let view = cx.update(|cx| cx.new(TextInput::new));
        type_str(&view, "hello", cx);
        view.read_with(cx, |v, _| {
            assert_eq!(v.text(), "hello");
        });
    }

    #[gpui::test]
    fn text_input_backspace(cx: &mut gpui::TestAppContext) {
        let view = cx.update(|cx| cx.new(TextInput::new));
        type_str(&view, "hello", cx);
        send_key(&view, "backspace", false, cx);
        view.read_with(cx, |v, _| {
            assert_eq!(v.text(), "hell");
        });
    }

    #[gpui::test]
    fn text_input_cursor_move_and_insert(cx: &mut gpui::TestAppContext) {
        // Type "hell", move left (cursor before last 'l'), type 'p' → "helpl".
        let view = cx.update(|cx| cx.new(TextInput::new));
        type_str(&view, "hell", cx);
        send_key(&view, "left", false, cx);
        type_str(&view, "p", cx);
        view.read_with(cx, |v, _| {
            assert_eq!(v.text(), "helpl");
        });
    }

    #[gpui::test]
    fn text_input_shift_uppercase(cx: &mut gpui::TestAppContext) {
        let view = cx.update(|cx| cx.new(TextInput::new));
        send_key(&view, "a", true, cx);
        view.read_with(cx, |v, _| {
            assert_eq!(v.text(), "A");
        });
    }

    #[gpui::test]
    fn text_input_shift_symbols(cx: &mut gpui::TestAppContext) {
        let view = cx.update(|cx| cx.new(TextInput::new));
        // ; → :,  - → _,  1 → !
        send_key(&view, ";", true, cx);
        send_key(&view, "-", true, cx);
        send_key(&view, "1", true, cx);
        view.read_with(cx, |v, _| {
            assert_eq!(v.text(), ":_!");
        });
    }

    #[gpui::test]
    fn text_input_dirty_flag_blocks_set_text(cx: &mut gpui::TestAppContext) {
        let view = cx.update(|cx| cx.new(TextInput::new));
        cx.update(|cx| {
            view.update(cx, |v, cx| {
                // Type one character — this sets dirty = true.
                v.apply_keystroke("h", false, false, cx);
                // External set_text must be a no-op while dirty.
                v.set_text("replaced", cx);
            });
        });
        view.read_with(cx, |v, _| {
            assert_eq!(v.text(), "h", "set_text must not overwrite while dirty");
        });
    }

    #[gpui::test]
    fn text_input_set_text_when_clean(cx: &mut gpui::TestAppContext) {
        let view = cx.update(|cx| cx.new(TextInput::new));
        cx.update(|cx| {
            view.update(cx, |v, cx| v.set_text("preset", cx));
        });
        view.read_with(cx, |v, _| {
            assert_eq!(v.text(), "preset");
        });
    }

    #[gpui::test]
    fn text_input_reset_dirty_re_enables_set_text(cx: &mut gpui::TestAppContext) {
        let view = cx.update(|cx| cx.new(TextInput::new));
        type_str(&view, "draft", cx);
        cx.update(|cx| {
            view.update(cx, |v, cx| {
                v.reset_dirty();
                v.set_text("updated", cx);
            });
        });
        view.read_with(cx, |v, _| {
            assert_eq!(v.text(), "updated");
        });
    }

    #[gpui::test]
    fn text_input_placeholder_is_set(cx: &mut gpui::TestAppContext) {
        let view = cx.update(|cx| cx.new(|cx| TextInput::new(cx).with_placeholder("Enter name…")));
        view.read_with(cx, |v, _| {
            assert_eq!(v.text(), "");
            assert!(!v.is_dirty());
        });
    }

    #[gpui::test]
    fn text_input_delete_key(cx: &mut gpui::TestAppContext) {
        let view = cx.update(|cx| cx.new(TextInput::new));
        type_str(&view, "hello", cx);
        // Move to start, delete first char.
        for _ in 0..5 {
            send_key(&view, "left", false, cx);
        }
        send_key(&view, "delete", false, cx);
        view.read_with(cx, |v, _| {
            assert_eq!(v.text(), "ello");
        });
    }

    #[gpui::test]
    fn shift_arrow_extends_selection_and_typing_replaces(cx: &mut gpui::TestAppContext) {
        let view = cx.update(|cx| cx.new(TextInput::new));
        type_str(&view, "hello", cx);
        // Select the last two chars with shift+left twice.
        send_key(&view, "left", true, cx);
        send_key(&view, "left", true, cx);
        view.read_with(cx, |v, _| {
            assert_eq!(
                v.field.selected_range(),
                3..5,
                "shift+left must select 'lo'"
            );
        });
        // Typing replaces the selection.
        type_str(&view, "p", cx);
        view.read_with(cx, |v, _| assert_eq!(v.text(), "help"));
    }

    #[gpui::test]
    fn ctrl_a_selects_all_then_backspace_clears(cx: &mut gpui::TestAppContext) {
        let view = cx.update(|cx| cx.new(TextInput::new));
        type_str(&view, "hello", cx);
        send_ctrl(&view, "a", cx);
        view.read_with(cx, |v, _| {
            assert_eq!(v.field.selected_range(), 0..5, "ctrl+a selects all");
        });
        send_key(&view, "backspace", false, cx);
        view.read_with(cx, |v, _| assert_eq!(v.text(), ""));
    }

    #[gpui::test]
    fn move_left_collapses_selection_to_start(cx: &mut gpui::TestAppContext) {
        let view = cx.update(|cx| cx.new(TextInput::new));
        type_str(&view, "hello", cx);
        send_ctrl(&view, "a", cx);
        // Plain left collapses the selection to its start without deleting.
        send_key(&view, "left", false, cx);
        view.read_with(cx, |v, _| {
            assert!(!v.field.has_selection());
            assert_eq!(v.field.cursor(), 0);
            assert_eq!(v.text(), "hello");
        });
    }

    #[test]
    fn shift_char_digits() {
        assert_eq!(shift_char('1'), "!");
        assert_eq!(shift_char('2'), "@");
        assert_eq!(shift_char('9'), "(");
        assert_eq!(shift_char('0'), ")");
    }

    #[test]
    fn shift_char_symbols() {
        assert_eq!(shift_char(';'), ":");
        assert_eq!(shift_char('-'), "_");
        assert_eq!(shift_char('/'), "?");
        assert_eq!(shift_char('`'), "~");
    }

    #[test]
    fn shift_char_letters() {
        assert_eq!(shift_char('a'), "A");
        assert_eq!(shift_char('z'), "Z");
    }
}
