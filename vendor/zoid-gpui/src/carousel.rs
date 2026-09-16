//! `Carousel` component for generated GPUI starters.
//!
//! Provides [`Carousel`] — a stateful GPUI [`Render`] + [`Focusable`] view
//! that displays one item at a time from a list, with:
//!
//! - **Prev/Next arrow buttons** (optional, controlled via [`Carousel::show_arrows`]).
//! - **Dot indicators** below the item (optional, controlled via
//!   [`Carousel::show_dots`]).
//! - **Keyboard navigation**: Left arrow → previous item; Right arrow → next
//!   item.
//! - **Auto-play**: Advances the index automatically at a configurable interval
//!   via [`gpui::Context::spawn`].  Stopped when the carousel is dropped.
//! - **`on_change` callback**: Fired whenever the index changes with the new
//!   index value.
//! - **Looping**: When enabled (default), navigation wraps around at the ends.
//!
//! # Example
//!
//! ```rust,ignore
//! let carousel = cx.new(|cx| {
//!     Carousel::new(cx)
//!         .with_items(vec![
//!             SharedString::from("Slide 1"),
//!             SharedString::from("Slide 2"),
//!             SharedString::from("Slide 3"),
//!         ])
//!         .show_arrows(true)
//!         .show_dots(true)
//!         .with_autoplay(std::time::Duration::from_secs(3))
//!         .on_change(|idx, _w, _cx| println!("slide changed to {idx}"))
//!         .dark(true)
//! });
//! ```
//!
//! ## Headless tests
//!
//! Because `on_change` requires `&mut Window`, use [`Carousel::apply_next`]
//! and [`Carousel::apply_prev`] to drive state in
//! [`gpui::TestAppContext`] tests without a windowed context.
//!
//! # GPUI version
//!
//! Implemented against `gpui 0.2.2`.

use std::{rc::Rc, sync::Arc, time::Duration};

use gpui::{
    App, Context, ElementId, Entity, FocusHandle, Focusable, IntoElement, KeyDownEvent, Render,
    SharedString, Window, div, prelude::*, px,
};

use crate::ui_tokens::{RadiusScale, SpacingScale, TypographyScale};
use gpui_kit::component::{Theme, ThemeColor};

// ── handler type ──────────────────────────────────────────────────────────────

/// Callback fired when the active carousel index changes.
///
/// Receives the new zero-based index plus mutable window and app context
/// references.  Only invoked from UI-driven events (button clicks, keyboard
/// navigation, auto-play) where a [`Window`] is available.
pub type CarouselChangeHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

// ── CarouselColors ────────────────────────────────────────────────────────────

/// Token-resolved display colors for a [`Carousel`].
///
/// Exposed so unit tests can verify token bindings without a full render
/// context.
#[derive(Clone, Copy, Debug)]
pub struct CarouselColors {
    /// Background of the slide content area.
    pub slide_bg: gpui::Hsla,
    /// Border color of the slide area.
    pub slide_border: gpui::Hsla,
    /// Text color for slide content labels.
    pub slide_text: gpui::Hsla,
    /// Background of a prev/next arrow button (idle).
    pub arrow_bg: gpui::Hsla,
    /// Background of a prev/next arrow button (hover).
    pub arrow_hover_bg: gpui::Hsla,
    /// Foreground color of the arrow glyph.
    pub arrow_fg: gpui::Hsla,
    /// Color of an inactive dot indicator.
    pub dot_inactive: gpui::Hsla,
    /// Color of the active dot indicator.
    pub dot_active: gpui::Hsla,
    /// Focus ring color.
    pub focus_ring: gpui::Hsla,
}

impl CarouselColors {
    /// Resolves carousel colors from the given `tokens`.
    #[must_use]
    pub fn resolve(theme: &Theme) -> Self {
        Self {
            slide_bg: theme.secondary,
            slide_border: theme.border,
            slide_text: theme.foreground,
            arrow_bg: theme.secondary,
            arrow_hover_bg: theme.secondary,
            arrow_fg: theme.muted_foreground,
            dot_inactive: theme.border,
            dot_active: theme.accent,
            focus_ring: theme.accent,
        }
    }
}

// ── Carousel ──────────────────────────────────────────────────────────────────

