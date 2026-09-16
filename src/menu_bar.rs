use gpui::{
    App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render,
    Window, anchored, deferred, div, prelude::*, px, rgb,
};

const APP_MENU_BAR_CONTEXT: &str = "AppMenuBar";

#[derive(Clone, Copy)]
pub enum AppMenuItem {
    Entry {
        label: &'static str,
        action: fn(&mut Window, &mut App),
    },
    Separator,
}

struct BarEntry {
    name: &'static str,
    items: Vec<AppMenuItem>,
    open_menu: Option<Entity<DropdownMenu>>,
}

struct DropdownMenu {
    items: Vec<AppMenuItem>,
    focus_handle: FocusHandle,
}

impl DropdownMenu {
    fn new(items: Vec<AppMenuItem>, cx: &mut Context<Self>) -> Self {
        Self {
            items,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for DropdownMenu {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<DismissEvent> for DropdownMenu {}

impl Render for DropdownMenu {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        deferred(
            anchored()
                .snap_to_window_with_margin(px(8.0))
                .child(
                    div().occlude().child(
                        div()
                            .key_context(APP_MENU_BAR_CONTEXT)
                            .track_focus(&self.focus_handle)
                            .min_w(px(160.0))
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(0x3a3d45))
                            .bg(rgb(0x23262d))
                            .shadow_lg()
                            .py_1()
                            .children(self.items.iter().copied().enumerate().map(
                                |(index, item)| match item {
                                    AppMenuItem::Separator => div()
                                        .h(px(1.0))
                                        .mx_2()
                                        .my_1()
                                        .bg(rgb(0x3a3d45))
                                        .into_any_element(),
                                    AppMenuItem::Entry { label, action } => div()
                                        .id(gpui::ElementId::Integer(index as u64))
                                        .px_3()
                                        .py_1()
                                        .rounded_sm()
                                        .text_size(px(13.0))
                                        .text_color(rgb(0xd0d3dc))
                                        .cursor_pointer()
                                        .hover(|style| style.bg(rgb(0x2a4a8f)))
                                        .on_click(cx.listener(move |_, _, window, cx| {
                                            action(window, &mut *cx);
                                            cx.emit(DismissEvent);
                                        }))
                                        .child(label)
                                        .into_any_element(),
                                },
                            )),
                    ),
                ),
        )
        .with_priority(1)
    }
}

pub struct AppMenuBar {
    entries: Vec<BarEntry>,
    focus_handle: FocusHandle,
}

impl AppMenuBar {
    #[must_use]
    pub fn new(menus: Vec<(&'static str, Vec<AppMenuItem>)>, cx: &mut Context<Self>) -> Self {
        Self {
            entries: menus
                .into_iter()
                .map(|(name, items)| BarEntry {
                    name,
                    items,
                    open_menu: None,
                })
                .collect(),
            focus_handle: cx.focus_handle(),
        }
    }

    fn any_open(&self) -> bool {
        self.entries.iter().any(|entry| entry.open_menu.is_some())
    }

    fn open_index(&self) -> Option<usize> {
        self.entries.iter().position(|entry| entry.open_menu.is_some())
    }

    pub fn open_menu_at(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.entries.len() {
            return;
        }

        for entry in &mut self.entries {
            entry.open_menu = None;
        }

        let items = self.entries[index].items.clone();
        let menu = cx.new(move |cx| DropdownMenu::new(items, cx));
        cx.subscribe(&menu, move |this, _, _: &DismissEvent, cx| {
            if let Some(entry) = this.entries.get_mut(index) {
                entry.open_menu = None;
            }
            cx.notify();
        })
        .detach();

        let handle = menu.read(cx).focus_handle(cx);
        window.on_next_frame(move |window, _cx| {
            window.on_next_frame(move |window, _cx| {
                window.focus(&handle, _cx);
            });
        });

        self.entries[index].open_menu = Some(menu);
        cx.notify();
    }

    pub fn close_all(&mut self, cx: &mut Context<Self>) {
        let was_open = self.any_open();
        for entry in &mut self.entries {
            entry.open_menu = None;
        }
        if was_open {
            cx.notify();
        }
    }

    pub fn open_first(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.entries.is_empty() {
            self.open_menu_at(0, window, cx);
        }
    }

    pub fn navigate_right(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(current) = self.open_index() else {
            return;
        };
        let next = (current + 1) % self.entries.len();
        self.open_menu_at(next, window, cx);
    }

    pub fn navigate_left(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(current) = self.open_index() else {
            return;
        };
        let previous = if current == 0 {
            self.entries.len() - 1
        } else {
            current - 1
        };
        self.open_menu_at(previous, window, cx);
    }
}

impl Focusable for AppMenuBar {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AppMenuBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        struct Snapshot {
            name: &'static str,
            is_open: bool,
            menu: Option<Entity<DropdownMenu>>,
        }

        let snapshots: Vec<Snapshot> = self
            .entries
            .iter()
            .map(|entry| Snapshot {
                name: entry.name,
                is_open: entry.open_menu.is_some(),
                menu: entry.open_menu.clone(),
            })
            .collect();
        let any_open = snapshots.iter().any(|entry| entry.is_open);

        div()
            .key_context(APP_MENU_BAR_CONTEXT)
            .track_focus(&self.focus_handle)
            .flex()
            .flex_row()
            .items_center()
            .h(px(28.0))
            .w_full()
            .bg(rgb(0x1c1f24))
            .border_b_1()
            .border_color(rgb(0x2a2d35))
            .px_2()
            .gap_x_0p5()
            .children(snapshots.into_iter().enumerate().map(|(index, entry)| {
                let button = div()
                    .id(gpui::ElementId::Integer(index as u64))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .text_size(px(13.0))
                    .text_color(rgb(0xd0d3dc))
                    .cursor_pointer()
                    .when(entry.is_open, |style| style.bg(rgb(0x2a2d35)))
                    .when(!entry.is_open, |style| style.hover(|hover| hover.bg(rgb(0x2a2d35))))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if this
                            .entries
                            .get(index)
                            .is_some_and(|menu| menu.open_menu.is_some())
                        {
                            this.close_all(cx);
                        } else {
                            this.open_menu_at(index, window, cx);
                        }
                    }))
                    .when(any_open && !entry.is_open, |style| {
                        style.on_mouse_move(cx.listener(move |this, _, window, cx| {
                            if this.any_open()
                                && this
                                    .entries
                                    .get(index)
                                    .is_some_and(|menu| menu.open_menu.is_none())
                            {
                                this.open_menu_at(index, window, cx);
                            }
                        }))
                    })
                    .child(entry.name);

                div()
                    .relative()
                    .child(button)
                    .when_some(entry.menu, |style, menu| style.child(menu))
                    .into_any_element()
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_menu_item_entry_builds() {
        let item = AppMenuItem::Entry {
            label: "Quit",
            action: |_window, _cx| {},
        };
        assert!(matches!(item, AppMenuItem::Entry { .. }));
    }
}
