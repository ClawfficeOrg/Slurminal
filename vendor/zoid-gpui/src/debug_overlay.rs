//! `DebugOverlay` component for generated GPUI starters.
//!
//! [`DebugOverlay`] is a GPUI entity (`View`) that renders a fixed-position,
//! semi-transparent diagnostic overlay in the top-right corner of the window.
//! It displays:
//!
//! - **FPS** (reported as `"N/A"` — GPUI 0.2.2 does not expose frame timing
//!   publicly).
//! - **Entity count** (reported as `"N/A"` — not exposed in GPUI 0.2.2).
//! - **Window size** (live from `window.bounds().size` at render time).
//! - **Focused element id** (live from `window.focused()` at render time).
//! - **User-supplied entries** via [`DebugOverlay::with_entries`].
//!
//! # Visibility
//!
//! The overlay is hidden by default.  Call [`DebugOverlay::toggle`],
//! [`DebugOverlay::show`], or [`DebugOverlay::hide`] to control visibility.
//! The gallery uses a toggle button; applications may bind F12 or a dev key
//! to the `toggle()` method.
//!
//! # GPUI version
//!
//! Implemented against `gpui 0.2.2`.

use std::sync::Arc;

use gpui::{Context, IntoElement, Render, SharedString, Window, div, prelude::*, px};

use gpui_kit::component::{Theme, ThemeColor};

/// Token-resolved display colors for a [`DebugOverlay`].
///
/// Exposed so unit tests can verify token bindings without a full render
/// context.
#[derive(Clone, Copy, Debug)]
pub struct DebugOverlayColors {
    /// Panel background color (semi-transparent).
    pub panel_bg: gpui::Hsla,
    /// Border color of the overlay panel.
    pub panel_border: gpui::Hsla,
    /// Text color for labels (key column).
    pub label_text: gpui::Hsla,
    /// Text color for values (value column).
    pub value_text: gpui::Hsla,
    /// Title text color.
    pub title_text: gpui::Hsla,
}

impl DebugOverlayColors {
    /// Resolves debug overlay colors from the given token set.
    ///
    /// The panel background is made translucent (alpha 0.65) so content behind
    /// it is visible. Text and title use 90% opacity for readability.
    #[must_use]
    pub fn resolve(theme: &Theme) -> Self {
        let panel_bg = theme.secondary;
        Self {
            panel_bg: gpui::hsla(panel_bg.h, panel_bg.s, panel_bg.l, 0.65),
            panel_border: gpui::hsla(theme.border.h, theme.border.s, theme.border.l, 0.9),
            label_text: gpui::hsla(
                theme.muted_foreground.h,
                theme.muted_foreground.s,
                theme.muted_foreground.l,
                0.9,
            ),
            value_text: gpui::hsla(
                theme.foreground.h,
                theme.foreground.s,
                theme.foreground.l,
                0.9,
            ),
            title_text: gpui::hsla(
                theme.foreground.h,
                theme.foreground.s,
                theme.foreground.l,
                0.9,
            ),
        }
    }
}

// ── DebugOverlay entity ─────────────────────────────────────────────────────

/// Fixed-position diagnostic overlay.
///
/// Hidden by default.  Use [`DebugOverlay::toggle`] to show/hide, or access
/// visibility via the public field for external toggling (e.g. from a dev
/// key handler).
pub struct DebugOverlay {
    /// Whether the overlay is currently visible.
    visible: bool,
    /// User-supplied key/value entries displayed in the overlay.
    entries: Arc<Vec<(SharedString, SharedString)>>,
    /// Whether the dark palette is active.
    dark: bool,
}

// ── Builder impl ─────────────────────────────────────────────────────────────

