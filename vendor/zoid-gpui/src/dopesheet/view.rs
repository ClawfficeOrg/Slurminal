//! DopesheetView — a GPUI entity that renders a dopesheet animation timeline.
//!
//! Layout (top → bottom):
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │ Controls:  ▶ ⏸ ⏭ ＋  Zoom [+][-]  Frame: 12      │
//! ├────┬────────────────────────────────────────────────┤
//! │    │ 0   5   10   15   20   25   30   35   40   45 │
//! ├────┼────────────────────────────────────────────────┤
//! │ Op │ ◆──────◆          ◆──────◆                   │
//! │ Po │ ◆──────◆                                     │
//! │ S  │ ◆─────────────────────────────────────◆       │
//! │    │            ▶ playhead                         │
//! └────┴───────────────────────────────────────────────┘
//! ```

use gpui::{
    App, Bounds, Context, EventEmitter, FocusHandle, Focusable, Hsla, IntoElement, MouseButton,
    ParentElement, Pixels, Render, SharedString, Styled, Window, canvas, div, fill, point,
    prelude::*, px, rgb, size,
};

/// Per-track data collected for canvas painting: (selected, spans, keyframe_frames, color).
type TrackPaintData = Vec<(bool, Vec<(u32, u32)>, Vec<u32>, Hsla)>;

use super::model::{DopesheetModel, Keyframe, PropValue, TrackId};

// ── constants ─────────────────────────────────────────────────────────────────

/// Height of the controls bar in pixels.
const CONTROLS_HEIGHT: f32 = 32.0;

/// Height of the time ruler in pixels.
const RULER_HEIGHT: f32 = 24.0;

/// Height of each track row in pixels.
const TRACK_ROW_HEIGHT: f32 = 28.0;

/// Width of the track-name label column in pixels.
const TRACK_LABEL_WIDTH: f32 = 100.0;

/// Dark-theme palette.
const BG_COLOR_DARK: u32 = 0x1a1d23;
const SURFACE_COLOR_DARK: u32 = 0x222530;
const BORDER_COLOR_DARK: u32 = 0x3a3d44;
const MUTED_COLOR_DARK: u32 = 0x8891a0;
const TEXT_COLOR_DARK: u32 = 0xe6e8ec;

/// Light-theme palette.
const BG_COLOR_LIGHT: u32 = 0xf5f5f5;
const SURFACE_COLOR_LIGHT: u32 = 0xe8e8e8;
const BORDER_COLOR_LIGHT: u32 = 0xd0d0d0;
const MUTED_COLOR_LIGHT: u32 = 0x888888;
const TEXT_COLOR_LIGHT: u32 = 0x333333;

// ── events ────────────────────────────────────────────────────────────────────

/// Events emitted by the `DopesheetView`.
#[derive(Clone, Debug, PartialEq)]
pub enum DopesheetEvent {
    /// The playhead was moved (by scrubbing or frame-nav).
    PlayheadMoved {
        /// New frame position.
        frame: u32,
    },
    /// A keyframe was added at the given track and frame.
    KeyframeAdded {
        /// Track the keyframe was added to.
        track: TrackId,
        /// Frame position.
        frame: u32,
    },
    /// A keyframe was removed.
    KeyframeRemoved {
        /// Track the keyframe was removed from.
        track: TrackId,
        /// Frame position.
        frame: u32,
    },
}

// ── view ──────────────────────────────────────────────────────────────────────

/// GPUI entity that renders a dopesheet animation timeline.
///
/// Owns a [`DopesheetModel`] and re-renders on model changes. The view
/// delegates all state mutations to the model — it never writes state
/// directly outside of user-driven events.
pub struct DopesheetView {
    /// GPUI focus handle for keyboard input.
    focus_handle: FocusHandle,
    /// The backing data model.
    pub model: DopesheetModel,
    /// The currently selected track id, if any.
    pub selected_track: Option<TrackId>,
    /// The currently selected keyframe frame, if any.
    pub selected_keyframe_frame: Option<u32>,
    /// Whether the view uses dark-theme colours.
    pub dark: bool,
}

