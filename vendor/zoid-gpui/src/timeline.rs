//! `Timeline` component for generated GPUI starters.
//!
//! Renders a vertical sequence of [`TimelineItem`] entries with a connecting
//! line and node dots, themed from [`TimelineColors`].
//!
//! Each item displays:
//!
//! - An optional leading icon glyph (emoji or short text).
//! - A muted timestamp label.
//! - A primary title in medium weight.
//! - An optional longer description in muted small text.
//!
//! The connecting line is rendered as a thin colored bar between node dots;
//! no line is drawn above the first item or below the last item.
//!
//! # Example
//!
//! ```rust,ignore
//! Timeline::new("release-timeline")
//!     .items(vec![
//!         TimelineItem::new("2024-01-01", "Project start").icon("🚀"),
//!         TimelineItem::new("2024-03-15", "First release")
//!             .description("v0.1.0 shipped to production")
//!             .icon("🎉"),
//!         TimelineItem::new("2024-06-01", "v1.0 GA"),
//!     ])
//!     .dark(false)
//! ```
//!
//! # GPUI version
//!
//! Implemented against `gpui 0.2.2`.

use gpui::{App, ElementId, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, px};

use crate::ui_tokens::TypographyScale;
use gpui_kit::component::{Theme, ThemeColor};

// ── TimelineItem ──────────────────────────────────────────────────────────────

/// A single event entry in a [`Timeline`].
///
/// All fields except `timestamp` and `title` are optional.
#[derive(Clone, Debug)]
pub struct TimelineItem {
    /// Short timestamp label displayed in muted text (e.g. `"2024-01-15"` or
    /// `"3 days ago"`).
    pub timestamp: SharedString,
    /// Primary title for this event, displayed in medium-weight text.
    pub title: SharedString,
    /// Optional longer description rendered below the title in small muted text.
    pub description: Option<SharedString>,
    /// Optional leading icon glyph shown before the timestamp (e.g. `"🚀"`).
    pub icon: Option<SharedString>,
}

impl TimelineItem {
    /// Creates a new item with the given `timestamp` and `title`.
    ///
    /// `description` and `icon` default to `None`.
    pub fn new(timestamp: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            timestamp: timestamp.into(),
            title: title.into(),
            description: None,
            icon: None,
        }
    }

    /// Sets the optional description text for this item.
    #[must_use]
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the optional leading icon glyph (emoji or short text) for this
    /// item.
    #[must_use]
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

// ── TimelineColors ────────────────────────────────────────────────────────────

/// Resolved color tokens for a [`Timeline`].
///
/// Exposed so unit tests can verify token binding without constructing a full
/// GPUI render context.
#[derive(Clone, Copy, Debug)]
pub struct TimelineColors {
    /// Color of the vertical connecting line between node dots.
    pub line: gpui::Hsla,
    /// Fill color of each node dot.
    pub dot: gpui::Hsla,
    /// Border / ring color around each node dot.
    pub dot_border: gpui::Hsla,
    /// Primary title text color.
    pub title_text: gpui::Hsla,
    /// Muted text color used for timestamps and descriptions.
    pub muted_text: gpui::Hsla,
}

impl TimelineColors {
    /// Resolves timeline colors from the given `tokens`.
    #[must_use]
    pub fn resolve(theme: &Theme) -> Self {
        Self {
            line: theme.border,
            dot: theme.accent,
            dot_border: theme.secondary,
            title_text: theme.foreground,
            muted_text: theme.muted_foreground,
        }
    }
}

// ── Timeline ──────────────────────────────────────────────────────────────────

/// Vertical timeline component that renders a sequence of [`TimelineItem`]
/// entries with connecting lines and node dots.
///
/// `Timeline` is a [`RenderOnce`] component — it does not hold GPUI entity
/// state.  Pass all items at construction time via [`Timeline::items`].
///
/// # Layout
///
/// ```text
///  ● 2024-01-01  Project start
///  │
///  ● 2024-03-15  First release
///  │             v0.1.0 shipped to production
///  │
///  ● 2024-06-01  v1.0 GA
/// ```
#[derive(IntoElement)]
pub struct Timeline {
    id: ElementId,
    items: Vec<TimelineItem>,
    dark: bool,
}