impl DebugOverlay {
    /// Creates a new `DebugOverlay` entity, initially hidden.
    #[must_use]
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            visible: false,
            entries: Arc::new(Vec::new()),
            dark: false,
        }
    }

    /// Sets the user-supplied key/value entries displayed in the overlay.
    #[must_use]
    pub fn with_entries(
        mut self,
        entries: Vec<(impl Into<SharedString>, impl Into<SharedString>)>,
    ) -> Self {
        self.entries = Arc::new(
            entries
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        );
        self
    }

    /// Enables dark mode palette.
    #[must_use]
    pub fn dark(mut self, dark: bool) -> Self {
        self.dark = dark;
        self
    }

    /// Toggles the overlay visibility.
    pub fn toggle(&mut self, _cx: &mut Context<Self>) {
        self.visible = !self.visible;
        _cx.notify();
    }

    /// Shows the overlay.
    pub fn show(&mut self, cx: &mut Context<Self>) {
        if !self.visible {
            self.visible = true;
            cx.notify();
        }
    }

    /// Hides the overlay.
    pub fn hide(&mut self, cx: &mut Context<Self>) {
        if self.visible {
            self.visible = false;
            cx.notify();
        }
    }

    /// Returns whether the overlay is currently visible.
    #[must_use]
    pub fn visible(&self) -> bool {
        self.visible
    }

    /// Replaces the user-supplied entries and notifies.
    pub fn set_entries(
        &mut self,
        entries: Vec<(impl Into<SharedString>, impl Into<SharedString>)>,
        cx: &mut Context<Self>,
    ) {
        self.entries = Arc::new(
            entries
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        );
        cx.notify();
    }

    /// Returns the current user-supplied entries.
    #[must_use]
    pub fn entries(&self) -> &Arc<Vec<(SharedString, SharedString)>> {
        &self.entries
    }

    /// Sets the dark mode flag and notifies.
    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        self.dark = dark;
        cx.notify();
    }
}

// ── Render ───────────────────────────────────────────────────────────────────

impl Render for DebugOverlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.visible {
            return div().into_any_element();
        }

        let theme = if self.dark {
            Theme::from(&*ThemeColor::dark())
        } else {
            Theme::from(&*ThemeColor::light())
        };
        let colors = DebugOverlayColors::resolve(&theme);

        let window_size = window.bounds().size;
        let size_label = SharedString::from(format!(
            "{} × {}",
            f32::from(window_size.width).round() as u32,
            f32::from(window_size.height).round() as u32,
        ));

        let focused_label = window
            .focused(cx)
            .map(|f| SharedString::from(format!("{:?}", f)))
            .unwrap_or_else(|| SharedString::from("(none)"));

        let entries = self.entries.clone();

        div()
            .id("debug-overlay")
            .absolute()
            .top(px(0.0))
            .right(px(0.0))
            .p(px(8.0))
            .min_w(px(200.0))
            .bg(colors.panel_bg)
            .border_1()
            .border_color(colors.panel_border)
            .rounded(px(6.0))
            .shadow_lg()
            .text_size(px(11.0))
            .font_family(crate::MONO_FONT_FAMILY)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    // Title
                    .child(
                        div()
                            .text_color(colors.title_text)
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .mb_1()
                            .child(SharedString::from("Debug Overlay")),
                    )
                    // FPS — stubbed
                    .child(Self::entry_row(
                        "FPS",
                        "N/A",
                        colors.label_text,
                        colors.value_text,
                    ))
                    // Entity count — stubbed
                    .child(Self::entry_row(
                        "Entities",
                        "N/A",
                        colors.label_text,
                        colors.value_text,
                    ))
                    // Window size
                    .child(Self::entry_row(
                        "Window",
                        &size_label,
                        colors.label_text,
                        colors.value_text,
                    ))
                    // Focused element
                    .child(Self::entry_row(
                        "Focus",
                        &focused_label,
                        colors.label_text,
                        colors.value_text,
                    ))
                    // Divider
                    .child(div().h(px(1.0)).my_1().bg(colors.panel_border))
                    // User entries
                    .children(
                        entries.iter().map(|(k, v)| {
                            Self::entry_row(k, v, colors.label_text, colors.value_text)
                        }),
                    ),
            )
            .into_any_element()
    }
}

// ── render helpers ───────────────────────────────────────────────────────────

impl DebugOverlay {
    /// Renders a single key/value row in the overlay.
    fn entry_row(
        key: impl Into<SharedString>,
        value: impl Into<SharedString>,
        label_color: gpui::Hsla,
        value_color: gpui::Hsla,
    ) -> gpui::AnyElement {
        div()
            .flex()
            .flex_row()
            .gap_2()
            .child(
                div()
                    .flex_shrink_0()
                    .w(px(64.0))
                    .text_color(label_color)
                    .child(key.into()),
            )
            .child(div().text_color(value_color).child(value.into()))
            .into_any_element()
    }
}
