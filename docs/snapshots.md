# Screenshots (`menew --snapshot`)

The port of the Mac app's `Sources/Menew/Snapshot.swift`. `menew --snapshot <dir> [-language en|pt|de|es|zh]` draws each screen
in light and dark with libadwaita's color scheme forced, saves one PNG per screen and exits. `scripts/snapshots.sh` runs it for the
five languages in a headless GNOME session: English goes to `docs/assets/`, the others to `docs/assets/<lang>/`, with the same file names.

## Fake data only

`--snapshot` is a public command, like on the Mac, but it never reads real data:

- the clipboard is an **in-memory database** filled with an invented history (a pinned note, code, files, a link, a color, two images
  drawn in the app, a text copied just now); the images are written to a temporary folder that is removed at the end;
- the apps are a **fixed list of 10 entries** (`demo-*.desktop`, files that do not exist), never the `.desktop` files of the machine;
- snippets are invented, recents, timers, focus and the paste queue are empty, the scripts folder is not read and `config.json` is not read;
- the clipboard of the session is not read and not written; the running daemon is never contacted (a separate, non-unique application id);
- nothing is written outside the folder it was given.

`scripts/snapshots.sh` checks this in the session it starts: it creates a bait app, a bait database and a bait config in the temporary
`HOME`, runs the snapshots and compares checksums afterwards. `cargo test -p menew-ui snapshot_uses_no_real_data` checks the app list.

## Screens

What the Mac captures (popover tabs, hub sections, graph, welcome tour, settings) and what exists on Linux:

| Mac screen | Linux file (`-light.png` / `-dark.png`) | Shows |
|---|---|---|
| Clipboard popover | `clipboard` | History with filters, a pinned note, a link, an image, and the preview pane |
| Clipboard, images | `clipboard-images` | The Images filter: thumbnails, preview and the text read from the image |
| Snippets | `clipboard-snippets` | The Snippets filter with variables |
| Command palette | `palette` | The palette with an empty search (actions and tips) |
| Command palette, apps | `palette-apps` | Search `te`: apps, a snippet and actions |
| Palette, conversion | `palette-conversion` | `5 km to mi` |
| Palette, emoji | `palette-emoji` | `emoji rocket` (needs Noto Color Emoji) |
| Palette, text transform | `palette-transform` | `snake_case` previewed on the copied text |
| Tray menu | not captured | The menu is drawn by the shell or the panel (a `dbusmenu`), not by Menew, so the app cannot render it |
| Notes | not yet | Joins after Phase 6 (notes) is merged: add `"notes"` to `SCREENS` and a case in `apply` in `crates/menew-ui/src/snapshot.rs` |

The Mac's hub, memory, recordings, agents, graph, system monitor and welcome screens have no Linux counterpart yet (see `parity.md`).

## Adding a screen

1. Add its name to `SCREENS` in `crates/menew-ui/src/snapshot.rs` and a case in `apply` that puts the palette in that state.
2. Run `bash scripts/snapshots.sh` and open the images.
3. Reference them from the README with the usual `<picture>` for light and dark.

## How the image is made

The palette card is drawn by the app itself (`PaletteWindow::capture_png`): the card with a soft shadow on a transparent margin,
so there is no desktop, panel or window frame to crop. Fonts come from the session: Cantarell, the Adwaita icon theme and Noto Color
Emoji (and a CJK font for Chinese); the script stops with an error when one is missing instead of drawing boxes.
