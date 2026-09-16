//! Office-style ribbon component for generated GPUI starters.
//!
//! Provides a tabbed toolbar strip modelled after the Microsoft Office ribbon.
//! The ribbon remains handrolled (no gpui-component equivalent). Upstream
//! `TabBar` with `TabVariant::Segmented` replaces the standalone
//! `SegmentedControl` widget; `RibbonItem::SegmentedControl` still exists for
//! ribbon-internal use.
//!
//! - [`RibbonTab`] — a single tab entry (id, label, panel).
//! - [`RibbonPanel`] — the content area shown when a tab is active.
//! - [`RibbonGroup`] — a named group of items within a panel.
//! - [`RibbonItem`] — an individual widget: button, icon button,
//!   segmented control (ribbon-internal), embedded select, or separator.
//! - [`Ribbon`] — the top-level stateful GPUI [`Render`] view.
//!
//! ## Visual structure
//!
//! ```text
//! ┌──────────────────────────────────────────────────────┐
//! │  Home │ Insert │ Format                              │  ← tab strip
//! ├──────────────────────────────────────────────────────┤
//! │ ┌──────────┐ │ ┌─────────────────┐ │ ┌────────────┐ │  ← panel
//! │ │ 📋 Paste │ │ │ B  I  U  (seg.) │ │ │ Styles ▼   │ │
//! │ │──────────│ │ │                 │ │ │            │ │
//! │ │Clipboard │ │ │      Font       │ │ │   Styles   │ │  ← group labels
//! │ └──────────┘ │ └─────────────────┘ │ └────────────┘ │
//! └──────────────────────────────────────────────────────┘
//! ```
//!
//! ## Responsive collapse
//!
//! Call [`Ribbon::set_width`] with the current container width; when the
//! width falls below [`Ribbon::collapse_threshold`] the ribbon switches to
//! *collapsed* mode where icon-button groups show only icons (no labels) and
//! group labels are hidden.  Call [`Ribbon::set_collapsed`] to override the
//! state directly.
//!
//! ## Example
//!
//! ```rust,ignore
//! let ribbon = cx.new(|cx| {
//!     Ribbon::new(cx)
//!         .with_tabs(vec![
//!             RibbonTab::new("home", "Home").with_panel(
//!                 RibbonPanel::new(vec![
//!                     RibbonGroup::new("Clipboard", vec![
//!                         RibbonItem::icon_button("📋", "Paste"),
//!                     ]),
//!                 ])
//!             ),
//!         ])
//!         .dark(dark)
//! });
//! ```
//!
//! # GPUI version
//!
//! Implemented against `gpui 0.2.2`.

use std::rc::Rc;

use gpui::{
    App, ClickEvent, Context, ElementId, IntoElement, Render, SharedString, Window, div,
    prelude::*, px,
};

use crate::select::Select;
use crate::ui_tokens::{RadiusScale, SpacingScale, TypographyScale};
use gpui_kit::component::{Theme, ThemeColor};

// ── constants ─────────────────────────────────────────────────────────────────

/// Stable feature id for the Ribbon component.
pub const RIBBON_FEATURE_ID: &str = "ribbon";

/// Default collapse threshold in logical pixels.
///
/// When the container width reported via [`Ribbon::set_width`] falls below
/// this value the ribbon switches to icon-only collapsed mode.
pub const DEFAULT_COLLAPSE_THRESHOLD: f32 = 520.0;

// ── handler types ─────────────────────────────────────────────────────────────

/// Click handler stored inside [`RibbonItem::Button`] and
/// [`RibbonItem::IconButton`] items.
type RibbonClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// Selection callback for [`SegmentedControl`] and
/// [`RibbonItem::SegmentedControl`].
///
/// Receives the zero-based index of the newly selected segment.
pub type SegmentedSelectHandler = Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;

// ── SegmentOption ─────────────────────────────────────────────────────────────

/// A single option within a [`SegmentedControl`].
#[derive(Clone, Debug)]
pub struct SegmentOption {
    /// Short visible label.
    pub label: SharedString,
    /// Optional leading icon glyph (emoji or short text).
    pub icon: Option<SharedString>,
}

impl SegmentOption {
    /// Creates an option with `label` and no icon.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            icon: None,
        }
    }

    /// Attaches an optional icon glyph shown before the label.
    #[must_use]
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

// ── RibbonItem ────────────────────────────────────────────────────────────────

