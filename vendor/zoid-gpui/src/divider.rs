//! `Divider` and `Spacer` layout primitives for generated GPUI starters.

use gpui::{App, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, px};

use crate::ui_tokens::{SpacingScale, TypographyScale};
use gpui_kit::component::ThemeColor;

// ---------------------------------------------------------------------------
// Divider
// ---------------------------------------------------------------------------

/// A horizontal rule that visually separates sections of content.
///
/// The line color is sourced from the `border` color token so it adapts
/// automatically when the caller switches between light and dark mode.
/// An optional centered label can be rendered across the line to provide
/// additional context (e.g. "or", "settings", "recent").
///
/// # Example
///
/// ```ignore
/// Divider::new()
///     .label("or")
///     .dark(true)
/// ```
#[derive(IntoElement)]
pub struct Divider {
    label: Option<SharedString>,
    dark: bool,
}

impl Divider {
    /// Creates a plain horizontal divider with no label, using the dark token
    /// palette by default.
    pub fn new() -> Self {
        Self {
            label: None,
            dark: true,
        }
    }

    /// Sets an optional centered text label rendered across the divider line.
    #[must_use]
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Selects light (`false`) or dark (`true`) token palette.
    #[must_use]
    pub fn dark(mut self, dark: bool) -> Self {
        self.dark = dark;
        self
    }

    /// Returns `true` if a label has been set.
    #[must_use]
    pub fn has_label(&self) -> bool {
        self.label.is_some()
    }

    /// Returns the border color used for the divider line, resolved from
    /// the current token palette.
    ///
    /// Exposed for unit tests that need to verify token binding without
    /// constructing a full GPUI render context.
    #[must_use]
    pub fn line_color(&self) -> gpui::Hsla {
        let theme = if self.dark {
            ThemeColor::dark()
        } else {
            ThemeColor::light()
        };
        theme.border
    }
}

impl Default for Divider {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Divider {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = if self.dark {
            *ThemeColor::dark()
        } else {
            *ThemeColor::light()
        };

        let line_color = theme.border;
        let text_color = theme.muted_foreground;
        let font_size = TypographyScale::default().sm;
        let spacing = SpacingScale::default().sm;

        if let Some(label) = self.label {
            // Labeled variant: two lines flanking centered text.
            div()
                .flex()
                .items_center()
                .gap(px(spacing))
                .child(div().flex_1().h(px(1.0)).bg(line_color))
                .child(
                    div()
                        .flex_shrink_0()
                        .text_color(text_color)
                        .text_size(px(font_size))
                        .child(label),
                )
                .child(div().flex_1().h(px(1.0)).bg(line_color))
        } else {
            // Plain variant: a single full-width horizontal line.
            div()
                .flex()
                .items_center()
                .child(div().flex_1().h(px(1.0)).bg(line_color))
        }
    }
}

// ---------------------------------------------------------------------------
// Spacer
// ---------------------------------------------------------------------------

/// A flexible or fixed-size gap element for use inside flex containers.
///
/// Without a fixed size the spacer expands to fill all remaining space in its
/// flex axis (`flex-grow: 1`), pushing adjacent siblings to opposite ends.
/// With a fixed size it renders as a non-shrinkable block whose width and
/// height are both set to `size`, making it suitable as a uniform gap in
/// either a row or a column.
///
/// # Example
///
/// ```ignore
/// // Flexible: pushes trailing button to the right end.
/// Spacer::new()
///
/// // Fixed: inserts a 16 px gap.
/// Spacer::new().size(16.0)
/// ```
#[derive(IntoElement)]
pub struct Spacer {
    fixed_size: Option<f32>,
}

impl Spacer {
    /// Creates a flexible spacer that grows to fill available space.
    pub fn new() -> Self {
        Self { fixed_size: None }
    }

    /// Sets an exact pixel size for the spacer.
    ///
    /// Both the width and the height are set to `size`, making the element
    /// behave as a uniform gap in any flex direction.
    #[must_use]
    pub fn size(mut self, size: f32) -> Self {
        self.fixed_size = Some(size);
        self
    }

    /// Returns `true` if this spacer has a fixed size set.
    #[must_use]
    pub fn is_fixed(&self) -> bool {
        self.fixed_size.is_some()
    }

    /// Returns the fixed size in pixels, or `None` if the spacer is flexible.
    #[must_use]
    pub fn fixed_size(&self) -> Option<f32> {
        self.fixed_size
    }
}

impl Default for Spacer {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Spacer {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        if let Some(size) = self.fixed_size {
            div().w(px(size)).h(px(size)).flex_shrink_0()
        } else {
            div().flex_1()
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_tokens::color_to_hex;

    // --- Divider ---

    #[test]
    fn divider_default_has_no_label_and_is_dark() {
        let d = Divider::new();
        assert!(!d.has_label());
        assert!(d.dark);
    }

    #[test]
    fn divider_label_setter_registers_label() {
        let d = Divider::new().label("or");
        assert!(d.has_label());
    }

    #[test]
    fn divider_dark_setter_toggles_palette() {
        let d = Divider::new().dark(false);
        assert!(!d.dark);
    }

    #[test]
    fn divider_line_color_uses_border_token_in_dark_mode() {
        let d = Divider::new().dark(true);
        let theme = ThemeColor::dark();
        assert_eq!(
            color_to_hex(d.line_color()),
            color_to_hex(theme.border),
            "divider line must use the border token in dark mode"
        );
    }

    #[test]
    fn divider_line_color_uses_border_token_in_light_mode() {
        let d = Divider::new().dark(false);
        let theme = ThemeColor::light();
        assert_eq!(
            color_to_hex(d.line_color()),
            color_to_hex(theme.border),
            "divider line must use the border token in light mode"
        );
    }

    #[test]
    fn divider_line_color_differs_between_light_and_dark() {
        let light_color = Divider::new().dark(false).line_color();
        let dark_color = Divider::new().dark(true).line_color();
        assert_ne!(
            color_to_hex(light_color),
            color_to_hex(dark_color),
            "divider line color must differ between light and dark mode"
        );
    }

    #[test]
    fn divider_default_trait_impl_matches_new() {
        let d1 = Divider::new();
        let d2 = Divider::default();
        assert_eq!(d1.has_label(), d2.has_label());
        assert_eq!(d1.dark, d2.dark);
    }

    // --- Spacer ---

    #[test]
    fn spacer_default_is_flexible() {
        let s = Spacer::new();
        assert!(!s.is_fixed());
        assert_eq!(s.fixed_size(), None);
    }

    #[test]
    fn spacer_size_setter_makes_fixed() {
        let s = Spacer::new().size(16.0);
        assert!(s.is_fixed());
        assert_eq!(s.fixed_size(), Some(16.0));
    }

    #[test]
    fn spacer_size_zero_is_still_fixed() {
        let s = Spacer::new().size(0.0);
        assert!(s.is_fixed());
        assert_eq!(s.fixed_size(), Some(0.0));
    }

    #[test]
    fn spacer_default_trait_impl_matches_new() {
        let s1 = Spacer::new();
        let s2 = Spacer::default();
        assert_eq!(s1.is_fixed(), s2.is_fixed());
    }
}
