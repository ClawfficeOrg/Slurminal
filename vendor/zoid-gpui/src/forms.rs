//! Text input state and form primitives for generated GPUI starters.

use std::ops::Range;

use gpui::{
    App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString, Window, div, prelude::*,
    px,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::ui_tokens::{RadiusScale, SpacingScale, TypographyScale};
use gpui_kit::component::ThemeColor;

/// Stable id for the UI kit forms feature.
pub const UI_KIT_FORMS_FEATURE_ID: &str = "ui-kit-forms";

/// Dependency-free text field state for generated forms.
///
/// Selection is modeled as a moving `cursor` (caret) plus a fixed `anchor`.
/// The selected range is `min(cursor, anchor)..max(cursor, anchor)`; the two
/// are equal when there is no selection. Shift-extension moves `cursor` while
/// leaving `anchor` in place, which is what lets selection grow *and* shrink.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TextFieldState {
    text: String,
    cursor: usize,
    anchor: usize,
}

impl TextFieldState {
    /// Creates an empty text field state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates text field state with the cursor at the end.
    #[must_use]
    pub fn with_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.len();
        Self {
            text,
            cursor,
            anchor: cursor,
        }
    }

    /// Returns the current text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the selected byte range (`start <= end`).
    #[must_use]
    pub fn selected_range(&self) -> Range<usize> {
        self.cursor.min(self.anchor)..self.cursor.max(self.anchor)
    }

    /// Returns the caret (moving end of the selection) byte offset.
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Returns `true` when there is a non-empty selection.
    #[must_use]
    pub fn has_selection(&self) -> bool {
        self.cursor != self.anchor
    }

    /// Moves the cursor one grapheme left, collapsing any selection to its
    /// start.
    pub fn move_left(&mut self) {
        if self.has_selection() {
            self.set_caret(self.selected_range().start);
        } else {
            self.set_caret(previous_grapheme_boundary(&self.text, self.cursor));
        }
    }

    /// Moves the cursor one grapheme right, collapsing any selection to its
    /// end.
    pub fn move_right(&mut self) {
        if self.has_selection() {
            self.set_caret(self.selected_range().end);
        } else {
            self.set_caret(next_grapheme_boundary(&self.text, self.cursor));
        }
    }

    /// Extends the selection one grapheme to the left (shift+left).
    pub fn select_left(&mut self) {
        self.cursor = previous_grapheme_boundary(&self.text, self.cursor);
    }

    /// Extends the selection one grapheme to the right (shift+right).
    pub fn select_right(&mut self) {
        self.cursor = next_grapheme_boundary(&self.text, self.cursor);
    }

    /// Selects the full text, leaving the caret at the end.
    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.cursor = self.text.len();
    }

    /// Places the caret at `pos` (clamped to a grapheme boundary), clearing any
    /// selection. Used for a mouse click / programmatic cursor placement.
    pub fn set_caret(&mut self, pos: usize) {
        let pos = clamp_to_boundary(&self.text, pos);
        self.cursor = pos;
        self.anchor = pos;
    }

    /// Extends the selection so the caret moves to `pos` (clamped), keeping the
    /// anchor fixed. Used for shift-click / mouse drag.
    pub fn select_to(&mut self, pos: usize) {
        self.cursor = clamp_to_boundary(&self.text, pos);
    }

    /// Replaces the current selection with text.
    pub fn insert_text(&mut self, text: &str) {
        self.replace_selection(text);
    }

    /// Deletes the previous grapheme cluster or current selection.
    pub fn backspace(&mut self) {
        if !self.has_selection() {
            let start = previous_grapheme_boundary(&self.text, self.cursor);
            self.anchor = start;
        }
        self.replace_selection("");
    }

    /// Deletes the next grapheme cluster or current selection.
    pub fn delete(&mut self) {
        if !self.has_selection() {
            let end = next_grapheme_boundary(&self.text, self.cursor);
            self.anchor = end;
        }
        self.replace_selection("");
    }

    fn replace_selection(&mut self, replacement: &str) {
        let range = self.selected_range();
        self.text.replace_range(range.clone(), replacement);
        let cursor = range.start + replacement.len();
        self.cursor = cursor;
        self.anchor = cursor;
    }
}

/// Clamps `pos` to the nearest valid grapheme boundary within `text`.
fn clamp_to_boundary(text: &str, pos: usize) -> usize {
    let pos = pos.min(text.len());
    if text.is_char_boundary(pos) {
        pos
    } else {
        previous_grapheme_boundary(text, pos)
    }
}

/// Visual text field shell for generated forms.
#[derive(IntoElement)]
pub struct TextField {
    id: ElementId,
    text: SharedString,
    placeholder: SharedString,
    disabled: bool,
}

impl TextField {
    /// Creates a visual text field from current text and placeholder.
    pub fn new(
        id: impl Into<ElementId>,
        text: impl Into<SharedString>,
        placeholder: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            placeholder: placeholder.into(),
            disabled: false,
        }
    }

    /// Marks the field disabled.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for TextField {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = ThemeColor::default();
        let is_empty = self.text.is_empty();
        let content = if is_empty {
            self.placeholder
        } else {
            self.text
        };

        div()
            .id(self.id)
            .flex()
            .items_center()
            .h(px(32.0))
            .w_full()
            .px(px(SpacingScale::default().sm))
            .rounded(px(RadiusScale::default().md))
            .border_1()
            .border_color(theme.border)
            .bg(theme.secondary)
            .text_color(if is_empty {
                theme.muted_foreground
            } else {
                theme.foreground
            })
            .text_size(px(TypographyScale::default().md))
            .when(!self.disabled, |this| {
                this.cursor_text()
                    .hover(move |style| style.border_color(theme.accent))
            })
            .when(self.disabled, |this| this.opacity(0.55))
            .child(content)
    }
}

/// Label, help, and error wrapper for form controls.
#[derive(IntoElement)]
pub struct FormField {
    id: ElementId,
    label: SharedString,
    help: Option<SharedString>,
    error: Option<SharedString>,
    child: gpui::AnyElement,
}

impl FormField {
    /// Creates a form field wrapper.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        child: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            help: None,
            error: None,
            child: child.into_any_element(),
        }
    }

    /// Adds helper text.
    #[must_use]
    pub fn help(mut self, help: impl Into<SharedString>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Adds an error message.
    #[must_use]
    pub fn error(mut self, error: impl Into<SharedString>) -> Self {
        self.error = Some(error.into());
        self
    }
}

impl RenderOnce for FormField {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = ThemeColor::default();

        div()
            .id(self.id)
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_size(px(TypographyScale::default().sm))
                    .text_color(theme.foreground)
                    .child(self.label),
            )
            .child(self.child)
            .when_some(self.help, |this, help| {
                this.child(
                    div()
                        .text_size(px(TypographyScale::default().sm))
                        .text_color(theme.muted_foreground)
                        .child(help),
                )
            })
            .when_some(self.error, |this, error| {
                this.child(
                    div()
                        .text_size(px(TypographyScale::default().sm))
                        .text_color(theme.danger)
                        .child(error),
                )
            })
    }
}

fn previous_grapheme_boundary(text: &str, cursor: usize) -> usize {
    text.grapheme_indices(true)
        .rev()
        .find_map(|(idx, _)| (idx < cursor).then_some(idx))
        .unwrap_or(0)
}

fn next_grapheme_boundary(text: &str, cursor: usize) -> usize {
    text.grapheme_indices(true)
        .find_map(|(idx, _)| (idx > cursor).then_some(idx))
        .unwrap_or(text.len())
}
