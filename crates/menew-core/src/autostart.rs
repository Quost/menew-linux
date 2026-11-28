//! "Start at login": an XDG autostart entry in `~/.config/autostart/menew.desktop`.
//! On by default for a new user (the first start writes it, like the Mac's `LaunchAtLogin.applyDefault`);
//! after that only the user's switch changes it.

use std::path::{Path, PathBuf};

pub const AUTOSTART_FILE: &str = "menew.desktop";

/// Quote a path for the `Exec` key of a desktop entry when it has spaces or special characters.
fn exec_quote(path: &str) -> String {
    let needs_quotes = path.chars().any(|c| {
        matches!(
            c,
            ' ' | '\t' | '\n' | '"' | '\'' | '\\' | '>' | '<' | '~' | '|' | '&' | ';' | '$' | '*' | '?' | '#' | '(' | ')' | '`'
        )
    });
    if !needs_quotes {
        return path.replace('%', "%%");
    }
    let mut out = String::from("\"");
    for c in path.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    // `%` starts a field code in Exec and must be doubled.
    out.replace('%', "%%")
}

/// Content of the autostart entry for a given `$APPIMAGE` value.
/// With an AppImage the entry must launch the AppImage file itself, since `menew` is not on PATH.
pub fn desktop_file_content_for(appimage: Option<&str>) -> String {
    let exec = match appimage.map(str::trim).filter(|p| !p.is_empty()) {
        Some(path) => exec_quote(path),
        None => "menew".to_string(),
    };
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Menew\n\
         Comment=Command palette and clipboard history\n\
         Exec={exec}\n\
         Terminal=false\n\
         NoDisplay=true\n\
         X-GNOME-Autostart-enabled=true\n"
    )
}

/// Content of the autostart entry. `Exec=menew` starts the daemon hidden (tray only);
/// when running from an AppImage (`$APPIMAGE` set) it uses the AppImage path instead.
pub fn desktop_file_content() -> String {
    desktop_file_content_for(std::env::var("APPIMAGE").ok().as_deref())
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
        let c = desktop_file_content_for(None);
        assert!(c.starts_with("[Desktop Entry]\n"));
        assert!(c.lines().any(|l| l == "Exec=menew"));
        assert!(c.lines().any(|l| l == "Type=Application"));
        assert!(!c.contains('\u{2014}'));
    }

    #[test]
    fn plain_install_uses_menew_command() {
        let c = desktop_file_content_for(None);
        assert!(c.lines().any(|l| l == "Exec=menew"));
        // an empty $APPIMAGE counts as not set
        assert_eq!(desktop_file_content_for(Some("")), c);
    }

    #[test]
    fn appimage_uses_the_appimage_path() {
        let c = desktop_file_content_for(Some("/home/u/Apps/Menew-0.1.0-alpha.1-x86_64.AppImage"));
        assert!(c
            .lines()
            .any(|l| l == "Exec=/home/u/Apps/Menew-0.1.0-alpha.1-x86_64.AppImage"));
        assert!(!c.lines().any(|l| l == "Exec=menew"));
    }

    #[test]
    fn appimage_path_with_spaces_is_quoted() {
        let c = desktop_file_content_for(Some("/home/u/My Apps/Menew.AppImage"));
        assert!(c
            .lines()
            .any(|l| l == "Exec=\"/home/u/My Apps/Menew.AppImage\""));
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
