# Review 1 (Claude) of the spike

The core problem: all Linux code is behind `#[cfg(target_os = "linux")]`, so `cargo check` and the tests on Windows compiled only `menew-core` and the stubs. The "What Was Verified" section of `spike-summary.md` is misleading. Fix everything below.

## Verification rule (required)

- Compile for real on Linux using WSL `Ubuntu-24.04` (it is on this machine, but has no Rust or GTK yet). Required packages: `build-essential pkg-config libgtk-4-dev libadwaita-1-dev libdbus-1-dev` + rustup. If `sudo` is needed, **stop and ask the user** to run the install. Don't try to work around it.
- Use a target dir inside WSL (e.g. `CARGO_TARGET_DIR=~/target-menew`) so it doesn't mix with the Windows build.
- Only call something "verified" if it was actually run. Report `cargo check --workspace`, `cargo clippy --workspace` and `cargo test --workspace`, all run on Linux, with the real output.

## Compile errors (fix)

1. `crates/menew/src/main.rs` and `crates/menew-ui/src/lib.rs`: `adw::` and `glib::` are used without imports (`use libadwaita as adw;`, `use gtk4::glib;`).
2. `main.rs:113`: `tray_rx`/`shortcut_rx` are moved inside `connect_activate` (an `Fn` closure). Create the palette and the receivers only once (e.g. `Option` in a `RefCell`, or set up in `startup`).
3. `crates/menew-clipboard/src/lib.rs`: `warn!` is not imported.
4. Check against the real API versions: `ksni` 0.3 (`spawn()` is async), `ashpd` 0.10 (`bind_shortcuts` signature, `GlobalShortcuts` clone).

## Runtime bugs (fix)

5. `menew-clipboard` `start_gnome_dbus_listener`: the `zbus` connection is dropped when the function returns, so the D-Bus name disappears. Keep the connection alive for the whole process.
6. `menew toggle`:
   - `run_with_args(&[])` throws the args away. Pass the real args.
   - On the primary instance, `toggle` must call `palette.toggle()`, not `activate()` (which creates another window).
   - Tray, D-Bus and the clipboard watcher must start **only on the primary instance** (after GApplication registers, e.g. in `startup`). A second `menew toggle` process must not start any of that.
7. The KDE listener with `wl-paste --watch cat` read line by line splits multi-line copies, and a non-UTF-8 copy (image) kills the loop. Either implement data-control directly (`wayland-client` + `wayland-protocols` ext/wlr data-control), or use `wl-paste --watch` with a delimiter per item and binary-safe reading, filtering MIME types to text. Make the findings doc describe exactly what was implemented.
8. `menew-ui` `activate_index`: the `visible_apps` borrow is held while `hide()` runs. `hide()` clears the entry, which emits `search-changed` immediately, which leads to `refresh_list`, which does `borrow_mut`: a panic. Copy the item and drop the borrow before `hide()`.
9. Palette keys: the `SearchEntry` consumes Esc/Enter before the window's controller. Use `PropagationPhase::Capture` or `entry.connect_activate`/`stop-search`, plus `set_key_capture_widget`.
10. Calculator result: actually copy it to the clipboard (`gdk::Clipboard::set_text`).
11. Privacy: `is_likely_secret` always returns `false`, and the GNOME extension sends everything. Ignore content marked by password managers: MIME `x-kde-passwordManagerHint` (value `secret`) and similar. The extension must check `selection.get_mimetypes()` before sending. Not saving passwords is a requirement.

## Minor (fix)

12. Extension `metadata.json`: add current shell versions (49+) and use a UUID without a personal username (e.g. `menew-bridge@menew.web.app`).
13. The tray asks for icon `menew`, which doesn't exist. Add an SVG icon (or use an existing symbolic one) and install it in `hicolor`.
14. Don't block GTK inside `#[tokio::main]`. Run tokio on its own thread (runtime + `std::thread`) and let GTK own the main thread. Talk to it via channels and `glib::MainContext`.
15. In `spike-findings.md`, remove unobserved claims ("instantaneous, flicker-free", "crashes on GNOME") or mark them as unverified.
16. (Optional, note it as a decision) The hand-written i18n should become gettext or Fluent before the alpha.

## When done

Update `docs/spike-summary.md` with what changed and the real Linux results (check/clippy/test). Then do the same hand back as before:

```sh
cockpit send --tab-id t57 --enter "agy finished review-1 fixes in menew-linux. Please review again (docs/spike-summary.md and docs/review-1.md)."
```

Don't commit yet.
