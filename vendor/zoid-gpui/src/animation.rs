//! Animation runtime for `zoid_gpui`.
//!
//! Provides [`Animated<T>`], a pure state machine that interpolates a value of
//! type `T` from a start to an end over a [`Duration`] with a configurable
//! easing curve.  The type is deliberately free of GPUI context references so
//! it can live inside any entity field without lifetime complications.
//!
//! # Driving re-renders
//!
//! `Animated<T>` does **not** call `cx.notify()` itself.  The owning view or
//! entity must call `cx.notify()` after each [`Animated::tick`] that returns
//! `Some(_)` to schedule a re-render.  Typical pattern:
//!
//! ```ignore
//! // inside a cx.spawn() loop:
//! let changed = entity.update(cx, |state, _cx| state.animation.tick(dt));
//! if changed.is_some() {
//!     entity.update(cx, |_state, cx| cx.notify());
//! }
//! ```
//!
//! # Easing
//!
//! Five curves are available in the [`easing`] submodule:
//!
//! - [`easing::linear`] — constant rate.
//! - [`easing::ease_in`] — starts slow, accelerates.
//! - [`easing::ease_out`] — starts fast, decelerates.
//! - [`easing::ease_in_out`] — smooth S-curve.
//! - [`easing::spring`] — overshoots, then settles; progress may exceed 1.0
//!   mid-animation.
//!
//! # Example
//!
//! ```
//! use std::time::Duration;
//! use zoid_gpui::animation::{Animated, easing};
//!
//! let mut anim = Animated::new(0.0_f32);
//! anim.animate_to(1.0, Duration::from_millis(300), easing::ease_in_out);
//! assert!(anim.is_running());
//!
//! let result = anim.tick(Duration::from_millis(150));
//! assert!(result.is_some());
//! let v = result.unwrap();
//! assert!(v > 0.0 && v < 1.0);
//! ```
//!
//! # GPUI version
//!
//! Implemented against `gpui 0.2.2`.

use std::time::Duration;

// ── EasingFn ─────────────────────────────────────────────────────────────────

/// A function pointer type for easing curves.
///
/// Receives a linear progress value `t ∈ [0.0, 1.0]` and returns a
/// transformed progress value.  The output may exceed `[0.0, 1.0]` for
/// overshooting curves such as [`easing::spring`].
pub type EasingFn = fn(f32) -> f32;

// ── easing submodule ──────────────────────────────────────────────────────────

/// Built-in easing functions for use with [`Animated`].
///
/// All functions accept `t ∈ [0.0, 1.0]` and return a transformed progress
/// value.  The spring easing may return values outside `[0.0, 1.0]`.
pub mod easing {
    /// Linear: constant rate, output equals input.
    #[must_use]
    pub fn linear(t: f32) -> f32 {
        t
    }

    /// Ease-in: slow start, fast finish (cubic).
    #[must_use]
    pub fn ease_in(t: f32) -> f32 {
        t * t * t
    }

    /// Ease-out: fast start, slow finish (cubic).
    #[must_use]
    pub fn ease_out(t: f32) -> f32 {
        let u = 1.0 - t;
        1.0 - u * u * u
    }

    /// Ease-in-out: slow start, fast middle, slow end (cubic S-curve).
    #[must_use]
    pub fn ease_in_out(t: f32) -> f32 {
        if t < 0.5 {
            4.0 * t * t * t
        } else {
            let u = -2.0 * t + 2.0;
            1.0 - u * u * u / 2.0
        }
    }

    /// Spring: overshoots the target, then settles.
    ///
    /// Output may exceed `1.0` near `t ≈ 0.5` before settling to `1.0` at
    /// `t = 1.0`.  Modelled after a damped harmonic oscillator.
    #[must_use]
    pub fn spring(t: f32) -> f32 {
        // Damped sine approximation: 1 - e^(-c*t) * cos(omega*t)
        // Parameters tuned for a single overshoot that settles cleanly at 1.0.
        let c = 8.0_f32;
        let omega = 12.0_f32;
        1.0 - (-c * t).exp() * (omega * t).cos()
    }
}

// ── Lerp trait ────────────────────────────────────────────────────────────────

/// Linear interpolation between two values of the same type.
///
/// Implementors must satisfy `lerp(self, other, 0.0) ≈ self` and
/// `lerp(self, other, 1.0) ≈ other`.  Values of `t` outside `[0.0, 1.0]` are
/// valid and may extrapolate beyond the range (used by spring easing).
pub trait Lerp: Copy {
    /// Interpolate between `self` (start) and `other` (end) at progress `t`.
    ///
    /// `t = 0.0` returns `self`; `t = 1.0` returns `other`.
    #[must_use]
    fn lerp(self, other: Self, t: f32) -> Self;
}

