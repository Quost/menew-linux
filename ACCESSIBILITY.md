# Accessibility

We want Menew to work for everyone who uses Linux. This page says honestly what works today, what doesn't yet, and how to tell us about a barrier.

## What works

- **Native controls.** Menew is built with GTK4 and libadwaita, so text, buttons, lists and menus are standard GTK widgets exposed to assistive technologies through AT-SPI.
- **Dark and light mode** follow the desktop's style preference (libadwaita).
- **Keyboard.** The palette works entirely from the keyboard: type to search, arrows to move, Return to run or copy, Tab to switch between apps and clipboard, Ctrl+P to pin, Ctrl+Delete to delete, Esc to close.
- **Five languages**: English, Portuguese, German, Spanish and Chinese.

## Known limitations

- **Screen readers (Orca) haven't been tested yet.** Some icon-only elements may not have a spoken label.
- The global shortcuts depend on the desktop's GlobalShortcuts portal; where it is missing you have to set a system shortcut that runs `menew toggle`.
- Nothing has been tested yet with large text or high-contrast themes.

We're working on these. Pull requests that improve accessibility are very welcome.

## Supported environments

GNOME and KDE Plasma on Wayland (X11 is best effort), on distributions with GTK 4 and libadwaita 1.2 or later.

## Report a barrier

[Open an issue](https://github.com/Quost/menew-linux/issues/new) and tell us what you tried, what assistive technology you use (Orca, on-screen keyboard, zoom…), your distribution and desktop, and what got in the way. Accessibility bugs are treated as bugs, not feature requests.
