//! `FileUpload` component for generated GPUI starters.
//!
//! [`FileUpload`] is a GPUI `View` that renders a drop zone and a **Browse…**
//! button.  Users can either drag files from their file manager onto the drop
//! zone or click the button to open the platform native file-picker dialog.
//!
//! ## Features
//!
//! - **Drop zone** — accepts GPUI [`ExternalPaths`] drag-and-drop events.
//! - **Browse button** — opens `prompt_for_paths` via the platform dialog.
//! - **Accept filter** — optional extension list (e.g. `["rs", "toml"]`).
//!   Paths whose extension does not match are silently dropped.  Applied
//!   client-side after selection because [`PathPromptOptions`] in GPUI 0.2.2
//!   does not expose file-type filters.
//! - **Multiple flag** — when `false`, only the first accepted path is kept.
//! - **Callback** — `on_files_selected(Vec<PathBuf>)` fires whenever accepted
//!   paths are committed from a drop event.
//!
//! ## Headless-test helpers
//!
//! Because the `on_files_selected` callback requires `&mut Window`, production
//! firing happens inside the `on_drop` GPUI listener (which provides a Window).
//! Tests that run against [`gpui::TestAppContext`] (no real Window) should use
//! [`FileUpload::apply_files_silent`] to inject paths and verify state without
//! needing to fire the callback.
//!
//! ## Example
//!
//! ```rust,ignore
//! let upload = cx.new(|cx| {
//!     FileUpload::new(cx)
//!         .with_accept(vec!["rs".to_owned(), "toml".to_owned()])
//!         .multiple(true)
//!         .on_files_selected(|paths, _window, _cx| println!("selected: {paths:?}"))
//!         .dark(true)
//! });
//! ```
//!
//! ## GPUI version
//!
//! Implemented against `gpui 0.2.2`.  Accept filtering is post-selection only;
//! [`PathPromptOptions`] carries no file-type restriction in this release.

use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    App, AsyncApp, Context, ExternalPaths, IntoElement, PathPromptOptions, Render, SharedString,
    Window, div, prelude::*, px,
};

use crate::ui_tokens::{FocusRingMetrics, RadiusScale, TypographyScale};
use gpui_kit::component::{Theme, ThemeColor};

// ── FileUploadColors ──────────────────────────────────────────────────────────

/// Token-resolved colors for a [`FileUpload`] component.
///
/// Exposed so unit tests can verify token binding without a GPUI render
/// context.
#[derive(Clone, Copy, Debug)]
pub struct FileUploadColors {
    /// Drop zone background at rest.
    pub zone_bg: gpui::Hsla,
    /// Drop zone border at rest.
    pub zone_border: gpui::Hsla,
    /// Drop zone label text.
    pub zone_text: gpui::Hsla,
    /// Drop zone background while files are being dragged over it.
    pub zone_bg_active: gpui::Hsla,
    /// Drop zone border while files are being dragged over it.
    pub zone_border_active: gpui::Hsla,
    /// Browse button background.
    pub button_bg: gpui::Hsla,
    /// Browse button text.
    pub button_text: gpui::Hsla,
    /// Selected-file entry background.
    pub file_bg: gpui::Hsla,
    /// Selected-file entry text.
    pub file_text: gpui::Hsla,
    /// Muted text color (used for error and secondary labels).
    pub text_muted: gpui::Hsla,
}

impl FileUploadColors {
    /// Resolves [`FileUpload`] colors from the given token set.
    #[must_use]
    pub fn resolve(theme: &Theme) -> Self {
        Self {
            zone_bg: theme.secondary,
            zone_border: theme.border,
            zone_text: theme.muted_foreground,
            zone_bg_active: theme.secondary,
            zone_border_active: FocusRingMetrics::dark().color,
            button_bg: theme.accent,
            button_text: theme.accent_foreground,
            file_bg: theme.secondary,
            file_text: theme.foreground,
            text_muted: theme.muted_foreground,
        }
    }
}

// ── FileUploadHandler ─────────────────────────────────────────────────────────

