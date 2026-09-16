//! Dopesheet animation timeline — keyframe editor for animating properties.
//!
//! - [`model`] — Pure data types: [`DopesheetModel`], [`Track`], [`Keyframe`],
//!   [`PropValue`], [`EasingType`].
//! - [`view`] — [`DopesheetView`] GPUI entity with controls bar, time ruler,
//!   track list, keyframe diamonds, and playhead.

pub mod model;
pub mod view;

pub use model::{DopesheetModel, EasingType, Keyframe, PropValue, Track, TrackId};
pub use view::{DopesheetEvent, DopesheetView};
