//! `Image` component for generated GPUI starters.
//!
//! [`Image`] is a stateless `RenderOnce` wrapper around [`gpui::img()`] that
//! provides a consistent API for displaying images with optional dimensions,
//! object-fit modes, placeholder rendering while loading, and a broken-image
//! icon on error.
//!
//! ## Object fit
//!
//! [`ImageObjectFit`] maps directly to [`gpui::ObjectFit`]:
//!
//! | Variant   | Behaviour                                       |
//! |-----------|-------------------------------------------------|
//! | `Contain` | Scale to fit within the box (default).          |
//! | `Cover`   | Scale to cover the box; clips to maintain ratio.|
//! | `Fill`    | Stretch to fill the box exactly.                |
//!
//! ## Loading and error states
//!
//! When `src` is `None` or an empty string, the component renders a styled
//! placeholder div immediately — no network or disk I/O occurs.
//!
//! When `src` is provided, GPUI loads the image asynchronously.  While
//! loading, [`gpui::StyledImage::with_loading`] renders the same placeholder.
//! On a permanent load failure, [`gpui::StyledImage::with_fallback`] renders
//! a broken-image icon (⚠) with a danger-tinted border so the missing image
//! is visually distinct.
//!
//! ## Security
//!
//! `src` is passed directly to GPUI's image-loading pipeline which accepts
//! valid URIs (`https://…`) and embedded resource identifiers.  **Do not**
//! pass unsanitised user-supplied file-system paths; validate against an
//! allowlist before setting `src` if paths come from external input.
//!
//! ## Example
//!
//! ```rust,ignore
//! Image::new("hero")
//!     .src("https://example.com/banner.png")
//!     .width(320.0)
//!     .height(200.0)
//!     .object_fit(ImageObjectFit::Cover)
//!     .alt("Hero banner")
//!     .dark(true)
//! ```
//!
//! ## GPUI version
//!
//! Implemented against `gpui 0.2.2`.

use gpui::{
    App, ElementId, IntoElement, ObjectFit, RenderOnce, SharedString, StyledImage, Window, div,
    prelude::*, px,
};

use crate::ui_tokens::RadiusScale;
use gpui_kit::component::{Theme, ThemeColor};

// ── ImageObjectFit ─────────────────────────────────────────────────────────────

/// How the image should be sized within its bounding box.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ImageObjectFit {
    /// Scale the image to fit within the box while preserving aspect ratio
    /// (default).
    #[default]
    Contain,
    /// Scale the image to cover the entire box; portions outside the box are
    /// clipped.
    Cover,
    /// Stretch the image to completely fill the box; aspect ratio is not
    /// preserved.
    Fill,
}

impl From<ImageObjectFit> for ObjectFit {
    fn from(fit: ImageObjectFit) -> Self {
        match fit {
            ImageObjectFit::Contain => ObjectFit::Contain,
            ImageObjectFit::Cover => ObjectFit::Cover,
            ImageObjectFit::Fill => ObjectFit::Fill,
        }
    }
}

// ── ImageColors ────────────────────────────────────────────────────────────────

/// Token-resolved colors for [`Image`] placeholder and error states.
///
/// Exposed so unit tests can verify token binding without constructing a full
/// GPUI render context.
#[derive(Clone, Copy, Debug)]
pub struct ImageColors {
    /// Background of the placeholder / loading box.
    pub placeholder_bg: gpui::Hsla,
    /// Border of the placeholder / loading box.
    pub placeholder_border: gpui::Hsla,
    /// Icon and text color inside the placeholder.
    pub placeholder_text: gpui::Hsla,
    /// Background of the broken-image error box.
    pub error_bg: gpui::Hsla,
    /// Border of the broken-image error box.
    pub error_border: gpui::Hsla,
    /// Icon color inside the error box.
    pub error_text: gpui::Hsla,
}

impl ImageColors {
    /// Resolves image colors from the given token set.
    #[must_use]
    pub fn resolve(theme: &Theme) -> Self {
        Self {
            placeholder_bg: theme.secondary,
            placeholder_border: theme.border,
            placeholder_text: theme.muted_foreground,
            error_bg: theme.secondary,
            error_border: theme.danger,
            error_text: theme.danger,
        }
    }
}

// ── Image ──────────────────────────────────────────────────────────────────────

/// Image component for generated GPUI starters.
///
/// Renders an image from `src` inside a fixed-size clipping container.
/// While the image is loading GPUI shows the placeholder; on permanent load
/// failure the broken-image icon is shown.  When `src` is `None` or empty the
/// placeholder is rendered directly.
///
/// # Example
///
/// ```ignore
/// Image::new("hero")
///     .src("https://example.com/banner.png")
///     .width(320.0)
///     .height(200.0)
///     .object_fit(ImageObjectFit::Cover)
///     .alt("Hero banner")
///     .dark(false)
/// ```
#[derive(IntoElement)]
pub struct Image {
    id: ElementId,
    src: Option<SharedString>,
    width: Option<f32>,
    height: Option<f32>,
    object_fit: ImageObjectFit,
    alt: SharedString,
    dark: bool,
}

