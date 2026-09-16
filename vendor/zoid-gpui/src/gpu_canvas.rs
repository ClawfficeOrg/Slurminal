//! [`GpuCanvas`] — a GPUI view exposing a low-level GPU paint surface.
//!
//! # Feature gate
//!
//! This module is only compiled when the `gpu-canvas` crate feature is
//! enabled.
//!
//! # Spike findings (task 3.3.4)
//!
//! GPUI 0.2.2 does **not** expose `blade-graphics` types (swapchain frames,
//! raw command encoders, surface handles) in its public API.  The
//! `blade-graphics` crate is an *optional internal* implementation detail of
//! GPUI's rendering backend; its types never appear in the public `gpui::`
//! namespace.
//!
//! The `zed-industries/zed/crates/gpui/src/platform/blade/` directory was
//! reviewed (2026-06-02) and confirmed that the blade renderer is fully
//! encapsulated: there is no public `Frame`, `Surface`, or `Swapchain` type
//! exported from the `gpui` crate.
//!
//! The highest-fidelity GPU paint surface available through the public GPUI
//! 0.2.2 API is the [`gpui::canvas`] element, which gives callers direct
//! access to [`Window::paint_quad`] and [`Window::paint_path`] — the same
//! paint primitives that GPUI's own blade back-end ultimately invokes.
//!
//! `GpuCanvas` therefore wraps `gpui::canvas()` and exposes a user-supplied
//! [`GpuCanvasPaintFn`] callback.  This is the correct practical
//! implementation: it provides the lowest-level GPU drawing surface accessible
//! to GPUI user code.  When GPUI exposes a public blade-surface API in a
//! future version, this component can be updated without changing its external
//! interface.
//!
//! # Example
//!
//! ```rust,ignore
//! use gpui::{fill, hsla};
//! use zoid_gpui::gpu_canvas::GpuCanvas;
//!
//! let canvas = cx.new(|_cx| {
//!     GpuCanvas::new(280.0, 160.0)
//!         .with_paint(|bounds, window, _app| {
//!             // Fill the canvas surface with a solid cornflower blue.
//!             window.paint_quad(fill(bounds, hsla(0.61, 0.79, 0.64, 1.0)));
//!         })
//!         .dark(true)
//! });
//! ```
//!
//! # GPUI version
//!
//! Implemented against `gpui 0.2.2`.

use std::rc::Rc;

use gpui::{
    App, Bounds, Context, Hsla, IntoElement, Pixels, Render, SharedString, Window, canvas, div,
    fill, hsla, prelude::*, px,
};

use crate::ui_tokens::RadiusScale;
use gpui_kit::component::ThemeColor;

/// Stable feature identifier for the `gpu-canvas` crate feature.
pub const GPU_CANVAS_FEATURE_ID: &str = "gpu-canvas";

/// Default canvas width in logical pixels.
pub const DEFAULT_CANVAS_WIDTH: f32 = 280.0;

/// Default canvas height in logical pixels.
pub const DEFAULT_CANVAS_HEIGHT: f32 = 160.0;

/// User-supplied per-frame GPU paint callback.
///
/// The closure receives the resolved window-space [`Bounds<Pixels>`] for the
/// canvas surface, a `&mut Window` for issuing paint commands, and a `&mut App`
/// for accessing GPUI entities.  It is invoked once per render frame while the
/// component is visible.
pub type GpuCanvasPaintFn = Rc<dyn Fn(Bounds<Pixels>, &mut Window, &mut App) + 'static>;

/// A GPUI view that exposes a low-level GPU paint surface via a user-supplied
/// callback.
///
/// `GpuCanvas` wraps GPUI's [`canvas`] element and forwards the per-frame
/// paint callback to the caller.  The callback receives the resolved pixel
/// [`Bounds`] and a `&mut Window`, allowing any paint primitive supported by
/// GPUI (`paint_quad`, `paint_path`, etc.) to be issued directly onto the GPU
/// command stream.
///
/// When no custom callback is attached, the canvas paints a solid-color
/// placeholder (see [`with_fill_color`](GpuCanvas::with_fill_color)).
///
/// # Spike note
///
/// GPUI 0.2.2 does not expose raw `blade-graphics` frame or swapchain handles
/// in its public API.  See the [module documentation](self) for the full spike
/// findings.
///
/// # Feature gate
///
/// Compiled only when the `gpu-canvas` crate feature is enabled.
pub struct GpuCanvas {
    /// Canvas width in logical pixels.
    width: f32,
    /// Canvas height in logical pixels.
    height: f32,
    /// Dark-mode palette toggle.
    dark: bool,
    /// Optional user-supplied paint callback.  When `None`, a solid-color
    /// placeholder fill is painted instead.
    paint_fn: Option<GpuCanvasPaintFn>,
    /// Solid fill color used as the default placeholder when no custom paint
    /// function is supplied.
    fill_color: Hsla,
}