/// Horizontally navigable item carousel with prev/next arrows, dot indicators,
/// keyboard navigation, and optional auto-play.
///
/// See the [module-level documentation](self) for a complete usage example.
pub struct Carousel {
    /// Ordered list of item labels.
    items: Arc<Vec<SharedString>>,
    /// Zero-based index of the currently visible item.
    index: usize,
    /// Whether prev/next arrow buttons are shown.
    show_arrows: bool,
    /// Whether dot indicators are shown below the slide.
    show_dots: bool,
    /// Auto-play interval, or `None` for manual-only navigation.
    autoplay_interval: Option<Duration>,
    /// Whether navigation wraps around at the first and last item.
    looping: bool,
    /// Optional callback fired on every index change.
    on_change: Option<CarouselChangeHandler>,
    /// Dark-mode toggle.
    dark: bool,
    /// GPUI focus handle for keyboard routing.
    focus_handle: FocusHandle,
    /// Live auto-play task.  Dropping this field cancels the task.
    _autoplay_task: Option<gpui::Task<()>>,
}

impl Carousel {
    // ── construction ──────────────────────────────────────────────────────────

    /// Creates an empty carousel with sensible defaults.
    ///
    /// Chain builder methods before passing the value to `cx.new`.
    #[must_use]
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            items: Arc::new(Vec::new()),
            index: 0,
            show_arrows: true,
            show_dots: true,
            autoplay_interval: None,
            looping: true,
            on_change: None,
            dark: true,
            focus_handle: cx.focus_handle(),
            _autoplay_task: None,
        }
    }

    /// Replaces the item list.
    ///
    /// The current index is clamped to a valid position after the replacement.
    #[must_use]
    pub fn with_items(mut self, items: Vec<SharedString>) -> Self {
        let len = items.len();
        self.items = Arc::new(items);
        if len > 0 {
            self.index = self.index.min(len - 1);
        } else {
            self.index = 0;
        }
        self
    }

    /// Sets the initially visible item by zero-based index.
    ///
    /// Clamped to `[0, item_count - 1]`.
    #[must_use]
    pub fn with_index(mut self, index: usize) -> Self {
        self.index = if self.items.is_empty() {
            0
        } else {
            index.min(self.items.len() - 1)
        };
        self
    }

    /// Controls whether prev/next arrow buttons are rendered.
    #[must_use]
    pub fn show_arrows(mut self, show: bool) -> Self {
        self.show_arrows = show;
        self
    }

    /// Controls whether dot indicators are rendered below the slide.
    #[must_use]
    pub fn show_dots(mut self, show: bool) -> Self {
        self.show_dots = show;
        self
    }

    /// Enables auto-play at the given interval.
    ///
    /// The auto-play task is started lazily on the first call to
    /// [`Render::render`].  Dropping the carousel cancels the task.
    #[must_use]
    pub fn with_autoplay(mut self, interval: Duration) -> Self {
        self.autoplay_interval = Some(interval);
        self
    }

    /// Controls whether navigation wraps around at the ends (default: `true`).
    #[must_use]
    pub fn looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    /// Registers a callback invoked whenever the active index changes.
    #[must_use]
    pub fn on_change(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Selects the light (`false`) or dark (`true`) token palette.
    #[must_use]
    pub fn dark(mut self, dark: bool) -> Self {
        self.dark = dark;
        self
    }

    // ── runtime setters ───────────────────────────────────────────────────────

    /// Switches the dark/light theme at runtime and schedules a re-render.
    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        self.dark = dark;
        cx.notify();
    }

    /// Jumps directly to `idx` and schedules a re-render.
    ///
    /// The `on_change` callback is **not** fired from this path.  Use it to
    /// set the index in headless tests without needing a [`Window`].
    pub fn set_index(&mut self, idx: usize, cx: &mut Context<Self>) {
        if self.items.is_empty() {
            return;
        }
        self.index = idx.min(self.items.len() - 1);
        cx.notify();
    }

    // ── headless navigation helpers ───────────────────────────────────────────

    /// Advances to the next item.
    ///
    /// Wraps around when [`looping`](Carousel::looping) is `true`; stops at
    /// the last item otherwise.  Does **not** fire `on_change`.
    pub fn apply_next(&mut self, cx: &mut Context<Self>) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        self.index = if self.looping {
            (self.index + 1) % len
        } else {
            (self.index + 1).min(len - 1)
        };
        cx.notify();
    }

    /// Moves to the previous item.
    ///
    /// Wraps around when [`looping`](Carousel::looping) is `true`; stops at
    /// the first item otherwise.  Does **not** fire `on_change`.
    pub fn apply_prev(&mut self, cx: &mut Context<Self>) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        self.index = if self.looping {
            self.index.checked_sub(1).unwrap_or(len - 1)
        } else {
            self.index.saturating_sub(1)
        };
        cx.notify();
    }

    // ── accessors ─────────────────────────────────────────────────────────────

    /// Returns the currently visible item's zero-based index.
    #[must_use]
    pub fn current_index(&self) -> usize {
        self.index
    }

    /// Returns the total number of items.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns the label of the currently visible item, or `None` for an empty
    /// carousel.
    #[must_use]
    pub fn current_label(&self) -> Option<&str> {
        self.items.get(self.index).map(|s| s.as_ref())
    }

    /// Returns `true` if an `on_change` callback is registered.
    #[must_use]
    pub fn has_on_change(&self) -> bool {
        self.on_change.is_some()
    }

    /// Returns `true` if auto-play is enabled (interval is set).
    #[must_use]
    pub fn is_autoplay_enabled(&self) -> bool {
        self.autoplay_interval.is_some()
    }

    // ── internal helpers ──────────────────────────────────────────────────────

    /// Fires `on_change` for `index` when a [`Window`] is available.
    fn fire_change(&self, index: usize, window: &mut Window, cx: &mut App) {
        if let Some(handler) = self.on_change.clone() {
            handler(index, window, cx);
        }
    }

    /// Advances the index by one and fires `on_change`.  Called by click and
    /// keyboard handlers that have a [`Window`].
    fn go_next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        self.index = if self.looping {
            (self.index + 1) % len
        } else {
            (self.index + 1).min(len - 1)
        };
        let idx = self.index;
        cx.notify();
        self.fire_change(idx, window, cx);
    }

    /// Moves the index backward by one and fires `on_change`.  Called by click
    /// and keyboard handlers that have a [`Window`].
    fn go_prev(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        self.index = if self.looping {
            self.index.checked_sub(1).unwrap_or(len - 1)
        } else {
            self.index.saturating_sub(1)
        };
        let idx = self.index;
        cx.notify();
        self.fire_change(idx, window, cx);
    }

    /// Starts the auto-play task.  Called lazily from [`Render::render`] when
    /// `autoplay_interval` is set but `_autoplay_task` is `None`.
    fn start_autoplay(&mut self, cx: &mut Context<Self>) {
        let Some(interval) = self.autoplay_interval else {
            return;
        };
        let task = cx.spawn(async move |weak_self, cx: &mut gpui::AsyncApp| {
            loop {
                cx.background_executor().timer(interval).await;
                let cont = weak_self
                    .update(cx, |carousel, cx| {
                        carousel.apply_next(cx);
                    })
                    .is_ok();
                if !cont {
                    break;
                }
            }
        });
        self._autoplay_task = Some(task);
    }

    /// Handles `key_down` events while the carousel is focused.
    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event.keystroke.key.as_str() {
            "left" => self.go_prev(window, cx),
            "right" => self.go_next(window, cx),
            _ => {}
        }
    }
}

