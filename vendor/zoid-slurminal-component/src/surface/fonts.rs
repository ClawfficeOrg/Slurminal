//! Nerd Font glyph coverage and fallback (task 9.2.2).
//!
//! GPUI shapes text per grapheme across a fallback chain: a glyph missing
//! from the primary face resolves out of `Font.fallbacks` instead of
//! blanking the run. This module builds that chain for the terminal:
//!
//! - The primary stays exactly what the app configured (or the platform
//!   monospace default). A configured Nerd Font name is honoured as-is.
//! - Behind it sit the common patched mono families plus the
//!   `Symbols Nerd Font` PUA block, so icon glyphs resolve per-glyph while
//!   normal text keeps the primary's metrics.
//! - The chain never contains the CSS generic `monospace` keyword, which
//!   GPUI's Windows backend looks up as a literal face name.
//!
//! GPUI 0.3.6 exposes no font-enumeration API (`TextSystem::font_id` is
//! private; `resolve_font` silently substitutes the system stack), so
//! presence is resolved lazily per glyph by the shaper — never probed
//! upfront. [`FontSelection`] documents what was selected versus what the
//! shaper will fall back through, which is the detectable/​configured split
//! the task asks to be written down. Behaviour with no Nerd Font installed
//! is defined: glyphs fall through the whole chain to the system stack or
//! `.notdef`, and row geometry never changes (cells are fixed boxes).

use gpui::{Font, FontFallbacks, FontFeatures, FontStyle, FontWeight, SharedString};

use super::{DEFAULT_MONO_FAMILY, SurfaceConfig};

/// Patched mono families tried in order behind the configured primary.
///
/// These are the install-time family names the Nerd Fonts patcher writes
/// (suffixed `Nerd Font`), plus the symbols-only face that covers the PUA
/// block for setups where the main face is unpatched.
pub const NERD_FONT_CANDIDATES: &[&str] = &[
    "JetBrainsMono Nerd Font",
    "CaskaydiaCove Nerd Font",
    "FiraCode Nerd Font",
    "Hack Nerd Font",
    "MesloLGS Nerd Font",
    "SauceCodePro Nerd Font",
    "DejaVuSansMono Nerd Font",
    "Symbols Nerd Font",
];

/// Where the primary family came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimarySource {
    /// Named explicitly by the app (ZML `font_family` or code).
    Configured,
    /// Empty configuration; fell back to [`DEFAULT_MONO_FAMILY`].
    PlatformDefault,
}

/// What [`terminal_font`] selected and why.
#[derive(Clone, Debug)]
pub struct FontSelection {
    /// Full GPUI font: primary plus the Nerd Font fallback chain.
    pub font: Font,
    /// Whether the primary was configured or defaulted.
    pub primary_source: PrimarySource,
    /// Whether the primary itself is a Nerd Font face.
    pub primary_is_nerd_font: bool,
}

/// Whether a family name denotes a Nerd Font patched face.
///
/// Matches the `Nerd Font` suffix the patcher writes, case-insensitively,
/// so `JetBrainsMono Nerd Font Mono` variants count too.
#[must_use]
pub fn is_nerd_font_family(name: &str) -> bool {
    name.to_lowercase().contains("nerd font")
}

/// Build the terminal font for a surface configuration.
///
/// The primary is the configured family (or the platform default when the
/// configuration is empty). The fallback chain is every
/// [`NERD_FONT_CANDIDATES`] entry except a duplicate of the primary, plus
/// the platform default when it is neither the primary nor already listed.
/// GPUI resolves each grapheme down this chain, so a missing icon never
/// blanks its line — and a present Nerd Font is picked up with no probing.
#[must_use]
pub fn terminal_font(config: &SurfaceConfig) -> FontSelection {
    let primary: SharedString = if config.font_family.as_ref().is_empty() {
        DEFAULT_MONO_FAMILY.into()
    } else {
        config.font_family.clone()
    };
    let primary_source = if config.font_family.as_ref().is_empty() {
        PrimarySource::PlatformDefault
    } else {
        PrimarySource::Configured
    };
    let primary_is_nerd_font = is_nerd_font_family(&primary);
    let mut chain: Vec<String> = NERD_FONT_CANDIDATES
        .iter()
        .filter(|name| **name != primary.as_ref())
        .map(ToString::to_string)
        .collect();
    if primary.as_ref() != DEFAULT_MONO_FAMILY && !chain.iter().any(|n| n == DEFAULT_MONO_FAMILY) {
        chain.push(DEFAULT_MONO_FAMILY.to_string());
    }
    FontSelection {
        font: Font {
            family: primary,
            features: FontFeatures::default(),
            fallbacks: Some(FontFallbacks::from_fonts(chain)),
            weight: FontWeight::NORMAL,
            style: FontStyle::Normal,
        },
        primary_source,
        primary_is_nerd_font,
    }
}

#[cfg(test)]
mod tests {
    use gpui::px;

    use crate::terminal::{Error, Terminal, TerminalConfig};

    use super::super::CellMetrics;
    use super::super::PaletteSource;
    use super::*;

