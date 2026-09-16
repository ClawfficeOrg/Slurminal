//! `BulletList` and `NumberedList` components for generated GPUI starters.
//!
//! Both components are [`RenderOnce`] and accept `Vec<AnyElement>` items so
//! callers can supply any GPUI element as a list item — plain text labels,
//! icons, complex rows, or recursively nested lists.
//!
//! ## Nesting
//!
//! Nesting is achieved by passing a `BulletList` or `NumberedList` (converted
//! via `.into_any_element()`) as one of the parent list's items.  Because each
//! list reserves a fixed-width gutter for its glyph / number label, nested
//! lists are automatically indented by the sum of all ancestor gutter widths,
//! producing the expected cascading visual indent without any special handling:
//!
//! ```text
//! • Item A
//! • Item B
//!   • Nested B1
//!   • Nested B2
//! • Item C
//! ```
//!
//! ## Theming
//!
//! Colors are resolved from the active theme via [`ListColors`].  Pass
//! `.dark(false)` to switch to the light palette.
//!
//! ## Numbering
//!
//! `NumberedList` starts numbering from `start` (default `1`).  The label
//! format depends on [`NumberingStyle`]: decimal (`"1."`, `"2."`), lowercase
//! alpha (`"a."`, `"b."`), or lowercase roman numerals (`"i."`, `"ii."`).
//! Default is [`NumberingStyle::Decimal`].
//!
//! Inner nested `NumberedList` instances always restart their own counter from
//! their own `start` value (default `1`) — there is no implicit counter
//! inheritance across levels.  This matches standard HTML `<ol>` semantics.  If
//! continuous cross-level numbering is required, callers can set `start`
//! explicitly.
//!
//! # GPUI version
//!
//! Implemented against `gpui 0.2.2`.

use gpui::{
    AnyElement, App, ElementId, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, px,
};

use crate::ui_tokens::TypographyScale;
use gpui_kit::component::{Theme, ThemeColor};

// ── ListColors ────────────────────────────────────────────────────────────────

/// Resolved color tokens for [`BulletList`] and [`NumberedList`].
///
/// Exposed so unit tests can verify token binding without constructing a full
/// GPUI render context.
#[derive(Clone, Copy, Debug)]
pub struct ListColors {
    /// Color of the bullet glyph or number label.
    pub glyph: gpui::Hsla,
    /// Primary text color applied to item content.
    pub text: gpui::Hsla,
}

impl ListColors {
    /// Resolves list colors from the given `tokens`.
    ///
    /// - `glyph` maps to `colors.text_muted` for subtle but readable markers.
    /// - `text` maps to `colors.text` for primary content.
    #[must_use]
    pub fn resolve(theme: &Theme) -> Self {
        Self {
            glyph: theme.muted_foreground,
            text: theme.foreground,
        }
    }
}

// ── BulletList ────────────────────────────────────────────────────────────────

/// An unordered list that renders each item prefixed by a bullet glyph.
///
/// Items are supplied as [`AnyElement`] so any GPUI element — plain text,
/// icon rows, or a nested [`BulletList`] / [`NumberedList`] — can appear as a
/// list entry.
///
/// # Layout
///
/// Each row is a horizontal flex container:
///
/// ```text
/// ┌────────────────────────────────────┐
/// │ [glyph] [item content …]           │
/// │ [glyph] [item with nested list]    │
/// │           [glyph] [nested item 1]  │
/// │           [glyph] [nested item 2]  │
/// └────────────────────────────────────┘
/// ```
///
/// The glyph column is `indent_px` wide (default `24.0 px`) and
/// `flex_shrink_0` so item content never collapses the marker.
///
/// # Example
///
/// ```rust,ignore
/// BulletList::new("my-list")
///     .items(vec![
///         div().child("First item").into_any_element(),
///         div().child("Second item").into_any_element(),
///     ])
///     .dark(false)
/// ```
#[derive(IntoElement)]
pub struct BulletList {
    id: ElementId,
    items: Vec<AnyElement>,
    dark: bool,
    glyph: SharedString,
    indent_px: f32,
}

