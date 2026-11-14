//! "Start at login": an XDG autostart entry in `~/.config/autostart/menew.desktop`.
//! Opt-in: nothing is written unless the user enables it.

use std::path::{Path, PathBuf};

pub const AUTOSTART_FILE: &str = "menew.desktop";

/// Content of the autostart entry. `Exec=menew` starts the daemon hidden (tray only).
pub fn desktop_file_content() -> String {
    "[Desktop Entry]\n\
     Type=Application\n\
     Name=Menew\n\
     Comment=Command palette and clipboard history\n\
     Exec=menew\n\
     Terminal=false\n\
     NoDisplay=true\n\
     X-GNOME-Autostart-enabled=true\n"
        .to_string()
}

pub fn autostart_path_in(config_dir: &Path) -> PathBuf {
    config_dir.join("autostart").join(AUTOSTART_FILE)
}

pub fn autostart_path() -> PathBuf {
    autostart_path_in(&crate::config::config_dir())
}

pub fn is_enabled_in(config_dir: &Path) -> bool {
    autostart_path_in(config_dir).is_file()
}

pub fn is_enabled() -> bool {
    is_enabled_in(&crate::config::config_dir())
}

pub fn set_enabled_in(config_dir: &Path, enabled: bool) -> std::io::Result<()> {
    let path = autostart_path_in(config_dir);
    if enabled {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, desktop_file_content())
    } else {
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
}

pub fn set_enabled(enabled: bool) -> std::io::Result<()> {
    set_enabled_in(&crate::config::config_dir(), enabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::config_dir_from;

    #[test]
    fn content_is_a_valid_hidden_start_entry() {
        let c = desktop_file_content();
        assert!(c.starts_with("[Desktop Entry]\n"));
        assert!(c.lines().any(|l| l == "Exec=menew"));
        assert!(c.lines().any(|l| l == "Type=Application"));
        assert!(!c.contains('\u{2014}'));
    }

    #[test]
    fn path_lives_under_home_config_autostart() {
        let home = tempfile::tempdir().unwrap();
        let cfg = config_dir_from(None, Some(home.path()));
        assert_eq!(
            autostart_path_in(&cfg),
            home.path().join(".config/autostart/menew.desktop")
        );
    }

    #[test]
    fn off_by_default_then_enable_and_disable() {
        let home = tempfile::tempdir().unwrap();
        let cfg = config_dir_from(None, Some(home.path()));
        assert!(!is_enabled_in(&cfg));
        // disabling when absent is not an error
        set_enabled_in(&cfg, false).unwrap();

        set_enabled_in(&cfg, true).unwrap();
        assert!(is_enabled_in(&cfg));
        let written = std::fs::read_to_string(autostart_path_in(&cfg)).unwrap();
        assert_eq!(written, desktop_file_content());

        set_enabled_in(&cfg, false).unwrap();
        assert!(!is_enabled_in(&cfg));
    }
}