/// A single interactive item within a [`RibbonGroup`].
#[allow(clippy::large_enum_variant)]
pub enum RibbonItem {
    /// A text button with an optional leading icon glyph.
    Button {
        /// Visible button label.
        label: SharedString,
        /// Optional leading icon glyph (emoji or short text).
        icon: Option<SharedString>,
        /// When `true` the button is non-interactive.
        disabled: bool,
        /// Optional click handler.
        on_click: Option<RibbonClickHandler>,
    },
    /// An icon-primary button.  In normal mode the icon is shown above a small
    /// label; in collapsed mode only the icon is shown.
    IconButton {
        /// Icon glyph shown prominently.
        icon: SharedString,
        /// Accessibility / label text shown below the icon in normal mode.
        label: SharedString,
        /// When `true` the button is non-interactive.
        disabled: bool,
        /// Optional click handler.
        on_click: Option<RibbonClickHandler>,
    },
    /// An inline segmented control.  Selection state is managed inside the
    /// parent [`Ribbon`] entity.
    SegmentedControl {
        /// Ordered list of segment options.
        options: Vec<SegmentOption>,
        /// Zero-based index of the currently active segment.
        selected: usize,
        /// Optional callback fired when the user selects a segment.
        on_select: Option<SegmentedSelectHandler>,
    },
    /// An embedded [`Select`] entity (e.g. a font-size dropdown).
    ///
    /// The caller must create the entity and manage its dark/light theme
    /// synchronisation.
    Select(gpui::Entity<Select<String>>),
    /// A thin vertical separator line between logical sub-groups.
    Separator,
}

impl RibbonItem {
    /// Convenience constructor for a [`RibbonItem::Button`].
    pub fn button(label: impl Into<SharedString>) -> Self {
        Self::Button {
            label: label.into(),
            icon: None,
            disabled: false,
            on_click: None,
        }
    }

    /// Convenience constructor for a [`RibbonItem::Button`] with an icon.
    pub fn button_with_icon(icon: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self::Button {
            label: label.into(),
            icon: Some(icon.into()),
            disabled: false,
            on_click: None,
        }
    }

    /// Convenience constructor for a [`RibbonItem::IconButton`].
    pub fn icon_button(icon: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self::IconButton {
            icon: icon.into(),
            label: label.into(),
            disabled: false,
            on_click: None,
        }
    }

    /// Convenience constructor for a [`RibbonItem::SegmentedControl`].
    pub fn segmented(options: Vec<SegmentOption>) -> Self {
        Self::SegmentedControl {
            options,
            selected: 0,
            on_select: None,
        }
    }
}

// ── RibbonGroup ───────────────────────────────────────────────────────────────

/// A named group of related items within a [`RibbonPanel`].
///
/// Rendered as a column of items with a small label at the bottom and a
/// vertical separator on the right side.
pub struct RibbonGroup {
    /// Short descriptive label shown below the items row.
    pub label: SharedString,
    /// Ordered items rendered left-to-right inside this group.
    pub items: Vec<RibbonItem>,
}

impl RibbonGroup {
    /// Creates a group with `label` and the given items.
    pub fn new(label: impl Into<SharedString>, items: Vec<RibbonItem>) -> Self {
        Self {
            label: label.into(),
            items,
        }
    }
}

// ── RibbonPanel ───────────────────────────────────────────────────────────────

/// The content area shown when its owning [`RibbonTab`] is active.
///
/// A panel consists of one or more [`RibbonGroup`]s rendered side-by-side.
pub struct RibbonPanel {
    /// Groups shown left-to-right in this panel.
    pub groups: Vec<RibbonGroup>,
}

impl RibbonPanel {
    /// Creates a panel containing the given groups.
    pub fn new(groups: Vec<RibbonGroup>) -> Self {
        Self { groups }
    }
}

// ── RibbonTab ─────────────────────────────────────────────────────────────────

/// A single tab entry in a [`Ribbon`].
///
/// Each tab owns exactly one [`RibbonPanel`] shown when the tab is active.
pub struct RibbonTab {
    /// Stable string identifier for this tab.
    pub id: SharedString,
    /// Visible label shown in the tab strip.
    pub label: SharedString,
    /// Content panel shown when this tab is active.
    pub panel: RibbonPanel,
}

impl RibbonTab {
    /// Creates a tab with `id` and `label` and an empty panel.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            panel: RibbonPanel::new(Vec::new()),
        }
    }

    /// Replaces the tab's panel.
    #[must_use]
    pub fn with_panel(mut self, panel: RibbonPanel) -> Self {
        self.panel = panel;
        self
    }
}