impl BulletList {
    /// Creates an empty bullet list with the default bullet glyph (`"•"`),
    /// dark-mode tokens, and a `24 px` gutter.  Add items via
    /// [`BulletList::items`].
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            dark: true,
            glyph: "•".into(),
            indent_px: 24.0,
        }
    }

    /// Replaces the item list with `items`.
    #[must_use]
    pub fn items(mut self, items: Vec<AnyElement>) -> Self {
        self.items = items;
        self
    }

    /// Selects light (`false`) or dark (`true`) token palette.
    #[must_use]
    pub fn dark(mut self, dark: bool) -> Self {
        self.dark = dark;
        self
    }

    /// Overrides the bullet glyph (e.g. `"◦"`, `"▸"`, `"-"`).
    ///
    /// Defaults to `"•"`.
    #[must_use]
    pub fn glyph(mut self, glyph: impl Into<SharedString>) -> Self {
        self.glyph = glyph.into();
        self
    }

    /// Overrides the gutter width in pixels (default `24.0`).
    ///
    /// The gutter holds the bullet glyph; setting a larger value gives the
    /// bullet more space before the item content begins.
    #[must_use]
    pub fn indent_px(mut self, px: f32) -> Self {
        self.indent_px = px;
        self
    }

    /// Returns the number of items in the list.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if this list contains at least one item.
    #[must_use]
    pub fn has_items(&self) -> bool {
        !self.items.is_empty()
    }

    /// Returns `true` if the dark token palette is selected.
    #[must_use]
    pub fn is_dark(&self) -> bool {
        self.dark
    }

    /// Returns the current bullet glyph string.
    #[must_use]
    pub fn current_glyph(&self) -> &str {
        self.glyph.as_ref()
    }

    /// Returns the gutter width in pixels.
    #[must_use]
    pub fn current_indent_px(&self) -> f32 {
        self.indent_px
    }
}

impl RenderOnce for BulletList {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = if self.dark {
            Theme::from(&*ThemeColor::dark())
        } else {
            Theme::from(&*ThemeColor::light())
        };
        let colors = ListColors::resolve(&theme);

        let mut container = div().id(self.id).flex().flex_col();

        for (idx, item) in self.items.into_iter().enumerate() {
            let glyph_id: SharedString = format!("bl-glyph-{idx}").into();
            let row = div()
                .flex()
                .flex_row()
                .items_start()
                .py(px(1.0))
                // Glyph column — fixed width, shrink-protected.
                .child(
                    div()
                        .id(ElementId::Name(glyph_id))
                        .w(px(self.indent_px))
                        .flex_shrink_0()
                        .text_size(px(TypographyScale::default().md))
                        .text_color(colors.glyph)
                        .child(self.glyph.clone()),
                )
                // Item content column — grows to fill remaining width.
                .child(
                    div()
                        .flex_grow(1.0)
                        .text_size(px(TypographyScale::default().md))
                        .text_color(colors.text)
                        .child(item),
                );
            container = container.child(row);
        }

        container
    }
}

// ── NumberingStyle ────────────────────────────────────────────────────────────

/// Numbering style for [`NumberedList`] item labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberingStyle {
    /// Decimal numbering: `1.`, `2.`, `3.`, …
    Decimal,
    /// Lowercase alpha: `a.`, `b.`, `c.`, …
    AlphaLower,
    /// Lowercase roman numerals: `i.`, `ii.`, `iii.`, …
    RomanLower,
}

// ── NumberedList ──────────────────────────────────────────────────────────────

/// An ordered list that renders each item prefixed by an auto-incremented
/// label (`"1."`, `"2."`, …).
///
/// Items are supplied as [`AnyElement`] so any GPUI element — plain text,
/// icon rows, or a nested [`BulletList`] / [`NumberedList`] — can appear as a
/// list entry.
///
/// ## Numbering semantics
///
/// Numbering starts at [`NumberedList::start`] (default `1`) and increments
/// by 1 for each item.  The numbering [`style`](NumberedList::style) controls
/// the label format (decimal, lowercase alpha, or lowercase roman numerals).
///
/// Nested `NumberedList` instances always restart their own counter from their
/// own `start` value; there is no cross-level counter inheritance.
///
/// # Layout
///
/// Each row is a horizontal flex container:
///
/// ```text
/// ┌────────────────────────────────────┐
/// │ [1.] [item content …]              │
/// │ [2.] [item with nested list]       │
/// │         [1.] [nested item 1]       │
/// │         [2.] [nested item 2]       │
/// └────────────────────────────────────┘
/// ```
///
/// # Example
///
/// ```rust,ignore
/// NumberedList::new("steps")
///     .items(vec![
///         div().child("Install dependencies").into_any_element(),
///         div().child("Run the server").into_any_element(),
///         div().child("Open the browser").into_any_element(),
///     ])
///     .dark(false)
/// ```
#[derive(IntoElement)]
pub struct NumberedList {
    id: ElementId,
    items: Vec<AnyElement>,
    dark: bool,
    start: usize,
    indent_px: f32,
    style: NumberingStyle,
}

