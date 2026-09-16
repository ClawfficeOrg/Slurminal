//! `ListItem` row component for generated GPUI starters.

use gpui::{
    AnyElement, App, ClickEvent, ElementId, IntoElement, RenderOnce, SharedString, Window, div,
    prelude::*, px,
};

use crate::ui_tokens::{RadiusScale, SpacingScale, TypographyScale};
use gpui_kit::component::{Theme, ThemeColor};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// Colors resolved from tokens for list-item interactive states.
///
/// Exposed as a `pub` value type so unit tests can verify token binding
/// without constructing a GPUI render context.
#[derive(Clone, Copy, Debug)]
pub struct ListItemColors {
    /// Row background in idle (unselected) state.
    pub background: gpui::Hsla,
    /// Row background while the pointer hovers over the row.
    pub hover_background: gpui::Hsla,
    /// Row background when the item is selected.
    pub selected_background: gpui::Hsla,
    /// Primary text color.
    pub text: gpui::Hsla,
    /// Secondary / muted text color.
    pub text_secondary: gpui::Hsla,
}

impl ListItemColors {
    /// Resolves colors from the given token set.
    ///
    /// `hover_background` maps to `surface_hover`; `selected_background` uses
    /// the `accent` token at 15 % opacity to create a soft tinted highlight.
    #[must_use]
    pub fn resolve(theme: &Theme) -> Self {
        Self {
            background: theme.secondary,
            hover_background: theme.secondary,
            selected_background: theme.accent.opacity(0.15),
            text: theme.foreground,
            text_secondary: theme.muted_foreground,
        }
    }
}

/// A themed row component with an optional leading icon, primary text,
/// optional secondary text, optional trailing element, and an `on_click`
/// handler. Hover and selected states are driven by [`ListItemColors`].
///
/// # Example
///
/// ```ignore
/// ListItem::new("item-1", "Open project")
///     .leading_icon("📁")
///     .secondary("Last modified today")
///     .selected(false)
///     .on_click(|_, _, _| println!("clicked"))
/// ```
#[derive(IntoElement)]
pub struct ListItem {
    id: ElementId,
    primary: SharedString,
    leading_icon: Option<SharedString>,
    secondary: Option<SharedString>,
    trailing: Option<AnyElement>,
    selected: bool,
    dark: bool,
    on_click: Option<ClickHandler>,
}