// ── Focusable ─────────────────────────────────────────────────────────────────

impl Focusable for Carousel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// ── Render ────────────────────────────────────────────────────────────────────

impl Render for Carousel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Start autoplay lazily on the first render if configured.
        if self.autoplay_interval.is_some() && self._autoplay_task.is_none() {
            self.start_autoplay(cx);
        }

        let theme = if self.dark {
            Theme::from(&*ThemeColor::dark())
        } else {
            Theme::from(&*ThemeColor::light())
        };
        let colors = CarouselColors::resolve(&theme);
        let focus = self.focus_handle.clone();

        let item_count = self.items.len();
        let current_index = self.index;
        let show_arrows = self.show_arrows;
        let show_dots = self.show_dots;

        // Current slide label (empty string for an empty carousel).
        let slide_label: SharedString = self
            .items
            .get(current_index)
            .cloned()
            .unwrap_or_else(|| SharedString::from(""));

        let entity = cx.entity();

        // ── Slide content ─────────────────────────────────────────────────────

        let slide = div()
            .id("carousel-slide")
            .flex()
            .flex_1()
            .items_center()
            .justify_center()
            .h(px(80.0))
            .bg(colors.slide_bg)
            .border_1()
            .border_color(colors.slide_border)
            .rounded(px(RadiusScale::default().md))
            .text_size(px(TypographyScale::default().md))
            .text_color(colors.slide_text)
            .child(slide_label);

        // ── Arrow buttons ─────────────────────────────────────────────────────

        let prev_btn = show_arrows.then(|| {
            make_arrow_button(
                ElementId::Name("carousel-prev".into()),
                "‹",
                &entity,
                &colors,
                |c, w, cx| c.go_prev(w, cx),
            )
        });
        let next_btn = show_arrows.then(|| {
            make_arrow_button(
                ElementId::Name("carousel-next".into()),
                "›",
                &entity,
                &colors,
                |c, w, cx| c.go_next(w, cx),
            )
        });

        // ── Slide row (arrows + slide) ────────────────────────────────────────

        let mut slide_row = div()
            .id("carousel-slide-row")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(SpacingScale::default().sm));

        if let Some(btn) = prev_btn {
            slide_row = slide_row.child(btn);
        }
        slide_row = slide_row.child(slide);
        if let Some(btn) = next_btn {
            slide_row = slide_row.child(btn);
        }

        // ── Dot indicators ────────────────────────────────────────────────────

        let dots_row = if show_dots && item_count > 0 {
            let mut row = div()
                .id("carousel-dots")
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .gap(px(SpacingScale::default().xs));

            for dot_idx in 0..item_count {
                let entity_dot = entity.clone();
                let is_active = dot_idx == current_index;
                let dot_color = if is_active {
                    colors.dot_active
                } else {
                    colors.dot_inactive
                };
                let dot = div()
                    .id(ElementId::Integer(dot_idx as u64 + 1_000_000))
                    .w(if is_active { px(8.0) } else { px(6.0) })
                    .h(if is_active { px(8.0) } else { px(6.0) })
                    .rounded_full()
                    .bg(dot_color)
                    .cursor_pointer()
                    .on_click(move |_ev, window, app| {
                        let maybe_handler = entity_dot.update(app, |carousel, cx| {
                            let old = carousel.index;
                            carousel.index = dot_idx;
                            if old != dot_idx {
                                cx.notify();
                                carousel.on_change.clone()
                            } else {
                                None
                            }
                        });
                        if let Some(handler) = maybe_handler {
                            handler(dot_idx, window, app);
                        }
                    });
                row = row.child(dot);
            }

            Some(row)
        } else {
            None
        };

        // ── Root container ────────────────────────────────────────────────────

        let mut root = div()
            .id("carousel-root")
            .flex()
            .flex_col()
            .gap(px(SpacingScale::default().sm))
            .track_focus(&focus)
            .on_click({
                let focus = focus.clone();
                move |_ev, window, cx| {
                    window.focus(&focus, cx);
                }
            })
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            .child(slide_row);

        if let Some(dots) = dots_row {
            root = root.child(dots);
        }

        root
    }
}

