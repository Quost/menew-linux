//! First run: what Menew sets up by itself, like the Mac's `LaunchAtLogin.applyDefault` and welcome tour
//! (`Onboarding.swift`). A new user gets "start at login" on, the GNOME extension turned on and, where the
//! desktop has no GlobalShortcuts portal (GNOME 45 to 47), the shortcuts created as GNOME custom shortcuts.
//! Only the rules live here; the daemon does the gsettings and D-Bus work (`crates/menew/src/setup.rs`).

use std::path::Path;

/// The Menew GNOME Shell extension (clipboard history and window snapping on GNOME).
pub const BRIDGE_UUID: &str = "menew-bridge@pururucos.com.br";
/// Its name until 0.1.0-alpha.2.
pub const OLD_BRIDGE_UUID: &str = "menew-bridge@menew.web.app";
/// Extensions that give GNOME a tray (StatusNotifierItem host).
pub const APPINDICATOR_UUIDS: [&str; 2] = ["ubuntu-appindicators@ubuntu.com", "appindicatorsupport@rgcjonas.gmail.com"];
/// Marker in the data folder: the welcome window was shown (the Mac's `onboarded`).
pub const WELCOME_DONE_FILE: &str = "welcome-done";

/// True when nobody ran Menew with this account yet: no history, notes, config or autostart entry.
/// Checked before the database is opened (opening creates it).
pub fn is_new_user(data_dir: &Path, config_dir: &Path) -> bool {
    !data_dir.join("clipboard.db").exists()
        && !data_dir.join("notes.json").exists()
        && !crate::config::Config::path_in(config_dir).exists()
        && !crate::autostart::is_enabled_in(config_dir)
}

pub fn welcome_done_in(data_dir: &Path) -> bool {
    data_dir.join(WELCOME_DONE_FILE).exists()
}

pub fn mark_welcome_done_in(data_dir: &Path) -> std::io::Result<()> {
    crate::private_fs::create_private_dir(data_dir)?;
    crate::private_fs::write_private(&data_dir.join(WELCOME_DONE_FILE), b"1\n")
}

/// `org.gnome.shell enabled-extensions` with the bridge on and its old name gone. `None` when nothing changes.
pub fn enable_bridge(enabled: &[String]) -> Option<Vec<String>> {
    let mut out: Vec<String> = enabled.iter().filter(|u| u.as_str() != OLD_BRIDGE_UUID).cloned().collect();
    if !out.iter().any(|u| u == BRIDGE_UUID) {
        out.push(BRIDGE_UUID.to_string());
    }
    (out != enabled).then_some(out)
}

/// Only the rename, for whoever had turned on the old extension: its name replaced by the new one.
/// `None` when the old name is not in the list.
pub fn migrate_bridge(enabled: &[String]) -> Option<Vec<String>> {
    if !enabled.iter().any(|u| u == OLD_BRIDGE_UUID) {
        return None;
    }
    enable_bridge(enabled)
}

/// `org.gnome.shell disabled-extensions` without the bridge (GNOME keeps an extension off while it is there).
pub fn undisable_bridge(disabled: &[String]) -> Option<Vec<String>> {
    let out: Vec<String> = disabled.iter().filter(|u| u.as_str() != BRIDGE_UUID).cloned().collect();
    (out.len() != disabled.len()).then_some(out)
}

pub fn has_appindicator(enabled: &[String]) -> bool {
    enabled.iter().any(|u| APPINDICATOR_UUIDS.contains(&u.as_str()))
}

/// A GNOME custom shortcut Menew creates when there is no GlobalShortcuts portal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomShortcut {
    /// Part of the dconf path: `.../custom-keybindings/menew-<id>/`.
    pub id: &'static str,
    /// Key of its name in `strings.rs`.
    pub name_key: &'static str,
    /// What follows the program in the command.
    pub args: &'static str,
    /// GNOME accelerator, the same keys as the portal triggers.
    pub binding: &'static str,
}

