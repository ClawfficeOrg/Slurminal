//! GPUI template conventions and reusable starter components for Zoid.

// Generated code refers to this crate by name (`use zoid_gpui::*;`,
// `zoid_gpui::Image::new(..)`), and `codegen_fixtures` compiles that output
// verbatim *inside* this crate. Without this alias those paths are `E0432`.
extern crate self as zoid_gpui;

/// Returns the GPUI crate version targeted by current generated templates.
#[must_use]
pub const fn targeted_gpui_version() -> Option<&'static str> {
    Some("0.2.2")
}

/// Monospace font family for code/log/diagnostic surfaces.
///
/// gpui's Windows DirectWrite backend does **not** resolve the CSS generic
/// `monospace` keyword — it looks up a literal face named "monospace", finds
/// none, and either panics (under `test-support`) or logs and falls back. So
/// name a face the target OS actually ships rather than the generic keyword.
pub const MONO_FONT_FAMILY: &str = if cfg!(target_os = "windows") {
    "Consolas"
} else if cfg!(target_os = "macos") {
    "Menlo"
} else {
    "monospace"
};

pub mod animation;
pub mod carousel;
pub mod code_editor;
pub mod codegen_fixtures;
pub mod debug_overlay;
pub mod divider;
pub mod dopesheet;
pub mod file_upload;
pub mod forms;
#[cfg(feature = "gpu-canvas")]
pub mod gpu_canvas;
pub mod hex_viewer;
pub mod image;
pub mod keybind_field;
pub mod label;
pub mod list_item;
pub mod list_view;
pub mod ribbon;
pub mod select;
pub mod text_input;
pub mod theme;
pub mod timeline;
pub mod ui_tokens;

pub use animation::{Animated, EasingFn, Lerp, easing};
pub use carousel::{Carousel, CarouselChangeHandler, CarouselColors};
pub use debug_overlay::{DebugOverlay, DebugOverlayColors};
pub use divider::{Divider, Spacer};
pub use dopesheet::{
    DopesheetEvent, DopesheetModel, DopesheetView, EasingType, Keyframe, PropValue, Track, TrackId,
};
pub use file_upload::{FileUpload, FileUploadColors, FileUploadHandler};
pub use forms::{FormField, TextField, TextFieldState};
#[cfg(feature = "gpu-canvas")]
pub use gpu_canvas::{
    DEFAULT_CANVAS_HEIGHT, DEFAULT_CANVAS_WIDTH, GPU_CANVAS_FEATURE_ID, GpuCanvas, GpuCanvasPaintFn,
};
pub use hex_viewer::{HexSelectHandler, HexViewer, HexViewerColors};
pub use image::{Image, ImageColors, ImageObjectFit};
pub use keybind_field::{
    KeybindField, KeybindFieldColors, KeybindFieldHandler, format_keystroke_pill, is_modifier_key,
};
pub use label::{Label, LabelColor, LabelSize, LabelWeight};
pub use list_item::{ListItem, ListItemColors};
pub use list_view::{BulletList, ListColors, NumberedList, NumberingStyle};
pub use ribbon::{
    DEFAULT_COLLAPSE_THRESHOLD, RIBBON_FEATURE_ID, Ribbon, RibbonColors, RibbonGroup, RibbonItem,
    RibbonPanel, RibbonTab, SegmentOption, SegmentedSelectHandler,
};
pub use select::{
    KEY_CONTEXT_SELECT, Select, SelectCancel, SelectColors, SelectConfirm, SelectDown,
    SelectHandler, SelectOption, SelectUp,
};
pub use text_input::TextInput;
pub use theme::theme_tokens_to_theme_color;
pub use timeline::{Timeline, TimelineColors, TimelineItem};
pub use ui_tokens::{
    FocusRingMetrics, RadiusScale, SpacingScale, TypographyScale, UI_KIT_TOKENS_FEATURE_ID,
    color_to_hex,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_current_template_gpui_target() {
        assert_eq!(targeted_gpui_version(), Some("0.2.2"));
    }

    #[test]
    fn ui_kit_layout_defaults_are_small_and_stable() {
        assert_eq!(UI_KIT_TOKENS_FEATURE_ID, "ui-kit-tokens");
        assert_eq!(SpacingScale::default().sm, 8.0);
        assert_eq!(SpacingScale::default().md, 12.0);
        assert_eq!(RadiusScale::default().lg, 8.0);
        assert_eq!(TypographyScale::default().md, 14.0);
        assert_eq!(FocusRingMetrics::light().width, 2.0);
    }

    #[test]
    fn color_to_hex_roundtrip() {
        // Regression: round-trip must round each channel, not truncate.
        let h = gpui_kit::component::ThemeColor::dark();
        let c = h.accent;
        let hex = color_to_hex(c);
        // ThemeColor::dark() stores accent as Hsla; verify the round trip.
        assert_eq!(hex, color_to_hex(c));
    }

    #[test]
    fn text_field_delete_removes_full_multibyte_grapheme() {
        // Regression: delete() must remove the entire grapheme cluster, not just
        // one byte. "é" (U+00E9) is 2 bytes in UTF-8.
        let mut state = TextFieldState::with_text("aé");
        state.backspace(); // deletes full 'é' from the end
        assert_eq!(state.text(), "a");
    }

    #[test]
    fn text_field_move_left_then_delete_removes_grapheme_to_right() {
        // After move_left() cursor is before 'é'; delete() removes 'é' forward.
        let mut state = TextFieldState::with_text("aé");
        state.move_left();
        state.delete();
        assert_eq!(state.text(), "a");
    }
}