    #[test]
    fn configured_family_stays_primary_with_nerd_chain_behind() -> Result<(), &'static str> {
        let config = SurfaceConfig {
            font_family: "Consolas".into(),
            font_size: px(14.0),
            palette: PaletteSource::Theme,
        };
        let selection = terminal_font(&config);
        assert_eq!(selection.font.family.as_ref(), "Consolas");
        assert_eq!(selection.primary_source, PrimarySource::Configured);
        assert!(!selection.primary_is_nerd_font);
        let chain = selection
            .font
            .fallbacks
            .as_ref()
            .ok_or("fallback chain must exist")?
            .fallback_list();
        assert_eq!(chain.len(), NERD_FONT_CANDIDATES.len());
        assert!(chain.contains(&"JetBrainsMono Nerd Font".to_string()));
        #[cfg(target_os = "windows")]
        assert!(
            !chain.iter().any(|n| n == "monospace"),
            "the CSS generic must never reach the Windows backend"
        );
        Ok(())
    }

    #[test]
    fn nerd_font_primary_is_kept_and_not_duplicated() -> Result<(), &'static str> {
        let config = SurfaceConfig {
            font_family: "JetBrainsMono Nerd Font".into(),
            font_size: px(14.0),
            palette: PaletteSource::Theme,
        };
        let selection = terminal_font(&config);
        assert!(selection.primary_is_nerd_font);
        let chain = selection
            .font
            .fallbacks
            .as_ref()
            .ok_or("fallback chain must exist")?
            .fallback_list();
        assert!(
            !chain.contains(&"JetBrainsMono Nerd Font".to_string()),
            "primary must not repeat in its own chain"
        );
        // Remaining candidates plus the platform default (not a candidate).
        assert_eq!(chain.len(), NERD_FONT_CANDIDATES.len());
        Ok(())
    }

    #[test]
    fn empty_configuration_falls_back_to_platform_default() {
        let config = SurfaceConfig {
            font_family: "".into(),
            font_size: px(14.0),
            palette: PaletteSource::Theme,
        };
        let selection = terminal_font(&config);
        assert_eq!(selection.font.family.as_ref(), DEFAULT_MONO_FAMILY);
        assert_eq!(selection.primary_source, PrimarySource::PlatformDefault);
    }

    #[test]
    fn nerd_font_probe_matches_patcher_names() {
        assert!(is_nerd_font_family("JetBrainsMono Nerd Font"));
        assert!(is_nerd_font_family("CaskaydiaCove Nerd Font Mono"));
        assert!(!is_nerd_font_family("Consolas"));
        assert!(!is_nerd_font_family("Menlo"));
    }

    /// Guards the Windows simdutf link fix: feeding valid multi-byte UTF-8
    /// must not abort native code.
    ///
    /// Background: `vt_write` with any complete multi-byte sequence (PUA
    /// U+E700, U+23FB, CJK, even 2-byte é) used to abort with `0xc0000005`
    /// on Windows/MSVC builds because the shared-lib link discarded
    /// simdutf's `.CRT$XCU` dynamic initializers, leaving the available
    /// list empty and the active implementation NULL. Fixed by building
    /// vendored simdutf with `-DSIMDUTF_USE_STATIC_INITIALIZATION=0`
    /// (see `GHOSTTY_SOURCE_DIR`, ClawfficeOrg/ghostty
    /// `win-vt-simdutf-fix-1.3`). This test feeds one of each shape and
    /// asserts the frame resolves with row geometry intact — it fails
    /// loudly (process abort) if the link ever regresses.
    #[test]
    fn non_ascii_feed_stays_alive() -> Result<(), Error> {
        for bytes in [
            "\u{e700}".as_bytes(),
            "\u{23fb}".as_bytes(),
            "中".as_bytes(),
            "caf\u{e9}".as_bytes(),
        ] {
            let mut term = Terminal::new(TerminalConfig {
                cols: 20,
                rows: 4,
                max_scrollback: 100,
                ..TerminalConfig::default()
            })?;
            term.feed(bytes);
            let snap = term.snapshot()?;
            let frame = super::super::grid::GridFrame::from_snapshot(&snap);
            assert_eq!(frame.rows[0].cells.len(), 20);
        }
        Ok(())
    }

    #[test]
    fn metrics_measure_fine_with_fallbacks_attached() {
        // Structural: the selection carries a chain and a measurable size.
        // The live-font assertion lives in the gpui::test below.
        let config = SurfaceConfig::default();
        let selection = terminal_font(&config);
        assert!(selection.font.fallbacks.is_some());
        let _ = CellMetrics::fallback(config.font_size);
    }

    #[gpui::test]
    fn fallback_font_resolves_and_measures(cx: &mut gpui::TestAppContext) {
        use super::super::measure_cell_metrics;

        let config = SurfaceConfig::default();
        let selection = terminal_font(&config);
        let metrics = cx.update(|cx| {
            // Must not panic even when no candidate face is installed:
            // resolve falls through to the system stack.
            let id = cx.text_system().resolve_font(&selection.font);
            let measured = measure_cell_metrics(cx, &config);
            (id, measured)
        });
        assert!(f32::from(metrics.1.cell_width) > 0.0);
        assert!(f32::from(metrics.1.cell_height) > 0.0);
    }
}