/// Callback type for [`FileUpload`] file selections.
///
/// Invoked with the accepted [`PathBuf`] list and mutable context references
/// whenever the committed file selection changes via a drop event.
pub type FileUploadHandler = Rc<dyn Fn(Vec<PathBuf>, &mut Window, &mut App)>;

// ── FileUpload ────────────────────────────────────────────────────────────────

/// Drop zone + **Browse…** file upload component.
///
/// Create via `cx.new(|cx| FileUpload::new(cx))` and configure with builder
/// setters before storing as an `Entity<FileUpload>`.
///
/// # Example
///
/// ```rust,ignore
/// let upload = cx.new(|cx| {
///     FileUpload::new(cx)
///         .with_accept(vec!["png".to_owned(), "jpg".to_owned()])
///         .multiple(true)
///         .on_files_selected(|paths, _window, _cx| {
///             println!("{} file(s) selected", paths.len());
///         })
///         .dark(true)
/// });
/// ```
pub struct FileUpload {
    /// Optional list of accepted file extensions (lower-cased, no leading dot).
    ///
    /// `None` means all files are accepted.  `Some` means only files whose
    /// extension (case-insensitive) appears in the list are accepted.
    accept: Option<Vec<String>>,
    /// Whether the user may select more than one file at a time.
    multiple: bool,
    /// Optional callback fired when files are committed from a drop event.
    on_files_selected: Option<FileUploadHandler>,
    /// Files accepted in the most recent selection or drop.
    selected_files: Vec<PathBuf>,
    /// Whether files are currently being dragged over the drop zone.
    ///
    /// Set to `true` by the mouse-down event that GPUI synthesises from
    /// [`gpui::FileDropEvent::Entered`] and cleared when the drop completes
    /// or the drag leaves.
    dragging_over: bool,
    /// Last error message from the native dialog, if any.
    browse_error: Option<String>,
    /// Whether to use dark-mode styling.
    dark: bool,
}