impl Image {
    /// Creates a new [`Image`] with no source, default dimensions, `Contain`
    /// fit, empty alt text, and dark-mode tokens.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            src: None,
            width: None,
            height: None,
            object_fit: ImageObjectFit::default(),
            alt: SharedString::default(),
            dark: true,
        }
    }

    /// Sets the image source (URI or embedded resource identifier).
    ///
    /// Passing an empty string is treated the same as `None`; the placeholder
    /// is rendered instead.
    #[must_use]
    pub fn src(mut self, src: impl Into<SharedString>) -> Self {
        let s: SharedString = src.into();
        self.src = if s.is_empty() { None } else { Some(s) };
        self
    }

    /// Sets the display width in logical pixels.
    ///
    /// Defaults to `160.0` when not specified.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Sets the display height in logical pixels.
    ///
    /// Defaults to `120.0` when not specified.
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// Sets how the image is fitted inside its bounding box.
    #[must_use]
    pub fn object_fit(mut self, fit: ImageObjectFit) -> Self {
        self.object_fit = fit;
        self
    }

    /// Sets the alt text shown in the placeholder and used for accessibility.
    #[must_use]
    pub fn alt(mut self, alt: impl Into<SharedString>) -> Self {
        self.alt = alt.into();
        self
    }

    /// Selects light (`false`) or dark (`true`) token palette.
    #[must_use]
    pub fn dark(mut self, dark: bool) -> Self {
        self.dark = dark;
        self
    }

    // ── accessors for tests ───────────────────────────────────────────────────

    /// Returns the image source string if one was set.
    #[must_use]
    pub fn image_src(&self) -> Option<&str> {
        self.src.as_ref().map(|s| s.as_ref())
    }

    /// Returns the explicit width override, if any.
    #[must_use]
    pub fn image_width(&self) -> Option<f32> {
        self.width
    }

    /// Returns the explicit height override, if any.
    #[must_use]
    pub fn image_height(&self) -> Option<f32> {
        self.height
    }

    /// Returns the current object-fit mode.
    #[must_use]
    pub fn image_object_fit(&self) -> ImageObjectFit {
        self.object_fit
    }

    /// Returns the alt text.
    #[must_use]
    pub fn image_alt(&self) -> &str {
        &self.alt
    }

    /// Returns `true` if the component uses the dark token palette.
    #[must_use]
    pub fn is_dark(&self) -> bool {
        self.dark
    }

    /// Returns `true` if a non-empty `src` has been set.
    #[must_use]
    pub fn has_src(&self) -> bool {
        self.src.is_some()
    }
}

// ── render helpers (free functions) ───────────────────────────────────────────

/// Renders the loading / no-source placeholder element.
///
/// Displays a camera icon centred inside a muted surface box with a dashed
/// border.  When `alt` is non-empty it is rendered beneath the icon as a
/// small label.
fn render_placeholder(w: f32, h: f32, colors: ImageColors, alt: SharedString) -> gpui::AnyElement {
    div()
        .w(px(w))
        .h(px(h))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(4.0))
        .bg(colors.placeholder_bg)
        .border_1()
        .border_color(colors.placeholder_border)
        .child(
            div()
                .text_size(px(24.0))
                .text_color(colors.placeholder_text)
                .child(SharedString::from("📷")),
        )
        .when(!alt.is_empty(), |el| {
            el.child(
                div()
                    .text_size(px(10.0))
                    .text_color(colors.placeholder_text)
                    .child(alt),
            )
        })
        .into_any_element()
}

/// Renders the broken-image error element.
///
/// Displays a warning icon (⚠) inside a danger-bordered box to visually
/// distinguish it from a loading placeholder.
fn render_error(w: f32, h: f32, colors: ImageColors) -> gpui::AnyElement {
    div()
        .w(px(w))
        .h(px(h))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .bg(colors.error_bg)
        .border_1()
        .border_color(colors.error_border)
        .child(
            div()
                .text_size(px(24.0))
                .text_color(colors.error_text)
                .child(SharedString::from("⚠")),
        )
        .into_any_element()
}

// ── RenderOnce ─────────────────────────────────────────────────────────────────

impl RenderOnce for Image {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = if self.dark {
            Theme::from(&*ThemeColor::dark())
        } else {
            Theme::from(&*ThemeColor::light())
        };
        let colors = ImageColors::resolve(&theme);

        let w = self.width.unwrap_or(160.0);
        let h = self.height.unwrap_or(120.0);

        let inner: gpui::AnyElement = if let Some(src) = self.src {
            let loading_colors = colors;
            let loading_alt = self.alt.clone();
            let error_colors = colors;
            let fit: ObjectFit = self.object_fit.into();

            gpui::img(src)
                .w(px(w))
                .h(px(h))
                .object_fit(fit)
                .with_loading(move || render_placeholder(w, h, loading_colors, loading_alt.clone()))
                .with_fallback(move || render_error(w, h, error_colors))
                .into_any_element()
        } else {
            render_placeholder(w, h, colors, self.alt)
        };

