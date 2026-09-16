//! Core data model for the dopesheet animation timeline.
//!
//! Pure data types — no GPUI context references. [`DopesheetModel`] is the
//! top-level state; [`Track`] / [`Keyframe`] / [`PropValue`] / [`EasingType`]
//! are the building blocks.

use gpui::{Hsla, SharedString};

use crate::animation::{EasingFn, Lerp, easing};

// ── EasingType ────────────────────────────────────────────────────────────────

/// Named easing curves for keyframe interpolation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EasingType {
    /// Constant rate.
    Linear,
    /// Slow start, fast finish.
    EaseIn,
    /// Fast start, slow finish.
    EaseOut,
    /// Smooth S-curve.
    EaseInOut,
    /// Overshoots target then settles.
    Spring,
}

impl EasingType {
    /// Resolve to the corresponding easing function.
    #[must_use]
    pub fn to_fn(self) -> EasingFn {
        match self {
            Self::Linear => easing::linear,
            Self::EaseIn => easing::ease_in,
            Self::EaseOut => easing::ease_out,
            Self::EaseInOut => easing::ease_in_out,
            Self::Spring => easing::spring,
        }
    }
}

// ── PropValue ─────────────────────────────────────────────────────────────────

/// The value type a keyframe holds.
#[derive(Clone, Debug, PartialEq)]
pub enum PropValue {
    /// Scalar number.
    Number(f32),
    /// Colour in HSL+A space.
    Color(Hsla),
    /// 2D position.
    Point {
        /// X coordinate.
        x: f32,
        /// Y coordinate.
        y: f32,
    },
    /// Boolean toggle.
    Bool(bool),
}

impl PropValue {
    /// Interpolate between two prop values of the same variant.
    /// Returns `None` if variants differ.
    #[must_use]
    pub fn lerp(&self, other: &Self, t: f32) -> Option<Self> {
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => Some(Self::Number(a.lerp(*b, t))),
            (Self::Color(a), Self::Color(b)) => Some(Self::Color(a.lerp(*b, t))),
            (Self::Point { x: ax, y: ay }, Self::Point { x: bx, y: by }) => Some(Self::Point {
                x: ax.lerp(*bx, t),
                y: ay.lerp(*by, t),
            }),
            (Self::Bool(a), Self::Bool(b)) => {
                // Booleans snap at midpoint.
                Some(Self::Bool(if t < 0.5 { *a } else { *b }))
            }
            _ => None,
        }
    }
}

// ── TrackId ───────────────────────────────────────────────────────────────────

/// Opaque identifier for a dopesheet track.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TrackId(u64);