impl FileUpload {
    /// Creates a new `FileUpload` with default settings.
    ///
    /// Defaults: dark styling, single-file mode, no accept filter, no callback.
    #[must_use]
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            accept: None,
            multiple: false,
            on_files_selected: None,
            selected_files: Vec::new(),
            dragging_over: false,
            browse_error: None,
            dark: true,
        }
    }

    // ── builder setters ───────────────────────────────────────────────────────

    /// Sets the accepted file extensions.
    ///
    /// Each entry should be a lowercase extension string without a leading dot
    /// (e.g. `"rs"`, `"toml"`, `"png"`).  An empty list is treated as "accept
    /// all" (same as calling with `None`).
    #[must_use]
    pub fn with_accept(mut self, extensions: Vec<String>) -> Self {
        let lower: Vec<String> = extensions.into_iter().map(|e| e.to_lowercase()).collect();
        self.accept = if lower.is_empty() { None } else { Some(lower) };
        self
    }

    /// Enables or disables multi-file selection.
    ///
    /// When `false` (the default), only the first accepted path is committed
    /// even if the user drops multiple files.
    #[must_use]
    pub fn multiple(mut self, multiple: bool) -> Self {
        self.multiple = multiple;
        self
    }

    /// Registers a callback that fires when files are committed via drop.
    ///
    /// The callback receives the accepted [`PathBuf`] list.  It is NOT invoked
    /// when files are committed through the browse dialog (async context has
    /// no `&mut Window`); use [`FileUpload::selected_files`] to observe the
    /// result in that case.
    #[must_use]
    pub fn on_files_selected(
        mut self,
        handler: impl Fn(Vec<PathBuf>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_files_selected = Some(Rc::new(handler));
        self
    }

    /// Enables dark-mode (`true`, the default) or light-mode (`false`) styling.
    #[must_use]
    pub fn dark(mut self, dark: bool) -> Self {
        self.dark = dark;
        self
    }

    // ── live update ───────────────────────────────────────────────────────────

    /// Updates the dark-mode flag and schedules a re-render.
    ///
    /// Called by the gallery when the active color mode changes.
    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        self.dark = dark;
        cx.notify();
    }

    // ── state accessors ───────────────────────────────────────────────────────

    /// Returns the files accepted in the most recent selection.
    #[must_use]
    pub fn selected_files(&self) -> &[PathBuf] {
        &self.selected_files
    }

    /// Returns `true` if an `on_files_selected` handler has been registered.
    #[must_use]
    pub fn has_on_files_selected(&self) -> bool {
        self.on_files_selected.is_some()
    }

    /// Returns `true` if files are currently being dragged over the drop zone.
    #[must_use]
    pub fn is_dragging_over(&self) -> bool {
        self.dragging_over
    }

    /// Returns the last browse-dialog error message, if any.
    #[must_use]
    pub fn browse_error(&self) -> Option<&str> {
        self.browse_error.as_deref()
    }

    // ── internal helpers ──────────────────────────────────────────────────────

    /// Applies the `accept` extension filter to `paths`.
    ///
    /// Returns all paths unchanged when `accept` is `None`.  When `accept` is
    /// `Some`, only paths whose file extension (lower-cased, no leading dot)
    /// matches an entry are kept.
    fn filter_paths(&self, paths: &[PathBuf]) -> Vec<PathBuf> {
        let Some(ref exts) = self.accept else {
            return paths.to_vec();
        };
        paths
            .iter()
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| exts.iter().any(|a| a == &e.to_lowercase()))
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    /// Clones and invokes the `on_files_selected` handler.
    ///
    /// Called only from GPUI event listeners that carry `&mut Window`.
    fn fire_on_files_selected(
        &self,
        files: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(cb) = self.on_files_selected.clone() {
            cb(files, window, cx);
        }
    }

    /// Handles a file drop: filters, truncates to single if needed, stores,
    /// and fires the callback.
    ///
    /// Called from the `on_drop` listener (has `&mut Window`).
    fn handle_drop(&mut self, ev: &ExternalPaths, window: &mut Window, cx: &mut Context<Self>) {
        let raw = ev.paths().to_vec();
        let mut accepted = self.filter_paths(&raw);
        if !self.multiple && accepted.len() > 1 {
            accepted.truncate(1);
        }
        self.selected_files = accepted.clone();
        self.dragging_over = false;
        self.browse_error = None;
        cx.notify();
        if !accepted.is_empty() {
            self.fire_on_files_selected(accepted, window, cx);
        }
    }

    // ── test helpers (headless, no &mut Window) ───────────────────────────────

    /// Injects `paths` as if they had been dropped, applying the accept filter
    /// and the `multiple` constraint.  Does **not** fire `on_files_selected`.
    ///
    /// Intended for headless unit tests that cannot provide `&mut Window`.
    pub fn apply_files_silent(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let mut accepted = self.filter_paths(&paths);
        if !self.multiple && accepted.len() > 1 {
            accepted.truncate(1);
        }
        self.selected_files = accepted;
        self.browse_error = None;
        cx.notify();
    }

    /// Applies the result of a native browse dialog.
    ///
    /// `paths` is `None` when the user cancelled; `Some(vec)` when files were
    /// chosen.  Uses [`apply_files_silent`](Self::apply_files_silent) internally,
    /// so the `on_files_selected` callback is **not** fired.
    ///
    /// Public to support headless unit tests.
    pub fn apply_browse_result(&mut self, paths: Option<Vec<PathBuf>>, cx: &mut Context<Self>) {
        if let Some(paths) = paths {
            self.apply_files_silent(paths, cx);
        }
    }

    /// Records an error from the native dialog and schedules a re-render.
    ///
    /// Public to support headless unit tests.
    pub fn apply_browse_error(&mut self, msg: String, cx: &mut Context<Self>) {
        self.browse_error = Some(msg);
        cx.notify();
    }

    // ── async browse ──────────────────────────────────────────────────────────

    /// Spawns an async task that opens the platform native file-picker dialog.
    ///
    /// Results are posted back through [`apply_browse_result`] or
    /// [`apply_browse_error`].  The task detaches immediately so the view is
    /// not held open.
    fn spawn_browse(&self, cx: &mut Context<Self>) {
        let multiple = self.multiple;
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            // gpui main: `AsyncApp::update` returns the closure value directly
            // (no longer `Result`), so this yields the `Receiver` itself.
            let rx = cx.update(|app| {
                app.prompt_for_paths(PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple,
                    prompt: Some("Choose file(s)".into()),
                })
            });
            match rx.await {
                Ok(Ok(pick)) => {
                    let _ = this.update(cx, |view, cx| {
                        view.apply_browse_result(pick, cx);
                    });
                }
                Ok(Err(e)) => {
                    let msg = format!("File picker error: {e}");
                    let _ = this.update(cx, |view, cx| {
                        view.apply_browse_error(msg, cx);
                    });
                }
                Err(_) => {
                    let _ = this.update(cx, |view, cx| {
                        view.apply_browse_error(
                            "File picker failed to return a result.".to_owned(),
                            cx,
                        );
                    });
                }
            }
        })
        .detach();
    }
}