impl NumberedList {
    /// Creates an empty numbered list starting at `1`, using dark-mode tokens
    /// and a `24 px` gutter.  Add items via [`NumberedList::items`].
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            dark: true,
            start: 1,
            indent_px: 24.0,
            style: NumberingStyle::Decimal,
        }
    }

    /// Replaces the item list with `items`.
    #[must_use]
    pub fn items(mut self, items: Vec<AnyElement>) -> Self {
        self.items = items;
        self
    }

    /// Selects light (`false`) or dark (`true`) token palette.
    #[must_use]
    pub fn dark(mut self, dark: bool) -> Self {
        self.dark = dark;
        self
    }

    /// Sets the starting number for the first item (default `1`).
    ///
    /// Useful when continuing a list across multiple components or sections.
    #[must_use]
    pub fn start(mut self, start: usize) -> Self {
        self.start = start;
        self
    }

    /// Overrides the gutter width in pixels (default `24.0`).
    ///
    /// The gutter holds the number label; wider gutters accommodate larger
    /// numbers (e.g. two-digit item counts).
    #[must_use]
    pub fn indent_px(mut self, px: f32) -> Self {
        self.indent_px = px;
        self
    }

    /// Sets the numbering style for this list (default [`NumberingStyle::Decimal`]).
    #[must_use]
    pub fn style(mut self, style: NumberingStyle) -> Self {
        self.style = style;
        self
    }

    /// Returns the number of items in the list.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if this list contains at least one item.
    #[must_use]
    pub fn has_items(&self) -> bool {
        !self.items.is_empty()
    }

    /// Returns `true` if the dark token palette is selected.
    #[must_use]
    pub fn is_dark(&self) -> bool {
        self.dark
    }

    /// Returns the start index for this list's numbering.
    #[must_use]
    pub fn current_start(&self) -> usize {
        self.start
    }

    /// Returns the gutter width in pixels.
    #[must_use]
    pub fn current_indent_px(&self) -> f32 {
        self.indent_px
    }

    /// Returns the numbering style for this list.
    #[must_use]
    pub fn current_style(&self) -> NumberingStyle {
        self.style
    }

    /// Returns the label string that would be rendered for the item at
    /// position `idx` (zero-based offset from the start).
    ///
    /// The format depends on [`NumberedList::style`]:
    /// - [`NumberingStyle::Decimal`] → `"1."`, `"2."`, … (default)
    /// - [`NumberingStyle::AlphaLower`] → `"a."`, `"b."`, …
    /// - [`NumberingStyle::RomanLower`] → `"i."`, `"ii."`, …
    ///
    /// Primarily useful for testing that label formatting is correct without
    /// constructing a full render context.
    #[must_use]
    pub fn label_at(&self, idx: usize) -> String {
        let n = self.start + idx;
        match self.style {
            NumberingStyle::Decimal => format!("{n}."),
            NumberingStyle::AlphaLower => format!("{}.", number_to_alpha_lower(n)),
            NumberingStyle::RomanLower => format!("{}.", number_to_roman_lower(n)),
        }
    }
}

// ── numbering helpers ─────────────────────────────────────────────────────────

/// Converts `n` (1‑based) to a lowercase alpha label (`a`, `b`, …, `z`, `aa`, …).
fn number_to_alpha_lower(mut n: usize) -> String {
    let mut s = String::new();
    while n > 0 {
        n -= 1;
        s.insert(0, (b'a' + (n % 26) as u8) as char);
        n /= 26;
    }
    s
}

/// Converts `n` (1‑based) to a lowercase roman numeral.
fn number_to_roman_lower(mut n: usize) -> String {
    const PAIRS: &[(usize, &str)] = &[
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut s = String::new();
    for &(val, sym) in PAIRS {
        while n >= val {
            s.push_str(sym);
            n -= val;
        }
    }
    s
}

impl RenderOnce for NumberedList {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = if self.dark {
            Theme::from(&*ThemeColor::dark())
        } else {
            Theme::from(&*ThemeColor::light())
        };
        let colors = ListColors::resolve(&theme);
        let start = self.start;

        let style = self.style;
        let mut container = div().id(self.id).flex().flex_col();

        for (idx, item) in self.items.into_iter().enumerate() {
            let num_id: SharedString = format!("nl-num-{idx}").into();
            let n = start + idx;
            let label: SharedString = match style {
                NumberingStyle::Decimal => format!("{n}.").into(),
                NumberingStyle::AlphaLower => format!("{}.", number_to_alpha_lower(n)).into(),
                NumberingStyle::RomanLower => format!("{}.", number_to_roman_lower(n)).into(),
            };

            let row = div()
                .flex()
                .flex_row()
                .items_start()
                .py(px(1.0))
                // Number label column — fixed width, shrink-protected.
                .child(
                    div()
                        .id(ElementId::Name(num_id))
                        .w(px(self.indent_px))
                        .flex_shrink_0()
                        .text_size(px(TypographyScale::default().md))
                        .text_color(colors.glyph)
                        .child(label),
                )
                // Item content column — grows to fill remaining width.
                .child(
                    div()
                        .flex_grow(1.0)
                        .text_size(px(TypographyScale::default().md))
                        .text_color(colors.text)
                        .child(item),
                );
            container = container.child(row);
        }

        container
    }
}