impl Default for GpuCanvas {
    fn default() -> Self {
        Self::new(DEFAULT_CANVAS_WIDTH, DEFAULT_CANVAS_HEIGHT)
    }
}

impl GpuCanvas {
    /// Creates a new canvas at the given logical-pixel size.
    ///
    /// No custom paint function is attached; the canvas will paint a
    /// solid-color placeholder until [`with_paint`](Self::with_paint) is
    /// called.
    ///
    /// The minimum supported size is `32 × 32` logical pixels.
    #[must_use]
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            width: width.max(32.0),
            height: height.max(32.0),
            dark: true,
            paint_fn: None,
            // Cornflower blue: a neutral, recognisable canvas placeholder.
            fill_color: hsla(0.61, 0.79, 0.64, 1.0),
        }
    }

    /// Attaches a custom GPU paint callback.
    ///
    /// The callback receives the resolved window-space [`Bounds<Pixels>`] for
    /// the canvas surface, a `&mut Window` for issuing paint commands, and a
    /// `&mut App` for accessing GPUI entities.
    ///
    /// Any previously attached callback is replaced.
    #[must_use]
    pub fn with_paint<F>(mut self, f: F) -> Self
    where
        F: Fn(Bounds<Pixels>, &mut Window, &mut App) + 'static,
    {
        self.paint_fn = Some(Rc::new(f));
        self
    }

    /// Sets the placeholder solid-fill color shown when no custom paint
    /// callback is attached.
    ///
    /// `h` is hue in `[0, 1]`, `s` saturation, `l` lightness, `a` alpha.
    /// All components are clamped to `[0.0, 1.0]`.
    #[must_use]
    pub fn with_fill_color(mut self, h: f32, s: f32, l: f32, a: f32) -> Self {
        self.fill_color = hsla(
            h.clamp(0.0, 1.0),
            s.clamp(0.0, 1.0),
            l.clamp(0.0, 1.0),
            a.clamp(0.0, 1.0),
        );
        self
    }

    /// Selects the light (`false`) or dark (`true`) token palette for the
    /// canvas border and background chrome.
    #[must_use]
    pub fn dark(mut self, dark: bool) -> Self {
        self.dark = dark;
        self
    }

    /// Switches the dark/light palette at runtime and schedules a re-render.
    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        self.dark = dark;
        cx.notify();
    }

    /// Returns the canvas width in logical pixels.
    #[must_use]
    pub fn canvas_width(&self) -> f32 {
        self.width
    }

    /// Returns the canvas height in logical pixels.
    #[must_use]
    pub fn canvas_height(&self) -> f32 {
        self.height
    }

    /// Returns `true` when the dark palette is active.
    #[must_use]
    pub fn is_dark(&self) -> bool {
        self.dark
    }

    /// Returns the placeholder solid-fill color.
    #[must_use]
    pub fn fill_color(&self) -> Hsla {
        self.fill_color
    }

    /// Returns `true` when a custom paint callback has been attached via
    /// [`with_paint`](Self::with_paint).
    #[must_use]
    pub fn has_paint_fn(&self) -> bool {
        self.paint_fn.is_some()
    }
}

impl Render for GpuCanvas {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = if self.dark {
            *ThemeColor::dark()
        } else {
            *ThemeColor::light()
        };

        let width = self.width;
        let height = self.height;
        let fill_color = self.fill_color;
        let paint_fn = self.paint_fn.clone();

        let surface = canvas(
            move |_bounds, _window, _app| (),
            move |bounds, (), window, app| {
                if let Some(ref f) = paint_fn {
                    f(bounds, window, app);
                } else {
                    // Default solid-color fill placeholder.
                    window.paint_quad(fill(bounds, fill_color));
                }
            },
        )
        .w(px(width))
        .h(px(height));

        let label_color = theme.muted_foreground;

        div()
            .id("gpu-canvas")
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(label_color)
                    .child(SharedString::from(format!(
                        "GpuCanvas {}×{} px  ·  GPUI canvas() paint surface",
                        width as u32, height as u32,
                    ))),
            )
            .child(
                div()
                    .relative()
                    .w(px(width))
                    .h(px(height))
                    .rounded(px(RadiusScale::default().md))
                    .overflow_hidden()
                    .border_1()
                    .border_color(theme.border)
                    .child(surface),
            )
    }
}