static NEXT_TRACK_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl TrackId {
    /// Creates a new unique track id.
    #[must_use]
    pub fn new() -> Self {
        Self(NEXT_TRACK_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

impl Default for TrackId {
    fn default() -> Self {
        Self::new()
    }
}

// ── Keyframe ──────────────────────────────────────────────────────────────────

/// A single keyframe on a track at a specific frame with an easing curve.
#[derive(Clone, Debug, PartialEq)]
pub struct Keyframe {
    /// Frame position (0-indexed).
    pub frame: u32,
    /// Value at this keyframe.
    pub value: PropValue,
    /// Easing to apply when interpolating *from* this keyframe to the next.
    pub easing: EasingType,
}

impl Keyframe {
    /// Creates a new keyframe at `frame` with the given `value`.
    #[must_use]
    pub fn new(frame: u32, value: PropValue) -> Self {
        Self {
            frame,
            value,
            easing: EasingType::Linear,
        }
    }

    /// Sets the easing curve for this keyframe.
    #[must_use]
    pub fn with_easing(mut self, easing: EasingType) -> Self {
        self.easing = easing;
        self
    }
}

// ── Track ─────────────────────────────────────────────────────────────────────

/// A named track holding a sorted list of keyframes.
#[derive(Clone, Debug)]
pub struct Track {
    /// Unique identifier.
    pub id: TrackId,
    /// Human-readable name (e.g. "Opacity", "Position X").
    pub name: SharedString,
    /// Keyframes sorted by frame number.
    pub keyframes: Vec<Keyframe>,
    /// Display colour for the track's keyframe diamonds and spans.
    pub color: Hsla,
}

impl Track {
    /// Creates a new empty track with the given name.
    #[must_use]
    pub fn new(name: impl Into<SharedString>, color: Hsla) -> Self {
        Self {
            id: TrackId::new(),
            name: name.into(),
            keyframes: Vec::new(),
            color,
        }
    }

    /// Inserts a keyframe sorted by frame. If a keyframe already exists at the
    /// same frame, it is replaced. Returns the index of the inserted/replaced
    /// keyframe.
    pub fn upsert_keyframe(&mut self, kf: Keyframe) -> usize {
        if let Some(idx) = self.keyframes.iter().position(|k| k.frame == kf.frame) {
            self.keyframes[idx] = kf;
            idx
        } else {
            let idx = self
                .keyframes
                .binary_search_by(|k| k.frame.cmp(&kf.frame))
                .unwrap_or_else(|e| e);
            self.keyframes.insert(idx, kf);
            idx
        }
    }

    /// Removes the keyframe at the given frame. Returns `true` if one was
    /// removed.
    pub fn remove_keyframe_at(&mut self, frame: u32) -> bool {
        let before = self.keyframes.len();
        self.keyframes.retain(|k| k.frame != frame);
        self.keyframes.len() != before
    }

    /// Returns the value at a given frame, interpolating between keyframes.
    /// Returns `None` if no keyframes exist or the frame is before the first
    /// keyframe.
    #[must_use]
    pub fn value_at(&self, frame: u32) -> Option<PropValue> {
        if self.keyframes.is_empty() {
            return None;
        }
        if frame <= self.keyframes[0].frame {
            return Some(self.keyframes[0].value.clone());
        }
        let last_idx = self.keyframes.len() - 1;
        if frame >= self.keyframes[last_idx].frame {
            return Some(self.keyframes[last_idx].value.clone());
        }
        // Find the surrounding keyframes.
        for i in 0..self.keyframes.len() - 1 {
            let kf_a = &self.keyframes[i];
            let kf_b = &self.keyframes[i + 1];
            if frame >= kf_a.frame && frame <= kf_b.frame {
                let duration = kf_b.frame - kf_a.frame;
                if duration == 0 {
                    return Some(kf_b.value.clone());
                }
                let t = (frame - kf_a.frame) as f32 / duration as f32;
                let eased = (kf_a.easing.to_fn())(t);
                return kf_a.value.lerp(&kf_b.value, eased);
            }
        }
        None
    }
}

// ── DopesheetModel ──────────────────────────────────────────────────────────

/// Top-level model for a dopesheet animation timeline.
#[derive(Clone, Debug)]
pub struct DopesheetModel {
    /// Frames per second (24, 30, 60).
    pub fps: u32,
    /// Total duration in frames.
    pub total_frames: u32,
    /// Horizontal zoom: pixels per frame.
    pub zoom_px_per_frame: f32,
    /// All tracks in the sheet.
    pub tracks: Vec<Track>,
    /// Current playhead frame position.
    pub playhead_frame: u32,
    /// Whether playback is active.
    pub playing: bool,
}

impl DopesheetModel {
    /// Creates a new empty dopesheet with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            fps: 30,
            total_frames: 120,
            zoom_px_per_frame: 12.0,
            tracks: Vec::new(),
            playhead_frame: 0,
            playing: false,
        }
    }

    /// Adds a new track. Returns the track's id.
    pub fn add_track(&mut self, name: impl Into<SharedString>, color: Hsla) -> TrackId {
        let track = Track::new(name, color);
        let id = track.id;
        self.tracks.push(track);
        id
    }

    /// Removes a track by id. Returns `true` if found.
    pub fn remove_track(&mut self, id: TrackId) -> bool {
        let before = self.tracks.len();
        self.tracks.retain(|t| t.id != id);
        self.tracks.len() != before
    }

    /// Finds a track by id.
    #[must_use]
    pub fn track(&self, id: TrackId) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    /// Finds a track by id (mutable).
    #[must_use]
    pub fn track_mut(&mut self, id: TrackId) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    /// Moves the playhead to `frame`, clamped to `[0, total_frames)`.
    pub fn set_playhead(&mut self, frame: u32) {
        self.playhead_frame = frame.min(self.total_frames.saturating_sub(1));
    }

    /// Sets the total frame count. Clamps playhead if beyond new boundary.
    pub fn set_total_frames(&mut self, frames: u32) {
        self.total_frames = frames.max(1);
        self.playhead_frame = self.playhead_frame.min(self.total_frames.saturating_sub(1));
    }

    /// Total width of the timeline in pixels at current zoom.
    #[must_use]
    pub fn total_width_px(&self) -> f32 {
        self.total_frames as f32 * self.zoom_px_per_frame
    }

    /// Frame at the given x-coordinate (pixels from left).
    #[must_use]
    pub fn frame_at_x(&self, x: f32) -> u32 {
        if self.zoom_px_per_frame <= 0.0 {
            return 0;
        }
        let frame = (x / self.zoom_px_per_frame).round() as u32;
        frame.min(self.total_frames.saturating_sub(1))
    }

    /// X-coordinate (pixels) for a given frame.
    #[must_use]
    pub fn x_at_frame(&self, frame: u32) -> f32 {
        frame as f32 * self.zoom_px_per_frame
    }

    /// Returns a zoom level that fits all frames into a typical viewport
    /// width of ~800px, clamped to [4, 60].
    #[must_use]
    pub fn fit_zoom(&self) -> f32 {
        if self.total_frames == 0 {
            return 12.0;
        }
        let zoom = 800.0 / self.total_frames as f32;
        zoom.clamp(4.0, 60.0)
    }
}

impl Default for DopesheetModel {
    fn default() -> Self {
        Self::new()
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

impl PropValue {
    /// Default label string for display.
    #[must_use]
    pub fn label(&self) -> SharedString {
        match self {
            Self::Number(v) => format!("{v:.1}").into(),
            Self::Color(_) => "Color".into(),
            Self::Point { x, y } => format!("({x:.0}, {y:.0})").into(),
            Self::Bool(b) => (if *b { "true" } else { "false" }).into(),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
mod tests {
    use super::*;

    #[test]
    fn default_model_has_sane_values() {
        let m = DopesheetModel::new();
        assert_eq!(m.fps, 30);
        assert_eq!(m.total_frames, 120);
        assert!(m.zoom_px_per_frame > 0.0);
        assert!(m.tracks.is_empty());
        assert_eq!(m.playhead_frame, 0);
    }

    #[test]
    fn add_remove_track() {
        let mut m = DopesheetModel::new();
        let id = m.add_track("Opacity", gpui::red());
        assert_eq!(m.tracks.len(), 1);
        assert!(m.remove_track(id));
        assert!(m.tracks.is_empty());
    }

    #[test]
    fn playhead_clamp_upper_bound() {
        let mut m = DopesheetModel::new();
        m.set_playhead(999);
        assert_eq!(m.playhead_frame, m.total_frames - 1);
    }

    #[test]
    fn frame_at_x_rounds_correctly() {
        let m = DopesheetModel::new();
        let zoom = m.zoom_px_per_frame;
        assert_eq!(m.frame_at_x(0.0), 0);
        assert_eq!(m.frame_at_x(zoom * 5.0), 5);
        assert_eq!(m.frame_at_x(zoom * 5.0 + zoom * 0.4), 5); // round down
        assert_eq!(m.frame_at_x(zoom * 5.0 + zoom * 0.6), 6); // round up
    }

    #[test]
    fn x_at_frame_returns_correct_px() {
        let m = DopesheetModel::new();
        assert!((m.x_at_frame(10) - m.zoom_px_per_frame * 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn track_upsert_keyframe_replaces_at_same_frame() {
        let mut t = Track::new("Opacity", gpui::red());
        t.upsert_keyframe(Keyframe::new(0, PropValue::Number(0.0)));
        t.upsert_keyframe(Keyframe::new(10, PropValue::Number(1.0)));
        assert_eq!(t.keyframes.len(), 2);
        // Replace frame 10.
        t.upsert_keyframe(Keyframe::new(10, PropValue::Number(0.5)));
        assert_eq!(t.keyframes.len(), 2);
        match &t.keyframes[1].value {
            PropValue::Number(v) => assert!((v - 0.5).abs() < f32::EPSILON),
            _ => unreachable!("expected Number"),
        }
    }

    #[test]
    fn track_value_at_interpolates_linear() {
        let mut t = Track::new("Opacity", gpui::red());
        t.upsert_keyframe(Keyframe::new(0, PropValue::Number(0.0)));
        t.upsert_keyframe(Keyframe::new(10, PropValue::Number(1.0)));
        let mid = t.value_at(5);
        assert!(mid.is_some());
        match mid.unwrap() {
            PropValue::Number(v) => assert!((v - 0.5).abs() < 0.01),
            _ => unreachable!("expected Number"),
        }
    }

    #[test]
    fn track_value_at_before_first_returns_first() {
        let mut t = Track::new("Opacity", gpui::red());
        t.upsert_keyframe(Keyframe::new(10, PropValue::Number(1.0)));
        let v = t.value_at(0);
        assert!(v.is_some());
        match v.unwrap() {
            PropValue::Number(val) => assert!((val - 1.0).abs() < f32::EPSILON),
            _ => unreachable!("expected Number"),
        }
    }

    #[test]
    fn track_value_at_after_last_returns_last() {
        let mut t = Track::new("Opacity", gpui::red());
        t.upsert_keyframe(Keyframe::new(0, PropValue::Number(0.0)));
        let v = t.value_at(100);
        assert!(v.is_some());
        match v.unwrap() {
            PropValue::Number(val) => assert!((val - 0.0).abs() < f32::EPSILON),
            _ => unreachable!("expected Number"),
        }
    }

    #[test]
    fn track_remove_keyframe_at_frame() {
        let mut t = Track::new("Opacity", gpui::red());
        t.upsert_keyframe(Keyframe::new(5, PropValue::Number(1.0)));
        assert!(t.remove_keyframe_at(5));
        assert!(t.keyframes.is_empty());
    }

    #[test]
    fn prop_value_lerp_number() {
        let a = PropValue::Number(0.0);
        let b = PropValue::Number(10.0);
        let mid = a.lerp(&b, 0.5).unwrap();
        assert_eq!(mid, PropValue::Number(5.0));
    }

    #[test]
    fn prop_value_lerp_bool_snaps() {
        let a = PropValue::Bool(false);
        let b = PropValue::Bool(true);
        assert_eq!(a.lerp(&b, 0.4).unwrap(), PropValue::Bool(false));
        assert_eq!(a.lerp(&b, 0.5).unwrap(), PropValue::Bool(true));
    }

    #[test]
    fn prop_value_lerp_mismatch_returns_none() {
        let a = PropValue::Number(0.0);
        let b = PropValue::Bool(true);
        assert!(a.lerp(&b, 0.5).is_none());
    }

    #[test]
    fn easing_type_to_fn_does_not_panic() {
        for e in &[
            EasingType::Linear,
            EasingType::EaseIn,
            EasingType::EaseOut,
            EasingType::EaseInOut,
            EasingType::Spring,
        ] {
            let _ = e.to_fn()(0.5);
        }
    }

    #[test]
    fn total_width_scales_with_zoom() {
        let m = DopesheetModel::new();
        assert!((m.total_width_px() - 120.0 * m.zoom_px_per_frame).abs() < f32::EPSILON);
    }
}