impl DopesheetView {
    /// Creates a new dopesheet view backed by `model`.
    pub fn new(model: DopesheetModel, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            model,
            selected_track: None,
            selected_keyframe_frame: None,
            dark: true,
        }
    }

    /// Sets the dark theme flag and notifies.
    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        self.dark = dark;
        cx.notify();
    }

    /// Resolves the 5-colour palette for the current `dark` flag.
    fn colors(&self) -> (u32, u32, u32, u32, u32) {
        if self.dark {
            (
                BG_COLOR_DARK,
                SURFACE_COLOR_DARK,
                BORDER_COLOR_DARK,
                MUTED_COLOR_DARK,
                TEXT_COLOR_DARK,
            )
        } else {
            (
                BG_COLOR_LIGHT,
                SURFACE_COLOR_LIGHT,
                BORDER_COLOR_LIGHT,
                MUTED_COLOR_LIGHT,
                TEXT_COLOR_LIGHT,
            )
        }
    }

    // ── render zones ──────────────────────────────────────────────────────

    /// Renders the top controls bar: play/pause, frame nav, zoom, frame display.
    fn render_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (_bg, surface, border, muted, text) = self.colors();
        let playing = self.model.playing;
        let frame_str = format!("Frame: {}", self.model.playhead_frame);

        div()
            .h(px(CONTROLS_HEIGHT))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .px_2()
            .bg(gpui::rgb(surface))
            .border_b_1()
            .border_color(gpui::rgb(border))
            // Play/Pause button
            .child(
                div()
                    .id("dopesheet-play")
                    .cursor_pointer()
                    .text_size(px(14.0))
                    .text_color(gpui::rgb(text))
                    .child(if playing { "⏸" } else { "▶" })
                    .on_click(cx.listener(move |this, _ev, _w, cx| {
                        this.model.playing = !this.model.playing;
                        cx.notify();
                    })),
            )
            // Previous keyframe
            .child(
                div()
                    .id("dope-prev-kf")
                    .cursor_pointer()
                    .px_1()
                    .py_0p5()
                    .rounded_sm()
                    .hover(|el| el.bg(gpui::rgb(0x2c2f38)))
                    .text_size(px(12.0))
                    .text_color(gpui::rgb(text))
                    .child("⏮")
                    .on_click(cx.listener(move |this, _ev, _w, cx| {
                        let cur = this.model.playhead_frame;
                        let prev = this
                            .model
                            .tracks
                            .iter()
                            .flat_map(|t| t.keyframes.iter().map(|k| k.frame))
                            .filter(|&f| f < cur)
                            .max()
                            .unwrap_or(0);
                        this.model.set_playhead(prev);
                        cx.notify();
                    })),
            )
            // Previous frame
            .child(
                div()
                    .id("dope-prev-fr")
                    .cursor_pointer()
                    .px_1()
                    .py_0p5()
                    .rounded_sm()
                    .hover(|el| el.bg(gpui::rgb(0x2c2f38)))
                    .text_size(px(12.0))
                    .text_color(gpui::rgb(text))
                    .child("◀")
                    .on_click(cx.listener(move |this, _ev, _w, cx| {
                        if this.model.playhead_frame > 0 {
                            this.model.set_playhead(this.model.playhead_frame - 1);
                            cx.notify();
                        }
                    })),
            )
            // Next frame
            .child(
                div()
                    .id("dope-next-fr")
                    .cursor_pointer()
                    .px_1()
                    .py_0p5()
                    .rounded_sm()
                    .hover(|el| el.bg(gpui::rgb(0x2c2f38)))
                    .text_size(px(12.0))
                    .text_color(gpui::rgb(text))
                    .child("▶")
                    .on_click(cx.listener(move |this, _ev, _w, cx| {
                        if this.model.playhead_frame + 1 < this.model.total_frames {
                            this.model.set_playhead(this.model.playhead_frame + 1);
                            cx.notify();
                        }
                    })),
            )
            // Next keyframe
            .child(
                div()
                    .id("dope-next-kf")
                    .cursor_pointer()
                    .px_1()
                    .py_0p5()
                    .rounded_sm()
                    .hover(|el| el.bg(gpui::rgb(0x2c2f38)))
                    .text_size(px(12.0))
                    .text_color(gpui::rgb(text))
                    .child("⏭")
                    .on_click(cx.listener(move |this, _ev, _w, cx| {
                        let cur = this.model.playhead_frame;
                        let next = this
                            .model
                            .tracks
                            .iter()
                            .flat_map(|t| t.keyframes.iter().map(|k| k.frame))
                            .filter(|&f| f > cur)
                            .min()
                            .unwrap_or(cur);
                        this.model.set_playhead(next);
                        cx.notify();
                    })),
            )
            // Separator
            .child(div().w(px(8.0)))
            // Add keyframe at playhead on selected track
            .child(
                div()
                    .id("dope-add-kf")
                    .cursor_pointer()
                    .px_1()
                    .py_0p5()
                    .rounded_sm()
                    .hover(|el| el.bg(gpui::rgb(0x2c2f38)))
                    .text_size(px(13.0))
                    .text_color(gpui::rgb(text))
                    .child("+")
                    .on_click(cx.listener(move |this, _ev, _w, cx| {
                        if let Some(tid) = this.selected_track {
                            let frame = this.model.playhead_frame;
                            if let Some(track) = this.model.track_mut(tid) {
                                track.upsert_keyframe(Keyframe::new(frame, PropValue::Number(0.0)));
                                this.selected_keyframe_frame = Some(frame);
                                cx.notify();
                            }
                        }
                    })),
            )
            // Delete selected keyframe
            .child(
                div()
                    .id("dope-del-kf")
                    .cursor_pointer()
                    .px_1()
                    .py_0p5()
                    .rounded_sm()
                    .hover(|el| el.bg(gpui::rgb(0x2c2f38)))
                    .text_size(px(13.0))
                    .text_color(gpui::rgb(text))
                    .child("−")
                    .on_click(cx.listener(move |this, _ev, _w, cx| {
                        if let Some(tid) = this.selected_track
                            && let Some(frame) = this.selected_keyframe_frame
                            && this
                                .model
                                .track_mut(tid)
                                .is_some_and(|t| t.remove_keyframe_at(frame))
                        {
                            this.selected_keyframe_frame = None;
                        }
                        cx.notify();
                    })),
            )
            // Frame display
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(gpui::rgb(muted))
                    .child(frame_str),
            )
            // Separator
            .child(div().flex_grow(1.0))
            // Track name hint
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(gpui::rgb(muted))
                    .child(match self.selected_track {
                        Some(tid) => self
                            .model
                            .track(tid)
                            .map(|t| t.name.clone())
                            .unwrap_or_default(),
                        None => "No track selected".into(),
                    }),
            )
            // Zoom in
            .child(
                div()
                    .id("dope-zoom-in")
                    .cursor_pointer()
                    .px_1()
                    .py_0p5()
                    .rounded_sm()
                    .hover(|el| el.bg(gpui::rgb(0x2c2f38)))
                    .text_size(px(12.0))
                    .text_color(gpui::rgb(text))
                    .child("🔍+")
                    .on_click(cx.listener(move |this, _ev, _w, cx| {
                        this.model.zoom_px_per_frame =
                            (this.model.zoom_px_per_frame * 1.3).min(60.0);
                        cx.notify();
                    })),
            )
            // Zoom out
            .child(
                div()
                    .id("dope-zoom-out")
                    .cursor_pointer()
                    .px_1()
                    .py_0p5()
                    .rounded_sm()
                    .hover(|el| el.bg(gpui::rgb(0x2c2f38)))
                    .text_size(px(12.0))
                    .text_color(gpui::rgb(text))
                    .child("🔍−")
                    .on_click(cx.listener(move |this, _ev, _w, cx| {
                        this.model.zoom_px_per_frame =
                            (this.model.zoom_px_per_frame / 1.3).max(4.0);
                        cx.notify();
                    })),
            )
            // Fit to content
            .child(
                div()
                    .id("dope-fit")
                    .cursor_pointer()
                    .px_1()
                    .py_0p5()
                    .rounded_sm()
                    .hover(|el| el.bg(gpui::rgb(0x2c2f38)))
                    .text_size(px(12.0))
                    .text_color(gpui::rgb(text))
                    .child("Fit")
                    .on_click(cx.listener(move |this, _ev, _w, cx| {
                        this.model.zoom_px_per_frame = this.model.fit_zoom();
                        cx.notify();
                    })),
            )
    }

    /// Renders the time ruler with frame markers and click-to-scrub.
    fn render_ruler(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (_bg, surface, border, muted, _text) = self.colors();
        let zoom = self.model.zoom_px_per_frame;
        let total_frames = self.model.total_frames;
        let total_w = total_frames as f32 * zoom;

        let mut row = div()
            .h(px(RULER_HEIGHT))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .bg(gpui::rgb(surface))
            .border_b_1()
            .border_color(gpui::rgb(border));

        // Track label column (empty header area)
        row = row.child(
            div()
                .w(px(TRACK_LABEL_WIDTH))
                .flex_shrink_0()
                .h_full()
                .border_r_1()
                .border_color(gpui::rgb(border)),
        );

        // Frame markers
        row = row.child(
            div()
                .h_full()
                .w(px(total_w))
                .flex()
                .flex_row()
                .relative()
                .children((0..total_frames).step_by(5).map(|f| {
                    let x = f as f32 * zoom;
                    div()
                        .absolute()
                        .left(px(x))
                        .top(px(2.0))
                        .text_size(px(9.0))
                        .text_color(gpui::rgb(muted))
                        .child(f.to_string())
                }))
                // Click overlay for scrubbing
                .child(
                    div()
                        .absolute()
                        .top(px(0.0))
                        .left(px(0.0))
                        .w(px(total_w))
                        .h(px(RULER_HEIGHT))
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, ev: &gpui::MouseDownEvent, _w, cx| {
                                let x = f32::from(ev.position.x);
                                let frame = this.model.frame_at_x(x);
                                this.model.set_playhead(frame);
                                cx.notify();
                            }),
                        ),
                ),
        );

        row
    }

    /// Renders the track area (label column + canvas with grid, spans, keyframes, playhead).
    fn render_track_area(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (_bg, _surface, border, _muted, text) = self.colors();
        let zoom = self.model.zoom_px_per_frame;
        let total_frames = self.model.total_frames;
        let total_w = total_frames as f32 * zoom;
        let total_h = self.model.tracks.len() as f32 * TRACK_ROW_HEIGHT;
        let track_count = self.model.tracks.len();
        let playhead_frame = self.model.playhead_frame;
        let selected_track = self.selected_track;

        // Collect track data needed for canvas painting.
        let sel_keyframe = self.selected_keyframe_frame;
        let track_data: TrackPaintData = self
            .model
            .tracks
            .iter()
            .map(|t| {
                let is_sel = selected_track == Some(t.id);
                let spans: Vec<(u32, u32)> = t
                    .keyframes
                    .windows(2)
                    .map(|w| (w[0].frame, w[1].frame))
                    .collect();
                let kf_frames: Vec<u32> = t.keyframes.iter().map(|k| k.frame).collect();
                (is_sel, spans, kf_frames, t.color)
            })
            .collect();

        // Track labels (left column)
        let track_items: Vec<(TrackId, SharedString)> = self
            .model
            .tracks
            .iter()
            .map(|t| (t.id, t.name.clone()))
            .collect();
        let sel_track = self.selected_track;
        let mut labels = div()
            .w(px(TRACK_LABEL_WIDTH))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col();
        for (tid, name) in track_items {
            let label_id = format!("dope-track-{:?}", tid);
            let bg = if sel_track == Some(tid) {
                gpui::hsla(0.6, 0.3, 0.2, 0.15)
            } else {
                gpui::hsla(0.0, 0.0, 0.0, 0.0)
            };
            labels = labels.child(
                div()
                    .id(label_id)
                    .h(px(TRACK_ROW_HEIGHT))
                    .w_full()
                    .flex()
                    .items_center()
                    .px_1()
                    .bg(bg)
                    .border_b_1()
                    .border_color(gpui::rgb(border))
                    .text_size(px(11.0))
                    .text_color(gpui::rgb(text))
                    .child(name)
                    .on_click(cx.listener(move |this, _ev, _w, cx| {
                        this.selected_track = Some(tid);
                        this.selected_keyframe_frame = None;
                        cx.notify();
                    })),
            );
        }

        // Canvas for grid, spans, keyframes, selection highlights.
        let canvas_layer = canvas(
            move |_viewport: Bounds<Pixels>, _window: &mut Window, _app: &mut App| {
                (
                    track_data,
                    playhead_frame,
                    sel_keyframe,
                    total_frames,
                    zoom,
                    track_count,
                )
            },
            move |viewport: Bounds<Pixels>,
                  (ref td, ph, sel_kf, total_fr, z, tc): _,
                  window: &mut Window,
                  _app: &mut App| {
                let ox = f32::from(viewport.origin.x);
                let oy = f32::from(viewport.origin.y);
                let vw = f32::from(viewport.size.width);
                let vh = f32::from(viewport.size.height);

                // Horizontal grid lines between tracks
                for t in 0..=tc {
                    let y = oy + t as f32 * TRACK_ROW_HEIGHT - 0.5;
                    window.paint_quad(fill(
                        Bounds {
                            origin: point(px(ox), px(y)),
                            size: size(px(vw), px(1.0)),
                        },
                        rgb(0x2a2d34),
                    ));
                }

                // Vertical frame markers every 5 frames
                for f in (0..=total_fr).step_by(5) {
                    let x = ox + f as f32 * z - 0.5;
                    window.paint_quad(fill(
                        Bounds {
                            origin: point(px(x), px(oy)),
                            size: size(px(1.0), px(vh)),
                        },
                        rgb(0x2a2d34),
                    ));
                }

                // Selection backgrounds, tween spans, keyframe diamonds
                for (t_idx, (is_sel, spans, kf_frames, color)) in td.iter().enumerate() {
                    let track_y = oy + t_idx as f32 * TRACK_ROW_HEIGHT;
                    let mid_y = track_y + TRACK_ROW_HEIGHT / 2.0;

                    if *is_sel {
                        window.paint_quad(fill(
                            Bounds {
                                origin: point(px(ox), px(track_y)),
                                size: size(px(vw), px(TRACK_ROW_HEIGHT)),
                            },
                            gpui::hsla(0.6, 0.3, 0.2, 0.15),
                        ));
                    }

                    // Tween spans between consecutive keyframes
                    for (sf, ef) in spans {
                        let x1 = ox + *sf as f32 * z;
                        let x2 = ox + *ef as f32 * z;
                        let span_w = (x2 - x1).max(2.0);
                        window.paint_quad(fill(
                            Bounds {
                                origin: point(px(x1), px(mid_y - 2.0)),
                                size: size(px(span_w), px(4.0)),
                            },
                            *color,
                        ));
                    }

                    // Keyframe diamonds
                    for &kf_frame in kf_frames {
                        let kx = ox + kf_frame as f32 * z;
                        let is_selected_kf = sel_kf == Some(kf_frame) && *is_sel;
                        let diamond_size: f32 = if is_selected_kf { 10.0 } else { 7.0 };
                        let half = diamond_size / 2.0;
                        // Diamond: a rotated square via centered paint_quad.
                        // We approximate as a filled square at 45° visual; since
                        // GPUI only paints axis-aligned quads, draw a small square.
                        let d_bounds = Bounds {
                            origin: point(px(kx - half), px(mid_y - half)),
                            size: size(px(diamond_size), px(diamond_size)),
                        };
                        if is_selected_kf {
                            // Selected: draw ring (larger outline) + inner fill
                            window.paint_quad(fill(d_bounds, gpui::rgb(0xffffff)));
                            let inner = Bounds {
                                origin: point(px(kx - half + 2.0), px(mid_y - half + 2.0)),
                                size: size(px(diamond_size - 4.0), px(diamond_size - 4.0)),
                            };
                            window.paint_quad(fill(inner, *color));
                        } else {
                            window.paint_quad(fill(d_bounds, *color));
                        }
                    }
                }

                // Playhead line
                let phx = ox + ph as f32 * z;
                window.paint_quad(fill(
                    Bounds {
                        origin: point(px(phx - 0.5), px(oy)),
                        size: size(px(2.0), px(vh)),
                    },
                    rgb(0xff4444),
                ));
            },
        )
        .w(px(total_w))
        .h(px(total_h));

        div().w_full().flex().flex_row().child(labels).child(
            div()
                .relative()
                .child(canvas_layer)
                // Scrubbing overlay: click on track area moves playhead / selects
                // keyframe / selects track.
                .child(
                    div()
                        .absolute()
                        .top(px(0.0))
                        .left(px(0.0))
                        .w(px(total_w))
                        .h(px(total_h))
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, ev: &gpui::MouseDownEvent, _w, cx| {
                                let x = f32::from(ev.position.x);
                                let y = f32::from(ev.position.y);
                                let frame = this.model.frame_at_x(x);
                                let track_idx = (y / TRACK_ROW_HEIGHT) as usize;
                                let zoom = this.model.zoom_px_per_frame;
                                let hit_radius = 6.0_f32;

                                // Select track by y position.
                                if track_idx < this.model.tracks.len() {
                                    let tid = this.model.tracks[track_idx].id;
                                    this.selected_track = Some(tid);

                                    // Check if click is near a keyframe on this track.
                                    let hit_kf =
                                        this.model.tracks[track_idx].keyframes.iter().find(|k| {
                                            let kx = k.frame as f32 * zoom;
                                            (x - kx).abs() < hit_radius
                                        });
                                    if let Some(kf) = hit_kf {
                                        this.selected_keyframe_frame = Some(kf.frame);
                                    } else {
                                        this.selected_keyframe_frame = None;
                                    }
                                }

                                this.model.set_playhead(frame);
                                cx.notify();
                            }),
                        ),
                ),
        )
    }
}