impl Lerp for f32 {
    fn lerp(self, other: Self, t: f32) -> Self {
        self + (other - self) * t
    }
}

impl Lerp for f64 {
    fn lerp(self, other: Self, t: f32) -> Self {
        self + (other - self) * f64::from(t)
    }
}

impl Lerp for gpui::Hsla {
    /// Component-wise linear interpolation in HSL+A space.
    ///
    /// Hue is interpolated directly (no shortest-arc wrapping) because the
    /// primary use-case is small colour transitions where wrapping would be
    /// unexpected.
    fn lerp(self, other: Self, t: f32) -> Self {
        gpui::hsla(
            self.h + (other.h - self.h) * t,
            self.s + (other.s - self.s) * t,
            self.l + (other.l - self.l) * t,
            self.a + (other.a - self.a) * t,
        )
    }
}

// ── Animated<T> ──────────────────────────────────────────────────────────────

/// An interpolating value that transitions from one state to another over time.
///
/// `Animated<T>` is a pure state machine.  It never touches GPUI context
/// directly; the caller is responsible for:
///
/// 1. Calling [`tick`](Animated::tick) with the elapsed delta time each frame.
/// 2. Calling `cx.notify()` when `tick` returns `Some(_)` to schedule a
///    re-render.
///
/// # Type parameter
///
/// `T` must implement [`Lerp`] and [`Copy`].  Built-in implementations are
/// provided for [`f32`], [`f64`], and [`gpui::Hsla`].
#[derive(Clone, Debug)]
pub struct Animated<T: Lerp + Copy> {
    /// Value at the beginning of the current transition.
    start: T,
    /// Target value at the end of the current transition.
    end: T,
    /// Total duration of the transition.
    duration: Duration,
    /// How much time has elapsed since the transition started.
    elapsed: Duration,
    /// Easing function applied to the linear progress before interpolating.
    easing: EasingFn,
    /// Whether a transition is currently in progress.
    running: bool,
    /// Whether any `animate_to` call has ever been issued.
    started: bool,
}

impl<T: Lerp + Copy> Animated<T> {
    /// Create a new `Animated` wrapping `value` in a non-running state.
    #[must_use]
    pub fn new(value: T) -> Self {
        Self {
            start: value,
            end: value,
            duration: Duration::ZERO,
            elapsed: Duration::ZERO,
            easing: easing::linear,
            running: false,
            started: false,
        }
    }

    /// Begin a transition from the current value to `target` over `duration`
    /// using the supplied `easing` function.
    ///
    /// If a transition is already in progress the current interpolated value
    /// becomes the new start value so the animation does not jump.
    pub fn animate_to(&mut self, target: T, duration: Duration, easing: EasingFn) {
        self.start = self.value();
        self.end = target;
        self.duration = duration;
        self.elapsed = Duration::ZERO;
        self.easing = easing;
        self.running = true;
        self.started = true;
    }

    /// Advance the animation by `dt` and return the new interpolated value, or
    /// `None` if the animation is not running.
    ///
    /// When the elapsed time reaches or exceeds the duration the animation
    /// stops (`is_running()` returns `false`) and the exact target value is
    /// returned.
    pub fn tick(&mut self, dt: Duration) -> Option<T> {
        if !self.running {
            return None;
        }
        self.elapsed = (self.elapsed + dt).min(self.duration);
        let progress = if self.duration.is_zero() {
            1.0_f32
        } else {
            self.elapsed.as_secs_f32() / self.duration.as_secs_f32()
        };
        let eased = (self.easing)(progress);
        let current = self.start.lerp(self.end, eased);
        if self.elapsed >= self.duration {
            self.running = false;
            return Some(self.end);
        }
        Some(current)
    }

    /// Return the current interpolated value without advancing the animation.
    ///
    /// Before any transition this returns the initial value passed to [`new`](Animated::new).
    /// After a transition completes this returns the target value.
    #[must_use]
    pub fn value(&self) -> T {
        if !self.running {
            if self.started {
                return self.end;
            }
            return self.start;
        }
        let progress = if self.duration.is_zero() {
            1.0_f32
        } else {
            self.elapsed.as_secs_f32() / self.duration.as_secs_f32()
        };
        let eased = (self.easing)(progress);
        self.start.lerp(self.end, eased)
    }

    /// Returns `true` while a transition is in progress.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::time::Duration;

    use super::{Animated, Lerp, easing};

    // ── 1: easing_linear_at_midpoint ─────────────────────────────────────────

    #[test]
    fn easing_linear_at_midpoint() {
        assert!((easing::linear(0.5) - 0.5).abs() < f32::EPSILON);
    }