/// The palette, clipboard, paste queue, screen and two quick actions. The window snapping keys are left out:
/// GNOME already uses Ctrl+Alt+Shift+Arrows (move to workspace) and snaps with Super+Arrows.
pub const GNOME_SHORTCUTS: [CustomShortcut; 7] = [
    CustomShortcut { id: "palette", name_key: "sc_palette", args: "toggle", binding: "<Control><Alt>space" },
    CustomShortcut { id: "clipboard", name_key: "sc_clipboard", args: "clipboard", binding: "<Control><Alt>v" },
    CustomShortcut { id: "paste-next", name_key: "sc_paste_next", args: "paste-next", binding: "<Control><Alt>n" },
    CustomShortcut { id: "capture", name_key: "shortcut_capture_desc", args: "capture", binding: "<Control><Shift><Alt>s" },
    CustomShortcut { id: "capture-text", name_key: "sc_capture_text", args: "capture-text", binding: "<Control><Shift><Alt>t" },
    CustomShortcut { id: "mic", name_key: "sc_mute_mic", args: "run qa.mic", binding: "<Control><Alt>m" },
    CustomShortcut { id: "note", name_key: "sc_new_note", args: "run note.new", binding: "<Control><Alt>j" },
];

pub const CUSTOM_KEYBINDINGS_DIR: &str = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/";

pub fn keybinding_path(id: &str) -> String {
    format!("{CUSTOM_KEYBINDINGS_DIR}menew-{id}/")
}

/// Quote a program path for a GNOME custom shortcut command (parsed with `g_shell_parse_argv`).
pub fn shell_quote(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "/._-+:=@".contains(c)) {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub fn shortcut_command(program: &str, s: &CustomShortcut) -> String {
    format!("{} {}", shell_quote(program), s.args)
}

/// An existing custom shortcut: its dconf path, command and keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingShortcut {
    pub path: String,
    pub command: String,
    pub binding: String,
}

fn same_keys(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// The shortcuts to create: not the ones Menew made before (same path), not a command that already runs
/// the same Menew command, and never keys another custom shortcut uses.
pub fn shortcuts_to_add(existing: &[ExistingShortcut]) -> Vec<CustomShortcut> {
    GNOME_SHORTCUTS
        .iter()
        .filter(|s| {
            let path = keybinding_path(s.id);
            !existing.iter().any(|e| {
                e.path == path
                    || same_keys(&e.binding, s.binding)
                    || (e.command.contains("menew") && e.command.trim_end().ends_with(&format!(" {}", s.args)))
            })
        })
        .copied()
        .collect()
}

/// How far one part of the setup is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Step {
    /// Not known yet (the portal answer or the shell has not come back).
    #[default]
    Checking,
    Done,
    /// Turned on, works after logging out and in again (GNOME on Wayland loads new extensions at login).
    NeedsLogout,
    /// Menew can't do it alone: the text says what to do.
    Missing,
    /// Not needed on this desktop.
    NotNeeded,
}