// ── RibbonColors ─────────────────────────────────────────────────────────────

/// Token-resolved display colors for a [`Ribbon`].
///
/// Exposed so unit tests can verify token bindings without constructing a full
/// render context.
#[derive(Clone, Copy, Debug)]
pub struct RibbonColors {
    /// Background of the tab strip row.
    pub tab_strip_bg: gpui::Hsla,
    /// Bottom border of the tab strip.
    pub tab_strip_border: gpui::Hsla,
    /// Background of the active tab button.
    pub tab_active_bg: gpui::Hsla,
    /// Underline accent of the active tab button.
    pub tab_active_underline: gpui::Hsla,
    /// Text color of the active tab button.
    pub tab_active_text: gpui::Hsla,
    /// Text color of inactive tab buttons.
    pub tab_text: gpui::Hsla,
    /// Hover background of inactive tab buttons.
    pub tab_hover_bg: gpui::Hsla,
    /// Background of the panel (content) area.
    pub panel_bg: gpui::Hsla,
    /// Bottom border of the panel area.
    pub panel_border: gpui::Hsla,
    /// Right-separator color between groups.
    pub group_separator: gpui::Hsla,
    /// Small group-label text color.
    pub group_label_text: gpui::Hsla,
    /// Background of a ribbon item (button/icon-button) in idle state.
    pub item_bg: gpui::Hsla,
    /// Hover background of a ribbon item.
    pub item_hover_bg: gpui::Hsla,
    /// Normal item text / icon color.
    pub item_text: gpui::Hsla,
    /// Disabled item text / icon color.
    pub item_disabled_text: gpui::Hsla,
    /// Background of the selected (active) segment.
    pub segment_active_bg: gpui::Hsla,
    /// Text of the selected segment.
    pub segment_active_text: gpui::Hsla,
    /// Background of unselected segments.
    pub segment_inactive_bg: gpui::Hsla,
    /// Text of unselected segments.
    pub segment_inactive_text: gpui::Hsla,
    /// Border / divider color for the segmented control outline.
    pub segment_border: gpui::Hsla,
}

impl RibbonColors {
    /// Resolves ribbon colors from the given `tokens`.
    #[must_use]
    pub fn resolve(theme: &Theme) -> Self {
        Self {
            tab_strip_bg: theme.secondary,
            tab_strip_border: theme.border,
            tab_active_bg: theme.secondary,
            tab_active_underline: theme.accent,
            tab_active_text: theme.accent,
            tab_text: theme.muted_foreground,
            tab_hover_bg: theme.secondary,
            panel_bg: theme.secondary,
            panel_border: theme.border,
            group_separator: theme.border,
            group_label_text: theme.muted_foreground,
            item_bg: gpui::hsla(0.0, 0.0, 0.0, 0.0),
            item_hover_bg: theme.secondary,
            item_text: theme.foreground,
            item_disabled_text: theme.muted_foreground,
            segment_active_bg: theme.accent,
            segment_active_text: theme.accent_foreground,
            segment_inactive_bg: theme.secondary,
            segment_inactive_text: theme.foreground,
            segment_border: theme.border,
        }
    }
}

// ── Ribbon ────────────────────────────────────────────────────────────────────

/// Office-style tabbed ribbon toolbar.
///
/// A stateful GPUI [`Render`] view that owns its tab list and active-tab index.
/// Tab switching, segmented-control selection, and collapse state are all
/// managed inside the entity.
///
/// # Usage
///
/// ```rust,ignore
/// let ribbon = cx.new(|cx| {
///     Ribbon::new(cx)
///         .with_tabs(vec![home_tab, insert_tab])
///         .with_collapse_threshold(480.0)
///         .dark(dark)
/// });
/// ```
pub struct Ribbon {
    /// Ordered list of tabs.
    tabs: Vec<RibbonTab>,
    /// Zero-based index of the currently active tab.
    active_tab: usize,
    /// When `true` groups show only icons; labels are hidden.
    collapsed: bool,
    /// Width (px) below which [`set_width`] switches to collapsed mode.
    ///
    /// [`set_width`]: Ribbon::set_width
    collapse_threshold: f32,
    /// Dark-mode palette toggle.
    dark: bool,
}

impl Ribbon {
    // ── construction ──────────────────────────────────────────────────────────