    // ── 2: easing_ease_in_out_symmetry ───────────────────────────────────────

    #[test]
    fn easing_ease_in_out_symmetry() {
        // ease_in_out is symmetric: f(x) = 1 - f(1 - x).
        let a = easing::ease_in_out(0.25);
        let b = easing::ease_in_out(0.75);
        assert!((a - (1.0 - b)).abs() < 1e-6, "a={a} b={b}");
    }

    // ── 3: easing_spring_overshoot ───────────────────────────────────────────

    #[test]
    fn easing_spring_overshoot() {
        // Spring must overshoot (> 1.0) at some point in the middle and
        // settle close to 1.0 at t = 1.0.
        let mid = easing::spring(0.3);
        assert!(mid > 1.0, "spring should overshoot at t=0.3, got {mid}");
        let end = easing::spring(1.0);
        assert!(
            (end - 1.0).abs() < 0.01,
            "spring should settle near 1.0 at t=1.0, got {end}"
        );
    }

    // ── 4: lerp_f32_basic ────────────────────────────────────────────────────

    #[test]
    fn lerp_f32_basic() {
        assert!((1.0_f32.lerp(3.0, 0.5) - 2.0).abs() < f32::EPSILON);
    }

    // ── 5: lerp_f32_clamp ────────────────────────────────────────────────────

    #[test]
    fn lerp_f32_clamp() {
        // Lerp itself does not clamp, but at t=0 and t=1 the values are exact.
        assert!(
            (1.0_f32.lerp(3.0, 0.0) - 1.0).abs() < f32::EPSILON,
            "t=0 => start"
        );
        assert!(
            (1.0_f32.lerp(3.0, 1.0) - 3.0).abs() < f32::EPSILON,
            "t=1 => end"
        );
    }

    // ── 6: animated_new_not_running ──────────────────────────────────────────

    #[test]
    fn animated_new_not_running() {
        let anim = Animated::new(5.0_f32);
        assert!(!anim.is_running());
        assert!((anim.value() - 5.0).abs() < f32::EPSILON);
    }

    // ── 7: animated_animate_to_starts_running ────────────────────────────────

    #[test]
    fn animated_animate_to_starts_running() {
        let mut anim = Animated::new(0.0_f32);
        anim.animate_to(10.0, Duration::from_millis(100), easing::linear);
        assert!(anim.is_running());
        // value() before any tick returns start (0.0).
        assert!((anim.value() - 0.0).abs() < f32::EPSILON);
    }

    // ── 8: animated_tick_intermediate ────────────────────────────────────────

    #[test]
    fn animated_tick_intermediate() {
        let mut anim = Animated::new(0.0_f32);
        anim.animate_to(10.0, Duration::from_millis(200), easing::linear);
        let v = anim
            .tick(Duration::from_millis(100))
            .expect("should be running");
        assert!(
            (v - 5.0).abs() < 1e-5,
            "midpoint with linear easing should be 5.0, got {v}"
        );
        assert!(anim.is_running(), "still running after half duration");
    }

    // ── 9: animated_tick_completion ──────────────────────────────────────────

    #[test]
    fn animated_tick_completion() {
        let mut anim = Animated::new(0.0_f32);
        anim.animate_to(10.0, Duration::from_millis(100), easing::linear);
        let v = anim
            .tick(Duration::from_millis(100))
            .expect("should return value on completion");
        assert!(
            (v - 10.0).abs() < f32::EPSILON,
            "tick to full duration must return target"
        );
        assert!(!anim.is_running(), "animation must stop after duration");
    }

    // ── 10: animated_tick_overshoot ──────────────────────────────────────────

    #[test]
    fn animated_tick_overshoot() {
        let mut anim = Animated::new(0.0_f32);
        anim.animate_to(1.0, Duration::from_millis(1000), easing::spring);
        // At t ≈ 0.3 spring easing overshoots.
        let v = anim.tick(Duration::from_millis(300)).expect("running");
        assert!(v > 1.0, "spring easing must overshoot at 30 %, got {v}");
    }

    // ── 11: animated_tick_idempotent_after_done ───────────────────────────────

    #[test]
    fn animated_tick_idempotent_after_done() {
        let mut anim = Animated::new(0.0_f32);
        anim.animate_to(5.0, Duration::from_millis(100), easing::linear);
        // Complete the animation.
        let _ = anim.tick(Duration::from_millis(200)); // overshoot in time → clamps
        assert!(!anim.is_running());
        // Extra ticks must return None and leave value at target.
        assert!(anim.tick(Duration::from_millis(100)).is_none());
        assert!((anim.value() - 5.0).abs() < f32::EPSILON);
    }
}