impl Timeline {
    /// Creates an empty timeline.  Add items via [`Timeline::items`].
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            dark: true,
        }
    }

    /// Replaces the item list with `items`.
    #[must_use]
    pub fn items(mut self, items: Vec<TimelineItem>) -> Self {
        self.items = items;
        self
    }

    /// Selects light (`false`) or dark (`true`) token palette.
    #[must_use]
    pub fn dark(mut self, dark: bool) -> Self {
        self.dark = dark;
        self
    }

    /// Returns the number of items in the timeline.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if this timeline contains at least one item.
    #[must_use]
    pub fn has_items(&self) -> bool {
        !self.items.is_empty()
    }

    /// Returns `true` if the dark token palette is selected.
    #[must_use]
    pub fn is_dark(&self) -> bool {
        self.dark
    }
}

impl RenderOnce for Timeline {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = if self.dark {
            Theme::from(&*ThemeColor::dark())
        } else {
            Theme::from(&*ThemeColor::light())
        };
        let colors = TimelineColors::resolve(&theme);
        let count = self.items.len();

        let mut container = div().id(self.id).flex().flex_col();

        for (idx, item) in self.items.into_iter().enumerate() {
            let is_first = idx == 0;
            let is_last = idx + 1 == count;

            // ── left gutter: connecting line + node dot ───────────────────────

            // Top connector (absent for the first item).
            let top_connector = if is_first {
                div().w(px(2.0)).h(px(12.0)).into_any_element()
            } else {
                div()
                    .w(px(2.0))
                    .h(px(12.0))
                    .bg(colors.line)
                    .into_any_element()
            };

            // Node dot — identified with a deterministic element id so tests can
            // count dot elements.
            let dot_id: SharedString = format!("tl-dot-{idx}").into();
            let node_dot = div()
                .id(ElementId::Name(dot_id))
                .w(px(10.0))
                .h(px(10.0))
                .rounded_full()
                .bg(colors.dot)
                .border_2()
                .border_color(colors.dot_border);

            // Bottom connector (absent for the last item).
            let bottom_connector = if is_last {
                div()
                    .w(px(2.0))
                    .flex_grow(1.0)
                    .min_h(px(8.0))
                    .into_any_element()
            } else {
                div()
                    .w(px(2.0))
                    .flex_grow(1.0)
                    .min_h(px(8.0))
                    .bg(colors.line)
                    .into_any_element()
            };

            let gutter = div()
                .w(px(24.0))
                .flex()
                .flex_col()
                .items_center()
                .flex_shrink_0()
                .child(top_connector)
                .child(node_dot)
                .child(bottom_connector);

            // ── right content column ──────────────────────────────────────────

            // Timestamp row: optional icon glyph + timestamp text.
            let ts_row = if let Some(icon_glyph) = item.icon {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(TypographyScale::default().sm))
                            .child(icon_glyph),
                    )
                    .child(
                        div()
                            .text_size(px(TypographyScale::default().sm))
                            .text_color(colors.muted_text)
                            .child(item.timestamp),
                    )
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(TypographyScale::default().sm))
                            .text_color(colors.muted_text)
                            .child(item.timestamp),
                    )
                    .into_any_element()
            };

            // Title row.
            let title_row = div()
                .text_size(px(TypographyScale::default().md))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(colors.title_text)
                .child(item.title);

            let mut content = div()
                .flex()
                .flex_col()
                .gap_1()
                .pb_3()
                .pl_2()
                .flex_grow(1.0)
                .child(ts_row)
                .child(title_row);

            // Optional description.
            if let Some(desc) = item.description {
                content = content.child(
                    div()
                        .text_size(px(TypographyScale::default().sm))
                        .text_color(colors.muted_text)
                        .child(desc),
                );
            }

            // Complete item row.
            let item_row = div()
                .flex()
                .flex_row()
                .min_h(px(48.0))
                .child(gutter)
                .child(content);

            container = container.child(item_row);
        }

        container
    }
}