// ── arrow button helper ─────────────────────────────────────────────────────

/// Builds a prev/next arrow button for the carousel.
fn make_arrow_button(
    id: ElementId,
    glyph: &str,
    entity: &Entity<Carousel>,
    colors: &CarouselColors,
    go: impl Fn(&mut Carousel, &mut Window, &mut Context<Carousel>) + 'static,
) -> impl IntoElement {
    let entity = entity.clone();
    div()
        .id(id)
        .w(px(28.0))
        .h(px(28.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(RadiusScale::default().sm))
        .bg(colors.arrow_bg)
        .text_size(px(TypographyScale::default().md))
        .text_color(colors.arrow_fg)
        .cursor_pointer()
        .hover(move |s| s.bg(colors.arrow_hover_bg))
        .child(SharedString::from(glyph))
        .on_click(move |_ev, window, app| {
            entity.update(app, |carousel, cx| {
                go(carousel, window, cx);
            });
        })
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_tokens::color_to_hex;
    use gpui_kit::component::{Theme, ThemeColor};

    // ── construction ─────────────────────────────────────────────────────────

    #[test]
    fn carousel_colors_dark_light_differ() {
        let dark = CarouselColors::resolve(&Theme::from(&*ThemeColor::dark()));
        let light = CarouselColors::resolve(&Theme::from(&*ThemeColor::light()));
        assert_ne!(
            color_to_hex(dark.slide_bg),
            color_to_hex(light.slide_bg),
            "slide_bg must differ between light and dark"
        );
        assert_ne!(
            color_to_hex(dark.dot_active),
            color_to_hex(light.dot_active),
            "dot_active must differ between light and dark"
        );
    }
}
