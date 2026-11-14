# Menew GNOME Shell Extension (Clipboard Bridge)

This extension bridges Wayland clipboard selection events to the Menew Linux background daemon.

## Why this is necessary on GNOME Wayland

Under Wayland, security isolation prevents unfocused background applications from reading the system clipboard.
Unlike wlroots-based compositors and KDE Plasma, GNOME's window manager (Mutter) does not implement the `wlr-data-control` or `ext-data-control` protocols.

Because of this architectural decision:
- An unfocused Linux application cannot receive clipboard change events directly.
- GNOME Shell extensions run inside the Mutter compositor process with privileged access to `global.display.get_selection()` and `Meta.Selection`.
- This extension listens to `owner-changed` on `Meta.SelectionType.SELECTION_CLIPBOARD`, inspects `get_mimetypes()` to reject passwords (`x-kde-passwordManagerHint`, `ConcealedType`), retrieves the copied text via `St.Clipboard`, and sends it to the Menew daemon over session D-Bus (`org.menew.ClipboardService`).

On KDE Plasma, this extension is **not needed** because KDE natively supports data-control protocols (`ext-data-control` and `wlr-data-control`).

## Installation

Run the included install script:

```bash
./install.sh
```

Or manually copy the extension files:

```bash
mkdir -p ~/.local/share/gnome-shell/extensions/menew-bridge@menew.web.app
cp metadata.json extension.js ~/.local/share/gnome-shell/extensions/menew-bridge@menew.web.app/
```

Restart GNOME Shell (log out and back in on Wayland) and enable the extension:

```bash
gnome-extensions enable menew-bridge@menew.web.app
```

## D-Bus Interface

- **Service Name**: `org.menew.ClipboardService`
- **Object Path**: `/org/menew/Clipboard`
- **Interface**: `org.menew.ClipboardService`
- **Method**: `PushClipboardText(s: text, s: mime) -> (b: success)`
