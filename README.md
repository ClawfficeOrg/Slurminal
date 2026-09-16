# Slurminal

A GPUI terminal emulator built on `zoid-slurminal-component`, Zoid's libghostty-vt
bindings and GPUI terminal surface. Targets WezTerm-level feature parity with a
settings UI over a config file, Nerd Font support, background images, kitty and
sixel graphics, a Quake-style drop-down mode, a ratatui console companion, and
animation.

Scaffolded from Zoid's `menubar` GPUI template. It is a first-class Zoid example,
and it is the component's real consumer: gaps in the component's interface show up
here first and get fixed upstream.

## Status

Planning. See [`docs/todo-v1.json`](docs/todo-v1.json) for the implementation plan.

The terminal emulator itself is not implemented in this repository. It lives in
`zoid-slurminal-component` inside the Zoid workspace; this repo owns the
application around it.

## Planning

```sh
taskerkeeper list docs/todo-v1.json
taskerkeeper ready docs/todo-v1.json --json
```

## Template Purpose

The application menu provides primary navigation (File, Edit, View, Window, Help),
with settings and theme accessible through the menu and a Preferences window. Quit
dispatches the `Quit` action, which calls `App::quit()`.

## Commands

```sh
cargo run
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## Structure

| Module            | Purpose                                                         |
|-------------------|-----------------------------------------------------------------|
| `main.rs`         | App entry point: menu install, window creation, quit handler    |
| `menu_actions.rs` | GPUI action definitions and menu construction                   |
| `settings.rs`     | Typed app settings with safe load/save                         |
| `theme.rs`        | Light/dark theme tokens and runtime toggle                      |

## Platform Notes

> **macOS only.** `App::set_menus()` installs a native menu bar **only on macOS**.
> On Windows and Linux the GPUI backend stores the menu data in memory but never
> calls any OS menu API — the menu bar is completely invisible at runtime. This is
> a GPUI platform limitation, not a template bug.
>
> Zed's Windows app achieves a visible menu by rendering a custom GPUI-drawn
> titlebar component instead of using `set_menus()`. Adding that kind of
> in-window menu bar is a future Zoid feature (see `todo.md`:
> `fix-menubar-platform` follow-up). Until then, use this template on macOS only.

- **macOS**: The OS renders the global menu bar. `set_menus()` populates it via
  `NSMenu`/`NSMenuItem` through Objective-C.
- **Windows / Linux**: `set_menus()` compiles but does nothing at runtime.
  Generated keybindings still work; only the visible menu bar is absent.
- The `Services` system menu item is excluded on non-macOS platforms automatically.

## GPUI Baseline

- GPUI: tracks Zed `main`. This project declares gpui as a bare git dependency,
  so the first `cargo build` resolves whatever Zed `main` is that day and writes
  it to your `Cargo.lock`. Commit that lock — it is what makes your build
  reproducible. `cargo update` is then a deliberate act, not a side effect.
- Last verified by Zoid against: `gpui-kit 0.6.1 (gpui-pre 0.3.5)`
- Source checked by Zoid: docs.rs `gpui/latest`, gpui.rs examples, and Zed `main` `crates/gpui`.

GPUI is pre-1.0. Re-check current GPUI docs before changing app lifecycle, actions,
menus, or window management.