// ── Render ────────────────────────────────────────────────────────────────────

impl Render for FileUpload {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = if self.dark {
            Theme::from(&*ThemeColor::dark())
        } else {
            Theme::from(&*ThemeColor::light())
        };
        let c = FileUploadColors::resolve(&theme);
        let dragging = self.dragging_over;

        let zone_bg = if dragging {
            c.zone_bg_active
        } else {
            c.zone_bg
        };
        let zone_border = if dragging {
            c.zone_border_active
        } else {
            c.zone_border
        };
        let zone_label = if dragging {
            "Drop files here"
        } else {
            "Drag files here"
        };

        // ── drop zone ─────────────────────────────────────────────────────────
        let drop_zone = div()
            .id("file-upload-drop-zone")
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .w(px(240.0))
            .h(px(100.0))
            .rounded(px(RadiusScale::default().md))
            .bg(zone_bg)
            .border_1()
            .border_color(zone_border)
            .child(
                div()
                    .text_size(px(TypographyScale::default().sm))
                    .text_color(c.zone_text)
                    .child(SharedString::from(zone_label)),
            )
            .on_drop::<ExternalPaths>(cx.listener(|this, ev: &ExternalPaths, window, cx| {
                this.handle_drop(ev, window, cx);
            }));

        // ── browse button ─────────────────────────────────────────────────────
        let browse_btn = div()
            .id("file-upload-browse-btn")
            .px_3()
            .py_1()
            .rounded(px(RadiusScale::default().sm))
            .bg(c.button_bg)
            .text_size(px(TypographyScale::default().sm))
            .text_color(c.button_text)
            .cursor_pointer()
            .child(SharedString::from("Browse\u{2026}"))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _ev, _window, cx| {
                    this.spawn_browse(cx);
                }),
            );

        // ── selected files list ───────────────────────────────────────────────
        let mut file_list = div().id("file-upload-file-list").flex().flex_col().gap_1();
        for (i, path) in self.selected_files.iter().enumerate() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_owned)
                .unwrap_or_else(|| path.to_string_lossy().into_owned());
            file_list = file_list.child(
                div()
                    .id(SharedString::from(format!("file-entry-{i}")))
                    .px_2()
                    .py_1()
                    .rounded(px(RadiusScale::default().sm))
                    .bg(c.file_bg)
                    .text_size(px(TypographyScale::default().sm))
                    .text_color(c.file_text)
                    .child(SharedString::from(name)),
            );
        }

        // ── error row ─────────────────────────────────────────────────────────
        let mut root = div()
            .id("file-upload-root")
            .flex()
            .flex_col()
            .gap_2()
            .child(drop_zone)
            .child(browse_btn)
            .child(file_list);

        if let Some(msg) = self.browse_error.as_deref() {
            root = root.child(
                div()
                    .text_size(px(TypographyScale::default().sm))
                    .text_color(theme.danger)
                    .child(SharedString::from(msg.to_owned())),
            );
        }

        root
    }
}