impl Focusable for DopesheetView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<DopesheetEvent> for DopesheetView {}

impl Render for DopesheetView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (bg, _surface, _border, _muted, _text) = self.colors();
        div()
            .id("dopesheet-view")
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .size_full()
            .bg(gpui::rgb(bg))
            .child(self.render_controls(cx))
            .child(self.render_ruler(cx))
            .child(
                div()
                    .id("dopesheet-track-scroll")
                    .flex()
                    .flex_row()
                    .flex_grow(1.0)
                    .overflow_x_scroll()
                    .child(self.render_track_area(cx)),
            )
    }
}

#[cfg(test)]
#[allow(missing_docs, clippy::unwrap_used)]
mod tests {
    use gpui::TestAppContext;

    use super::*;

    #[gpui::test]
    fn renders_controls_ruler_and_tracks(cx: &mut TestAppContext) {
        let mut model = DopesheetModel::new();
        model.add_track("Opacity", gpui::red());
        model.add_track("Position", gpui::blue());
        let _view = cx.update(|cx| cx.new(|cx| DopesheetView::new(model, cx)));
    }

    #[gpui::test]
    fn render_with_keyframes(cx: &mut TestAppContext) {
        let mut model = DopesheetModel::new();
        let tid = model.add_track("Opacity", gpui::red());
        let track = model.track_mut(tid).unwrap();
        track.upsert_keyframe(Keyframe::new(0, PropValue::Number(0.0)));
        track.upsert_keyframe(Keyframe::new(30, PropValue::Number(1.0)));
        let _view = cx.update(|cx| cx.new(|cx| DopesheetView::new(model, cx)));
    }

    #[gpui::test]
    fn default_model_empty_tracks(cx: &mut TestAppContext) {
        let model = DopesheetModel::new();
        assert!(model.tracks.is_empty());
        let _view = cx.update(|cx| cx.new(|cx| DopesheetView::new(model, cx)));
    }
}