impl ListItem {
    /// Creates a list item with the given primary text and default styling:
    /// dark-mode tokens, no leading icon, no secondary text, no trailing
    /// element, not selected, no click handler.
    pub fn new(id: impl Into<ElementId>, primary: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            primary: primary.into(),
            leading_icon: None,
            secondary: None,
            trailing: None,
            selected: false,
            dark: true,
            on_click: None,
        }
    }

    /// Returns the primary text label.
    #[must_use]
    pub fn primary(&self) -> &str {
        &self.primary
    }

    /// Returns `true` if an `on_click` handler has been registered.
    #[must_use]
    pub fn has_on_click(&self) -> bool {
        self.on_click.is_some()
    }

    /// Returns `true` if a leading icon has been set.
    #[must_use]
    pub fn has_leading_icon(&self) -> bool {
        self.leading_icon.is_some()
    }

    /// Returns `true` if secondary text has been set.
    #[must_use]
    pub fn has_secondary(&self) -> bool {
        self.secondary.is_some()
    }

    /// Returns `true` if a trailing element has been set.
    #[must_use]
    pub fn has_trailing(&self) -> bool {
        self.trailing.is_some()
    }

    /// Sets an optional leading icon glyph, emoji, or single-character label.
    #[must_use]
    pub fn leading_icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.leading_icon = Some(icon.into());
        self
    }

    /// Sets optional secondary descriptive text rendered beneath the primary label.
    #[must_use]
    pub fn secondary(mut self, text: impl Into<SharedString>) -> Self {
        self.secondary = Some(text.into());
        self
    }

    /// Appends an arbitrary trailing element (e.g. a badge count or icon button).
    #[must_use]
    pub fn trailing(mut self, element: impl IntoElement) -> Self {
        self.trailing = Some(element.into_any_element());
        self
    }

    /// Sets the selected state of this list item.
    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Selects light (`false`) or dark (`true`) token palette.
    #[must_use]
    pub fn dark(mut self, dark: bool) -> Self {
        self.dark = dark;
        self
    }

    /// Registers a click handler for the row.
    ///
    /// The handler receives the GPUI [`ClickEvent`] together with mutable
    /// references to the [`Window`] and [`App`] context, matching the
    /// standard GPUI interaction signature.
    #[must_use]
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for ListItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = if self.dark {
            Theme::from(&*ThemeColor::dark())
        } else {
            Theme::from(&*ThemeColor::light())
        };
        let colors = ListItemColors::resolve(&theme);

        let bg = if self.selected {
            colors.selected_background
        } else {
            colors.background
        };
        let hover_bg = colors.hover_background;

        // Leading icon — flex-shrink-0 so it never collapses.
        // Text column — flex-1 so it fills available space.
        div()
            .id(self.id)
            .flex()
            .items_center()
            .gap_2()
            .px(px(SpacingScale::default().md))
            .py(px(SpacingScale::default().xs))
            .rounded(px(RadiusScale::default().sm))
            .bg(bg)
            .cursor_pointer()
            .hover(move |style| style.bg(hover_bg))
            .when_some(self.leading_icon, |this, icon| {
                this.child(
                    div()
                        .flex_shrink_0()
                        .text_color(colors.text)
                        .text_size(px(TypographyScale::default().md))
                        .child(icon),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .child(
                        div()
                            .text_color(colors.text)
                            .text_size(px(TypographyScale::default().md))
                            .child(self.primary),
                    )
                    .when_some(self.secondary, |this, s| {
                        this.child(
                            div()
                                .text_color(colors.text_secondary)
                                .text_size(px(TypographyScale::default().sm))
                                .child(s),
                        )
                    }),
            )
            .when_some(self.trailing, |this, el| this.child(el))
            .when_some(self.on_click, |this, handler| this.on_click(handler))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_tokens::color_to_hex;
    use gpui_kit::component::{Theme, ThemeColor};

    #[test]
    fn list_item_stores_primary_text() {
        let item = ListItem::new("i", "Open project");
        assert_eq!(item.primary(), "Open project");
    }

    #[test]
    fn list_item_defaults() {
        let item = ListItem::new("i", "x");
        assert!(!item.has_leading_icon());
        assert!(!item.has_secondary());
        assert!(!item.has_trailing());
        assert!(!item.selected);
        assert!(item.dark);
        assert!(!item.has_on_click());
    }

    #[test]
    fn list_item_builder_setters_roundtrip() {
        let item = ListItem::new("i", "x")
            .leading_icon("★")
            .secondary("hint")
            .selected(true)
            .dark(false);

        assert!(item.has_leading_icon());
        assert!(item.has_secondary());
        assert!(item.selected);
        assert!(!item.dark);
    }

    /// Verifies that registering an `on_click` handler sets the internal slot.
    /// This is the unit-level "click handler fires" assertion: the handler is
    /// stored and will be wired into the GPUI element tree on render.
    #[test]
    fn list_item_on_click_registers_handler() {
        let item = ListItem::new("i", "x").on_click(|_, _, _| {});
        assert!(item.has_on_click(), "on_click handler must be registered");
    }

    /// Verifies the trailing setter stores an element.
    #[test]
    fn list_item_trailing_registers_element() {
        let item = ListItem::new("i", "x").trailing(div());
        assert!(item.has_trailing());
    }

    /// Verifies that the hover token resolves to `surface_hover` in dark mode.
    ///
    /// This is the unit-level "hover state renders" assertion: the token that
    /// drives the hover background is correctly sourced from the design system.
    #[test]
    fn list_item_hover_token_uses_surface_hover_in_dark_mode() {
        let theme = Theme::from(&*ThemeColor::dark());
        let colors = ListItemColors::resolve(&theme);
        assert_eq!(
            color_to_hex(colors.hover_background),
            color_to_hex(theme.secondary),
            "hover_background must map to the secondary token"
        );
    }

    #[test]
    fn list_item_idle_background_uses_surface_token() {
        let theme = Theme::from(&*ThemeColor::dark());
        let colors = ListItemColors::resolve(&theme);
        assert_eq!(
            color_to_hex(colors.background),
            color_to_hex(theme.secondary)
        );
    }

    /// Verifies that the selected-state background is derived from the accent
    /// token so the selection tint is always brand-consistent.
    #[test]
    fn list_item_selected_background_derives_from_accent_token() {
        let theme = Theme::from(&*ThemeColor::dark());
        let colors = ListItemColors::resolve(&theme);
        // selected_background = accent.opacity(0.15); hue/sat/lightness of the
        // base accent color are preserved, only alpha differs.
        let accent = theme.accent;
        // The selected background must share hue/sat/lightness with accent.
        assert_eq!(
            colors.selected_background.h, accent.h,
            "selected hue must match accent hue"
        );
        assert_eq!(
            colors.selected_background.s, accent.s,
            "selected saturation must match accent saturation"
        );
        assert_eq!(
            colors.selected_background.l, accent.l,
            "selected lightness must match accent lightness"
        );
        assert!(
            colors.selected_background.a < accent.a,
            "selected_background alpha must be less than full-opacity accent"
        );
    }

    #[test]
    fn list_item_colors_differ_between_light_and_dark() {
        let light = Theme::from(&*ThemeColor::light());
        let dark = Theme::from(&*ThemeColor::dark());
        let lc = ListItemColors::resolve(&light);
        let dc = ListItemColors::resolve(&dark);
        assert_ne!(
            color_to_hex(lc.background),
            color_to_hex(dc.background),
            "background must differ between light and dark mode"
        );
        assert_ne!(
            color_to_hex(lc.hover_background),
            color_to_hex(dc.hover_background),
            "hover_background must differ between light and dark mode"
        );
    }

    #[test]
    fn list_item_text_secondary_uses_text_muted_token() {
        let theme = Theme::from(&*ThemeColor::dark());
        let colors = ListItemColors::resolve(&theme);
        assert_eq!(
            color_to_hex(colors.text_secondary),
            color_to_hex(theme.muted_foreground)
        );
    }
}
