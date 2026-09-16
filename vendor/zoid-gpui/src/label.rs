//! `Label` primitive for generated GPUI starters.

use gpui::{
    App, FontWeight, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
};

use crate::ui_tokens::TypographyScale;
use gpui_kit::component::{Theme, ThemeColor};

/// Size variants for [`Label`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LabelSize {
    /// 12 px — captions, metadata.
    Small,
    /// 14 px — body text (default).
    #[default]
    Medium,
    /// 16 px — emphasized / section headings.
    Large,
}

/// Font-weight variants for [`Label`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LabelWeight {
    /// 400 — normal prose (default).
    #[default]
    Regular,
    /// 500 — medium emphasis.
    Medium,
    /// 700 — strong emphasis, headings.
    Bold,
}

/// Theme-token color for [`Label`] text.
///
/// Each variant maps to one named color slot in the active theme, so
/// labels automatically adapt to light and dark mode when the caller provides
/// the correct token set.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LabelColor {
    /// Primary text color (default).
    #[default]
    Text,
    /// Muted secondary text color.
    TextMuted,
    /// Accent / brand color.
    Accent,
    /// Destructive status color.
    Danger,
    /// Warning status color.
    Warning,
    /// Success status color.
    Success,
}

impl LabelColor {
    /// Resolves this token to a concrete GPUI color using `tokens`.
    ///
    /// All variants map to named slots in the active theme; there is no
    /// runtime fallback path, so the caller must supply a valid token set.
    #[must_use]
    pub fn resolve(self, theme: &Theme) -> gpui::Hsla {
        match self {
            Self::Text => theme.foreground,
            Self::TextMuted => theme.muted_foreground,
            Self::Accent => theme.accent,
            Self::Danger => theme.danger,
            Self::Warning => theme.warning,
            Self::Success => theme.success,
        }
    }
}

/// Single-line text label component for generated GPUI starters.
///
/// Accepts a text string, a [`LabelSize`] variant, a [`LabelWeight`] variant,
/// and a [`LabelColor`] that maps to a theme token.  The component adapts to
/// light and dark mode automatically when the caller supplies the appropriate
/// token set via the `dark` flag.
///
/// # Example
///
/// ```ignore
/// Label::new("Hello, world!")
///     .size(LabelSize::Large)
///     .weight(LabelWeight::Bold)
///     .color(LabelColor::Accent)
///     .dark(true)
/// ```
#[derive(IntoElement)]
pub struct Label {
    text: SharedString,
    size: LabelSize,
    weight: LabelWeight,
    color: LabelColor,
    dark: bool,
}

impl Label {
    /// Creates a label with default styling: medium size, regular weight,
    /// primary text color, dark mode.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            size: LabelSize::default(),
            weight: LabelWeight::default(),
            color: LabelColor::default(),
            dark: true,
        }
    }

    /// Returns the label's text content.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the size variant.
    #[must_use]
    pub fn size(mut self, size: LabelSize) -> Self {
        self.size = size;
        self
    }

    /// Sets the font-weight variant.
    #[must_use]
    pub fn weight(mut self, weight: LabelWeight) -> Self {
        self.weight = weight;
        self
    }

    /// Sets the theme-token color.
    #[must_use]
    pub fn color(mut self, color: LabelColor) -> Self {
        self.color = color;
        self
    }

    /// Selects light (`false`) or dark (`true`) token palette.
    #[must_use]
    pub fn dark(mut self, dark: bool) -> Self {
        self.dark = dark;
        self
    }
}

impl RenderOnce for Label {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = if self.dark {
            Theme::from(&*ThemeColor::dark())
        } else {
            Theme::from(&*ThemeColor::light())
        };

        let font_size = match self.size {
            LabelSize::Small => TypographyScale::default().sm,
            LabelSize::Medium => TypographyScale::default().md,
            LabelSize::Large => TypographyScale::default().lg,
        };

        let font_weight = match self.weight {
            LabelWeight::Regular => FontWeight::NORMAL,
            LabelWeight::Medium => FontWeight::MEDIUM,
            LabelWeight::Bold => FontWeight::BOLD,
        };

        let color = self.color.resolve(&theme);

        div()
            .text_size(px(font_size))
            .font_weight(font_weight)
            .text_color(color)
            .child(self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_tokens::color_to_hex;
    use gpui_kit::component::{Theme, ThemeColor};

    #[test]
    fn label_stores_text() {
        let label = Label::new("Hello, world!");
        assert_eq!(label.text(), "Hello, world!");
    }

    #[test]
    fn label_default_variants() {
        let label = Label::new("test");
        assert_eq!(label.size, LabelSize::Medium);
        assert_eq!(label.weight, LabelWeight::Regular);
        assert_eq!(label.color, LabelColor::Text);
        assert!(label.dark);
    }

    #[test]
    fn label_builder_setters_roundtrip() {
        let label = Label::new("x")
            .size(LabelSize::Small)
            .weight(LabelWeight::Bold)
            .color(LabelColor::Danger)
            .dark(false);

        assert_eq!(label.size, LabelSize::Small);
        assert_eq!(label.weight, LabelWeight::Bold);
        assert_eq!(label.color, LabelColor::Danger);
        assert!(!label.dark);
    }

    #[test]
    fn label_color_token_resolves_in_dark_mode() {
        let theme = Theme::from(&*ThemeColor::dark());
        assert_eq!(
            color_to_hex(LabelColor::Text.resolve(&theme)),
            color_to_hex(theme.foreground)
        );
        assert_eq!(
            color_to_hex(LabelColor::TextMuted.resolve(&theme)),
            color_to_hex(theme.muted_foreground)
        );
        assert_eq!(
            color_to_hex(LabelColor::Accent.resolve(&theme)),
            color_to_hex(theme.accent)
        );
        assert_eq!(
            color_to_hex(LabelColor::Danger.resolve(&theme)),
            color_to_hex(theme.danger)
        );
        assert_eq!(
            color_to_hex(LabelColor::Warning.resolve(&theme)),
            color_to_hex(theme.warning)
        );
        assert_eq!(
            color_to_hex(LabelColor::Success.resolve(&theme)),
            color_to_hex(theme.success)
        );
    }

    #[test]
    fn label_color_token_resolves_in_light_mode() {
        let theme = Theme::from(&*ThemeColor::light());
        assert_eq!(
            color_to_hex(LabelColor::Text.resolve(&theme)),
            color_to_hex(theme.foreground)
        );
        assert_eq!(
            color_to_hex(LabelColor::TextMuted.resolve(&theme)),
            color_to_hex(theme.muted_foreground)
        );
        assert_eq!(
            color_to_hex(LabelColor::Accent.resolve(&theme)),
            color_to_hex(theme.accent)
        );
    }

    #[test]
    fn label_color_differs_between_light_and_dark() {
        let light = Theme::from(&*ThemeColor::light());
        let dark = Theme::from(&*ThemeColor::dark());

        // Primary text color must differ between modes.
        assert_ne!(
            color_to_hex(LabelColor::Text.resolve(&light)),
            color_to_hex(LabelColor::Text.resolve(&dark)),
            "LabelColor::Text must differ between light and dark mode"
        );
        assert_ne!(
            color_to_hex(LabelColor::Accent.resolve(&light)),
            color_to_hex(LabelColor::Accent.resolve(&dark)),
            "LabelColor::Accent must differ between light and dark mode"
        );
    }
}