    /// Creates an empty ribbon with sensible defaults (light, not collapsed,
    /// threshold = [`DEFAULT_COLLAPSE_THRESHOLD`]).
    #[must_use]
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            tabs: Vec::new(),
            active_tab: 0,
            collapsed: false,
            collapse_threshold: DEFAULT_COLLAPSE_THRESHOLD,
            dark: false,
        }
    }

    /// Replaces the full tab list.
    ///
    /// The active-tab index is clamped so it always points at a valid tab.
    #[must_use]
    pub fn with_tabs(mut self, tabs: Vec<RibbonTab>) -> Self {
        let len = tabs.len();
        self.tabs = tabs;
        if len > 0 {
            self.active_tab = self.active_tab.min(len - 1);
        } else {
            self.active_tab = 0;
        }
        self
    }

    /// Sets the collapse-threshold in logical pixels.
    #[must_use]
    pub fn with_collapse_threshold(mut self, threshold: f32) -> Self {
        self.collapse_threshold = threshold;
        self
    }

    /// Selects the dark (`true`) or light (`false`) token palette.
    #[must_use]
    pub fn dark(mut self, dark: bool) -> Self {
        self.dark = dark;
        self
    }

    // ── runtime setters ───────────────────────────────────────────────────────

    /// Switches the dark/light theme and schedules a re-render.
    ///
    /// Also propagates the new theme flag to any embedded
    /// [`RibbonItem::Select`] entities.
    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        self.dark = dark;
        // Propagate to nested Select entities.
        for tab in &self.tabs {
            for group in &tab.panel.groups {
                for item in &group.items {
                    if let RibbonItem::Select(entity) = item {
                        let e = entity.clone();
                        e.update(cx, |s, cx| {
                            s.set_dark(dark);
                            cx.notify();
                        });
                    }
                }
            }
        }
        cx.notify();
    }

    /// Switches to the tab at zero-based `index` and schedules a re-render.
    ///
    /// The index is clamped to the valid range.
    pub fn set_active_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.tabs.is_empty() {
            return;
        }
        self.active_tab = index.min(self.tabs.len() - 1);
        cx.notify();
    }

    /// Directly overrides the collapsed state and schedules a re-render.
    pub fn set_collapsed(&mut self, collapsed: bool, cx: &mut Context<Self>) {
        if self.collapsed != collapsed {
            self.collapsed = collapsed;
            cx.notify();
        }
    }

    /// Updates the ribbon width.
    ///
    /// Automatically switches to collapsed mode when `width <
    /// collapse_threshold` and back to normal mode when `width >=
    /// collapse_threshold`.  Schedules a re-render only when the collapsed
    /// state actually changes.
    pub fn set_width(&mut self, width: f32, cx: &mut Context<Self>) {
        let new_collapsed = width < self.collapse_threshold;
        if new_collapsed != self.collapsed {
            self.collapsed = new_collapsed;
            cx.notify();
        }
    }

    /// Updates the segmented-control selection at (tab, group, item, segment)
    /// and schedules a re-render.
    ///
    /// This is called internally from the on-click closure generated during
    /// [`Render::render`].
    pub fn set_segment_selected(
        &mut self,
        tab_i: usize,
        group_i: usize,
        item_i: usize,
        seg_i: usize,
        cx: &mut Context<Self>,
    ) {
        let updated = self
            .tabs
            .get_mut(tab_i)
            .and_then(|t| t.panel.groups.get_mut(group_i))
            .and_then(|g| g.items.get_mut(item_i))
            .map(|it| {
                if let RibbonItem::SegmentedControl {
                    selected, options, ..
                } = it
                {
                    let max = options.len().saturating_sub(1);
                    let clamped = seg_i.min(max);
                    if *selected != clamped {
                        *selected = clamped;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            })
            .unwrap_or(false);

        if updated {
            cx.notify();
        }
    }

    // ── accessors ─────────────────────────────────────────────────────────────

    /// Returns the zero-based index of the currently active tab.
    #[must_use]
    pub fn active_tab(&self) -> usize {
        self.active_tab
    }

    /// Returns `true` when the ribbon is in collapsed (icon-only) mode.
    #[must_use]
    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    /// Returns the collapse-threshold in logical pixels.
    #[must_use]
    pub fn collapse_threshold(&self) -> f32 {
        self.collapse_threshold
    }

    /// Returns `true` if the dark palette is active.
    #[must_use]
    pub fn is_dark(&self) -> bool {
        self.dark
    }
}

// ── Render ────────────────────────────────────────────────────────────────────

impl Render for Ribbon {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = if self.dark {
            Theme::from(&*ThemeColor::dark())
        } else {
            Theme::from(&*ThemeColor::light())
        };
        let colors = RibbonColors::resolve(&theme);
        let entity = cx.entity();
        let active_tab = self.active_tab;
        let collapsed = self.collapsed;
        let n_tabs = self.tabs.len();

        // ── Tab strip ─────────────────────────────────────────────────────────

        let mut tab_strip = div()
            .id("ribbon-tabs")
            .flex()
            .flex_row()
            .items_center()
            .bg(colors.tab_strip_bg)
            .border_b_1()
            .border_color(colors.tab_strip_border);

        for i in 0..n_tabs {
            let label = self.tabs[i].label.clone();
            let is_active = i == active_tab;
            let entity_t = entity.clone();
            let bg = if is_active {
                colors.tab_active_bg
            } else {
                colors.tab_strip_bg
            };
            let text_col = if is_active {
                colors.tab_active_text
            } else {
                colors.tab_text
            };
            let hover_bg = colors.tab_hover_bg;

            let tab_btn = div()
                .id(ElementId::Name(format!("ribbon-tab-{i}").into()))
                .px(px(SpacingScale::default().md))
                .py(px(SpacingScale::default().xs))
                .text_size(px(TypographyScale::default().sm))
                .text_color(text_col)
                .bg(bg)
                .border_b_2()
                .border_color(if is_active {
                    colors.tab_active_underline
                } else {
                    colors.tab_strip_bg
                })
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .child(label)
                .on_click(move |_, _, app| {
                    entity_t.update(app, |r, cx| {
                        r.set_active_tab(i, cx);
                    });
                });

            tab_strip = tab_strip.child(tab_btn);
        }

        // ── Panel (active-tab groups) ─────────────────────────────────────────

        let mut panel = div()
            .id("ribbon-panel")
            .flex()
            .flex_row()
            .bg(colors.panel_bg)
            .border_b_1()
            .border_color(colors.panel_border)
            .py(px(SpacingScale::default().xs));

        if let Some(tab) = self.tabs.get(active_tab) {
            let n_groups = tab.panel.groups.len();

            for (g_i, group) in tab.panel.groups.iter().enumerate() {
                let group_label = group.label.clone();
                let is_last_group = g_i + 1 == n_groups;
                let n_items = group.items.len();

                // Items row
                let mut items_row = div()
                    .id(ElementId::Name(format!("ribbon-g{g_i}-items").into()))
                    .flex()
                    .flex_row()
                    .items_end()
                    .gap_1();

                for (item_i, item) in group.items.iter().enumerate() {
                    let item_id_base = format!("ribbon-t{active_tab}-g{g_i}-i{item_i}");

                    let item_el: gpui::AnyElement = match item {
                        // ── Button ────────────────────────────────────────────
                        RibbonItem::Button {
                            label,
                            icon,
                            disabled,
                            on_click,
                        } => {
                            let handler = on_click.clone();
                            let is_disabled = *disabled;
                            let btn_id = ElementId::Name(format!("{item_id_base}-btn").into());
                            let item_hover_bg = colors.item_hover_bg;

                            let mut btn = div()
                                .id(btn_id)
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_1()
                                .px(px(SpacingScale::default().sm))
                                .h(px(28.0))
                                .rounded(px(RadiusScale::default().sm))
                                .text_size(px(TypographyScale::default().sm))
                                .text_color(if is_disabled {
                                    colors.item_disabled_text
                                } else {
                                    colors.item_text
                                });

                            if !is_disabled {
                                btn = btn.cursor_pointer().hover(move |s| s.bg(item_hover_bg));
                            } else {
                                btn = btn.opacity(0.55);
                            }

                            // In collapsed mode show only icon (or first char)
                            if collapsed {
                                if let Some(ico) = icon {
                                    btn = btn.child(ico.clone());
                                } else {
                                    let abbr: SharedString = label
                                        .chars()
                                        .next()
                                        .map(|c| c.to_string().into())
                                        .unwrap_or_else(|| "•".into());
                                    btn = btn.child(abbr);
                                }
                            } else {
                                if let Some(ico) = icon {
                                    btn = btn.child(ico.clone());
                                }
                                btn = btn.child(label.clone());
                            }

                            if !is_disabled {
                                btn = btn.on_click(move |ev, w, app| {
                                    if let Some(h) = &handler {
                                        h(ev, w, app);
                                    }
                                });
                            }

                            btn.into_any_element()
                        }

                        // ── IconButton ────────────────────────────────────────
                        RibbonItem::IconButton {
                            icon,
                            label,
                            disabled,
                            on_click,
                        } => {
                            let handler = on_click.clone();
                            let is_disabled = *disabled;
                            let btn_id = ElementId::Name(format!("{item_id_base}-ibtn").into());
                            let item_hover_bg = colors.item_hover_bg;
                            let item_text = colors.item_text;
                            let item_disabled = colors.item_disabled_text;

                            let mut btn = div()
                                .id(btn_id)
                                .flex()
                                .rounded(px(RadiusScale::default().sm))
                                .text_color(if is_disabled {
                                    item_disabled
                                } else {
                                    item_text
                                });

                            if collapsed {
                                // Icon-only square button
                                btn = btn
                                    .items_center()
                                    .justify_center()
                                    .size(px(32.0))
                                    .text_size(px(18.0))
                                    .child(icon.clone());
                            } else {
                                // Stacked: icon above label
                                btn = btn
                                    .flex_col()
                                    .items_center()
                                    .gap_1()
                                    .px(px(SpacingScale::default().xs))
                                    .py(px(SpacingScale::default().xs))
                                    .child(div().text_size(px(18.0)).child(icon.clone()))
                                    .child(div().text_size(px(10.0)).child(label.clone()));
                            }

                            if !is_disabled {
                                btn = btn.cursor_pointer().hover(move |s| s.bg(item_hover_bg));
                                btn = btn.on_click(move |ev, w, app| {
                                    if let Some(h) = &handler {
                                        h(ev, w, app);
                                    }
                                });
                            } else {
                                btn = btn.opacity(0.55);
                            }

                            btn.into_any_element()
                        }

                        // ── SegmentedControl ──────────────────────────────────
                        RibbonItem::SegmentedControl {
                            options,
                            selected,
                            on_select,
                        } => {
                            let on_sel = on_select.clone();
                            let entity_s = entity.clone();
                            let sel = *selected;
                            let seg_id_base = format!("{item_id_base}-seg");
                            let n_opts = options.len();
                            let sel_clamped = sel.min(n_opts.saturating_sub(1));

                            let mut seg_row = div()
                                .id(ElementId::Name(seg_id_base.clone().into()))
                                .flex()
                                .flex_row()
                                .items_center()
                                .border_1()
                                .border_color(colors.segment_border)
                                .rounded(px(RadiusScale::default().sm))
                                .overflow_hidden();

                            for (seg_i, opt) in options.iter().enumerate() {
                                let is_sel = seg_i == sel_clamped;
                                let is_last_seg = seg_i + 1 == n_opts;
                                let on_sel_s = on_sel.clone();
                                let entity_seg = entity_s.clone();
                                let opt_icon = opt.icon.clone();
                                let opt_label = opt.label.clone();
                                let bg = if is_sel {
                                    colors.segment_active_bg
                                } else {
                                    colors.segment_inactive_bg
                                };
                                let hover_bg = if is_sel {
                                    colors.segment_active_bg
                                } else {
                                    colors.item_hover_bg
                                };
                                let text_col = if is_sel {
                                    colors.segment_active_text
                                } else {
                                    colors.segment_inactive_text
                                };
                                let seg_el_id =
                                    ElementId::Name(format!("{seg_id_base}-{seg_i}").into());

                                let mut seg_opt = div()
                                    .id(seg_el_id)
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1()
                                    .px(px(SpacingScale::default().sm))
                                    .py(px(4.0))
                                    .bg(bg)
                                    .text_color(text_col)
                                    .text_size(px(TypographyScale::default().sm))
                                    .cursor_pointer()
                                    .hover(move |s| s.bg(hover_bg));

                                if !is_last_seg {
                                    seg_opt =
                                        seg_opt.border_r_1().border_color(colors.segment_border);
                                }

                                if let Some(ref ico) = opt_icon {
                                    seg_opt = seg_opt.child(ico.clone());
                                }
                                // In collapsed mode show only icon; if no icon, still show label
                                let show_label = !collapsed || opt_icon.is_none();
                                if show_label {
                                    seg_opt = seg_opt.child(opt_label);
                                }

                                seg_opt = seg_opt.on_click(move |_, w, app| {
                                    entity_seg.update(app, |r, cx| {
                                        r.set_segment_selected(active_tab, g_i, item_i, seg_i, cx);
                                    });
                                    if let Some(ref h) = on_sel_s {
                                        h(seg_i, w, app);
                                    }
                                });

                                seg_row = seg_row.child(seg_opt);
                            }

                            seg_row.into_any_element()
                        }

                        // ── Select entity ─────────────────────────────────────
                        RibbonItem::Select(select_entity) => div()
                            .id(ElementId::Name(format!("{item_id_base}-sel").into()))
                            .flex()
                            .items_center()
                            .child(select_entity.clone())
                            .into_any_element(),

                        // ── Separator ─────────────────────────────────────────
                        RibbonItem::Separator => {
                            let is_last_item = item_i + 1 == n_items;
                            div()
                                .id(ElementId::Name(format!("{item_id_base}-sep").into()))
                                .w(px(1.0))
                                .h(px(20.0))
                                .bg(colors.group_separator)
                                .mx(px(if is_last_item {
                                    0.0
                                } else {
                                    SpacingScale::default().xs
                                }))
                                .into_any_element()
                        }
                    };

                    items_row = items_row.child(item_el);
                }

                // Group column: items on top, label below (when not collapsed)
                let mut group_col = div()
                    .id(ElementId::Name(format!("ribbon-group-{g_i}").into()))
                    .flex()
                    .flex_col()
                    .items_center()
                    .px(px(SpacingScale::default().sm))
                    .child(items_row);

                if !collapsed {
                    group_col = group_col.child(
                        div()
                            .text_size(px(10.0))
                            .text_color(colors.group_label_text)
                            .mt(px(2.0))
                            .child(group_label),
                    );
                }

                // Right separator between groups (not after last)
                if !is_last_group {
                    group_col = group_col.border_r_1().border_color(colors.group_separator);
                }

                panel = panel.child(group_col);
            }
        }

        // ── Outer container ───────────────────────────────────────────────────

        div()
            .id("ribbon")
            .flex()
            .flex_col()
            .w_full()
            .border_1()
            .border_color(colors.panel_border)
            .rounded(px(RadiusScale::default().sm))
            .overflow_hidden()
            .child(tab_strip)
            .child(panel)
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // ── RibbonColors ──────────────────────────────────────────────────────────

    /// Verifies that `RibbonColors::resolve` maps the expected token fields.
    #[test]
    fn ribbon_colors_resolve_from_tokens() {
        let light = Theme::from(&*ThemeColor::light());
        let colors = RibbonColors::resolve(&light);
        assert_eq!(colors.tab_active_underline, light.accent);
        assert_eq!(colors.group_label_text, light.muted_foreground);
        assert_eq!(colors.segment_active_bg, light.accent);
        assert_eq!(colors.panel_bg, light.secondary);

        let dark = Theme::from(&*ThemeColor::dark());
        let dark_colors = RibbonColors::resolve(&dark);
        assert_eq!(dark_colors.tab_active_underline, dark.accent);
    }

    // ── Tab switching ─────────────────────────────────────────────────────────

    /// Proves that `set_active_tab` changes the stored index and clamps OOB
    /// indices to the last tab.
    #[gpui::test]
    fn tab_switch_changes_panel(cx: &mut gpui::TestAppContext) {
        let ribbon = cx.update(|cx| {
            cx.new(|cx| {
                Ribbon::new(cx).with_tabs(vec![
                    RibbonTab::new("home", "Home").with_panel(RibbonPanel::new(vec![
                        RibbonGroup::new("Clipboard", vec![RibbonItem::icon_button("📋", "Paste")]),
                    ])),
                    RibbonTab::new("insert", "Insert").with_panel(RibbonPanel::new(vec![
                        RibbonGroup::new("Tables", vec![RibbonItem::icon_button("📊", "Table")]),
                    ])),
                ])
            })
        });

        // Initially on tab 0.
        ribbon.read_with(cx, |r, _| {
            assert_eq!(r.active_tab(), 0, "must start on tab 0");
        });

        // Switch to tab 1.
        cx.update(|cx| {
            ribbon.update(cx, |r, cx| {
                r.set_active_tab(1, cx);
            });
        });

        ribbon.read_with(cx, |r, _| {
            assert_eq!(r.active_tab(), 1, "must switch to tab 1");
        });

        // Out-of-bounds index is clamped.
        cx.update(|cx| {
            ribbon.update(cx, |r, cx| {
                r.set_active_tab(99, cx);
            });
        });

        ribbon.read_with(cx, |r, _| {
            assert_eq!(r.active_tab(), 1, "OOB index must clamp to last tab");
        });
    }

    // ── Collapse at narrow width ──────────────────────────────────────────────

    /// Proves that `set_width` toggles collapsed mode when the width crosses
    /// the threshold, and `set_collapsed` overrides it directly.
    #[gpui::test]
    fn collapse_at_narrow_width(cx: &mut gpui::TestAppContext) {
        let ribbon = cx.update(|cx| cx.new(|cx| Ribbon::new(cx).with_collapse_threshold(400.0)));

        // Starts uncollapsed.
        ribbon.read_with(cx, |r, _| {
            assert!(!r.is_collapsed(), "must start uncollapsed");
        });

        // Wide — stays uncollapsed.
        cx.update(|cx| {
            ribbon.update(cx, |r, cx| {
                r.set_width(500.0, cx);
            });
        });
        ribbon.read_with(cx, |r, _| {
            assert!(
                !r.is_collapsed(),
                "width=500 must not collapse (threshold=400)"
            );
        });

        // Narrow — collapses.
        cx.update(|cx| {
            ribbon.update(cx, |r, cx| {
                r.set_width(300.0, cx);
            });
        });
        ribbon.read_with(cx, |r, _| {
            assert!(r.is_collapsed(), "width=300 must collapse (threshold=400)");
        });

        // Exactly at threshold — not collapsed (< not <=).
        cx.update(|cx| {
            ribbon.update(cx, |r, cx| {
                r.set_width(400.0, cx);
            });
        });
        ribbon.read_with(cx, |r, _| {
            assert!(!r.is_collapsed(), "width==threshold must not collapse");
        });

        // Direct override via set_collapsed.
        cx.update(|cx| {
            ribbon.update(cx, |r, cx| {
                r.set_collapsed(true, cx);
            });
        });
        ribbon.read_with(cx, |r, _| {
            assert!(r.is_collapsed(), "set_collapsed(true) must force collapse");
        });
    }

    // ── Segmented-control selection ───────────────────────────────────────────

    /// Proves that `set_segment_selected` updates the stored selection and
    /// clamps out-of-bounds indices.
    #[gpui::test]
    fn segmented_selection_state(cx: &mut gpui::TestAppContext) {
        let ribbon = cx.update(|cx| {
            cx.new(|cx| {
                Ribbon::new(cx).with_tabs(vec![RibbonTab::new("home", "Home").with_panel(
                    RibbonPanel::new(vec![RibbonGroup::new(
                        "Style",
                        vec![RibbonItem::SegmentedControl {
                            options: vec![
                                SegmentOption::new("B"),
                                SegmentOption::new("I"),
                                SegmentOption::new("U"),
                            ],
                            selected: 0,
                            on_select: None,
                        }],
                    )]),
                )])
            })
        });

        // Initial selection is 0.
        let sel = |r: &Ribbon| {
            if let Some(RibbonItem::SegmentedControl { selected, .. }) =
                r.tabs[0].panel.groups[0].items.first()
            {
                *selected
            } else {
                usize::MAX
            }
        };
        ribbon.read_with(cx, |r, _| {
            assert_eq!(sel(r), 0, "initial selection must be 0")
        });

        // Change to index 2.
        cx.update(|cx| {
            ribbon.update(cx, |r, cx| r.set_segment_selected(0, 0, 0, 2, cx));
        });
        ribbon.read_with(cx, |r, _| assert_eq!(sel(r), 2, "must update to 2"));

        // Out-of-bounds clamps to last.
        cx.update(|cx| {
            ribbon.update(cx, |r, cx| r.set_segment_selected(0, 0, 0, 99, cx));
        });
        ribbon.read_with(cx, |r, _| {
            assert_eq!(sel(r), 2, "OOB must clamp to last (2)")
        });
    }

    // ── Dark-mode propagation ─────────────────────────────────────────────────

    /// Proves that `set_dark` flips the dark flag on the ribbon entity.
    #[gpui::test]
    fn dark_mode_propagates(cx: &mut gpui::TestAppContext) {
        let ribbon = cx.update(|cx| cx.new(|cx| Ribbon::new(cx).dark(false)));
        ribbon.read_with(cx, |r, _| assert!(!r.is_dark(), "must start light"));

        cx.update(|cx| {
            ribbon.update(cx, |r, cx| r.set_dark(true, cx));
        });
        ribbon.read_with(cx, |r, _| assert!(r.is_dark(), "must switch to dark"));
    }
}