        div()
            .id(self.id)
            .w(px(w))
            .h(px(h))
            .overflow_hidden()
            .rounded(px(RadiusScale::default().sm))
            .child(inner)
    }
}

// ── unit tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_tokens::color_to_hex;
    use gpui_kit::component::{Theme, ThemeColor};

    // ── Image defaults ─────────────────────────────────────────────────────────

    #[test]
    fn image_default_values() {
        let img = Image::new("img");
        assert!(!img.has_src(), "new Image must have no source");
        assert_eq!(img.image_src(), None, "image_src must be None by default");
        assert_eq!(img.image_width(), None, "width must be None by default");
        assert_eq!(img.image_height(), None, "height must be None by default");
        assert_eq!(
            img.image_object_fit(),
            ImageObjectFit::Contain,
            "default fit must be Contain"
        );
        assert_eq!(img.image_alt(), "", "alt must be empty by default");
        assert!(img.is_dark(), "dark must be true by default");
    }

    // ── Image builder setters ──────────────────────────────────────────────────

    #[test]
    fn image_builder_setters() {
        let img = Image::new("img")
            .src("https://example.com/photo.png")
            .width(320.0)
            .height(200.0)
            .object_fit(ImageObjectFit::Cover)
            .alt("A test photo")
            .dark(false);

        assert!(img.has_src(), "has_src must be true after setting src");
        assert_eq!(
            img.image_src(),
            Some("https://example.com/photo.png"),
            "image_src must match"
        );
        assert_eq!(img.image_width(), Some(320.0), "width must be set");
        assert_eq!(img.image_height(), Some(200.0), "height must be set");
        assert_eq!(
            img.image_object_fit(),
            ImageObjectFit::Cover,
            "object_fit must be Cover"
        );
        assert_eq!(img.image_alt(), "A test photo", "alt must be set");
        assert!(!img.is_dark(), "dark(false) must set light mode");
    }

    // ── Placeholder on missing source ──────────────────────────────────────────

    #[test]
    fn image_shows_placeholder_when_src_is_none() {
        let img = Image::new("img").alt("Missing image");
        assert!(
            !img.has_src(),
            "Image with no src must show placeholder (has_src is false)"
        );
        assert_eq!(
            img.image_alt(),
            "Missing image",
            "alt text must be stored for placeholder display"
        );
    }

    #[test]
    fn image_empty_src_treated_as_none() {
        let img = Image::new("img").src("");
        assert!(!img.has_src(), "empty src string must be treated as None");
        assert_eq!(
            img.image_src(),
            None,
            "image_src must return None for empty string src"
        );
    }

    // ── Source present ─────────────────────────────────────────────────────────

    #[test]
    fn image_renders_with_known_source() {
        let img = Image::new("img").src("https://example.com/banner.png");
        assert!(
            img.has_src(),
            "Image with valid src must report has_src == true"
        );
        assert_eq!(
            img.image_src(),
            Some("https://example.com/banner.png"),
            "image_src must return the set URL"
        );
    }

    // ── ObjectFit mapping ──────────────────────────────────────────────────────

    #[test]
    fn image_object_fit_converts_to_gpui() {
        assert!(matches!(
            ObjectFit::from(ImageObjectFit::Contain),
            ObjectFit::Contain
        ));
        assert!(matches!(
            ObjectFit::from(ImageObjectFit::Cover),
            ObjectFit::Cover
        ));
        assert!(matches!(
            ObjectFit::from(ImageObjectFit::Fill),
            ObjectFit::Fill
        ));
    }

    // ── ImageColors token binding ──────────────────────────────────────────────

    #[test]
    fn image_colors_resolve_from_dark_tokens() {
        let theme = Theme::from(&*ThemeColor::dark());
        let colors = ImageColors::resolve(&theme);
        assert_eq!(
            color_to_hex(colors.placeholder_bg),
            color_to_hex(theme.secondary),
            "placeholder_bg must use secondary token"
        );
        assert_eq!(
            color_to_hex(colors.placeholder_border),
            color_to_hex(theme.border),
            "placeholder_border must use border token"
        );
        assert_eq!(
            color_to_hex(colors.error_border),
            color_to_hex(theme.danger),
            "error_border must use danger token"
        );
    }

    #[test]
    fn image_colors_differ_between_light_and_dark() {
        let light = Theme::from(&*ThemeColor::light());
        let dark = Theme::from(&*ThemeColor::dark());
        let cl = ImageColors::resolve(&light);
        let cd = ImageColors::resolve(&dark);
        assert_ne!(
            color_to_hex(cl.placeholder_bg),
            color_to_hex(cd.placeholder_bg),
            "placeholder_bg must differ between light and dark"
        );
    }

    // ── ImageObjectFit derives ─────────────────────────────────────────────────

    #[test]
    fn image_object_fit_variants_are_distinct() {
        assert_ne!(ImageObjectFit::Contain, ImageObjectFit::Cover);
        assert_ne!(ImageObjectFit::Cover, ImageObjectFit::Fill);
        assert_ne!(ImageObjectFit::Contain, ImageObjectFit::Fill);
    }
}
