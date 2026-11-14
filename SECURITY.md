# Security policy

Menew reads your clipboard, can inject a paste keystroke through `/dev/uinput` when you allow it, and talks to a GNOME Shell extension over D-Bus, so security reports matter a lot to us.

## Supported versions

Only the latest release gets security fixes.

| Version | Supported |
|---|---|
| Latest release | ✅ |
| Older releases | ❌ |

## Reporting a vulnerability

**Please don't open a public issue.** Report it privately instead:

1. Go to the [Security tab](https://github.com/Quost/menew-linux/security) of this repository.
2. Click **Report a vulnerability** and describe the problem, the steps to reproduce it, the Menew version, your distribution and desktop (GNOME/KDE, Wayland/X11).

What happens next:

- You'll get a reply within **7 days**.
- Once confirmed, we'll work on a fix and ship it as a new release, usually within 30 days depending on severity.
- We'll credit you in the release notes, unless you'd rather stay anonymous.

## Scope

Good examples of things to report:

- Clipboard content leaving your computer, or being readable by other users or apps (for example through the `org.menew.ClipboardService` D-Bus name).
- Passwords marked by password managers (`x-kde-passwordManagerHint`) ending up in the history.
- Ways to abuse the uinput paste or the GNOME extension to inject input or read the clipboard without the user's action.
- Problems with the release artifacts (.deb, AppImage, checksums).

Menew has no servers and no accounts, and sends no telemetry.