/// What the welcome window shows about the desktop.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SetupReport {
    pub gnome: bool,
    pub autostart: bool,
    pub clipboard: Step,
    /// Key of the text under the clipboard row when it is `Missing`.
    pub clipboard_hint: &'static str,
    pub shortcuts: Step,
    /// The shortcuts are GNOME custom shortcuts made by Menew (not the portal).
    pub shortcuts_custom: bool,
    pub tray: Step,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn enabling_adds_the_bridge_once_and_drops_the_old_name() {
        assert_eq!(enable_bridge(&v(&["a@b"])), Some(v(&["a@b", BRIDGE_UUID])));
        assert_eq!(enable_bridge(&v(&["a@b", BRIDGE_UUID])), None);
        assert_eq!(enable_bridge(&v(&[OLD_BRIDGE_UUID, "a@b"])), Some(v(&["a@b", BRIDGE_UUID])));
        assert_eq!(enable_bridge(&v(&[OLD_BRIDGE_UUID, BRIDGE_UUID])), Some(v(&[BRIDGE_UUID])));
    }

    #[test]
    fn migration_only_touches_lists_with_the_old_name() {
        assert_eq!(migrate_bridge(&v(&["a@b"])), None);
        assert_eq!(migrate_bridge(&v(&[])), None);
        assert_eq!(migrate_bridge(&v(&[OLD_BRIDGE_UUID])), Some(v(&[BRIDGE_UUID])));
    }

    #[test]
    fn undisable_removes_only_the_bridge() {
        assert_eq!(undisable_bridge(&v(&["x", BRIDGE_UUID])), Some(v(&["x"])));
        assert_eq!(undisable_bridge(&v(&["x"])), None);
    }

    #[test]
    fn appindicator_is_found_by_either_uuid() {
        assert!(has_appindicator(&v(&["ubuntu-appindicators@ubuntu.com"])));
        assert!(has_appindicator(&v(&["x", "appindicatorsupport@rgcjonas.gmail.com"])));
        assert!(!has_appindicator(&v(&["x"])));
    }

    #[test]
    fn new_user_has_no_history_notes_config_or_autostart() {
        let data = tempfile::tempdir().unwrap();
        let config = tempfile::tempdir().unwrap();
        assert!(is_new_user(data.path(), config.path()));
        std::fs::write(data.path().join("clipboard.db"), b"").unwrap();
        assert!(!is_new_user(data.path(), config.path()));
        std::fs::remove_file(data.path().join("clipboard.db")).unwrap();
        crate::autostart::set_enabled_in(config.path(), true).unwrap();
        assert!(!is_new_user(data.path(), config.path()));
    }

    #[test]
    fn welcome_marker_round_trips() {
        let data = tempfile::tempdir().unwrap();
        let dir = data.path().join("menew");
        assert!(!welcome_done_in(&dir));
        mark_welcome_done_in(&dir).unwrap();
        assert!(welcome_done_in(&dir));
    }

    #[test]
    fn commands_quote_paths_with_spaces() {
        let s = &GNOME_SHORTCUTS[0];
        assert_eq!(shortcut_command("menew", s), "menew toggle");
        assert_eq!(shortcut_command("/home/a/Apps/Menew 1.AppImage", s), "'/home/a/Apps/Menew 1.AppImage' toggle");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn shortcuts_skip_existing_paths_commands_and_keys() {
        assert_eq!(shortcuts_to_add(&[]).len(), GNOME_SHORTCUTS.len());
        let existing = vec![
            ExistingShortcut { path: keybinding_path("palette"), command: "menew toggle".into(), binding: "<Control><Alt>space".into() },
            ExistingShortcut { path: "/x/custom0/".into(), command: "/usr/bin/menew clipboard".into(), binding: "<Super>v".into() },
            ExistingShortcut { path: "/x/custom1/".into(), command: "gnome-terminal".into(), binding: "<control><alt>N".into() },
        ];
        let ids: Vec<&str> = shortcuts_to_add(&existing).iter().map(|s| s.id).collect();
        assert_eq!(ids, vec!["capture", "capture-text", "mic", "note"]);
    }

    #[test]
    fn shortcut_names_are_translated() {
        for s in GNOME_SHORTCUTS {
            assert!(crate::strings::lookup(s.name_key, crate::Language::En).is_some(), "{} has no text", s.name_key);
        }
    }

    #[test]
    fn custom_shortcuts_match_the_portal_triggers() {
        // the same keys as the GlobalShortcuts portal asks for (`menew-shortcuts`)
        let portal = [
            ("toggle", "<Control><Alt>space"),
            ("clipboard", "<Control><Alt>v"),
            ("paste-next", "<Control><Alt>n"),
            ("capture", "<Control><Shift><Alt>s"),
            ("capture-text", "<Control><Shift><Alt>t"),
        ];
        for (args, keys) in portal {
            assert!(GNOME_SHORTCUTS.iter().any(|s| s.args == args && s.binding == keys), "{args}");
        }
    }
}
