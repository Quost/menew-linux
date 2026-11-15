//! User configuration stored in `~/.config/menew/config.json`.
//!
//! Reading never fails: a missing, unreadable or malformed file yields the defaults.

use crate::quick_links::QuickLink;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// How a clipboard entry is delivered to the previously focused application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PasteMode {
    /// Inject Ctrl+V through uinput when `/dev/uinput` is writable, otherwise copy only.
    #[default]
    #[serde(rename = "auto")]
    Auto,
    /// Only copy to the clipboard and show a notification.
    #[serde(rename = "copy-only")]
    CopyOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub paste_mode: PasteMode,
    /// Quick links of the palette (`{q}` is where the typed text goes). When the key is missing the
    /// default list of the Mac app is used; when present it replaces that list.
    pub quick_links: Option<Vec<QuickLink>>,
    /// World clocks: tz database names ("Europe/Lisbon"). Settings come in Phase 11.
    pub world_clocks: Vec<String>,
    /// A 5 minute break after a focus session (on by default, like the Mac).
    pub focus_breaks: bool,
    /// Apps whose copies are not kept (`ignoredApps` on the Mac): the application id or window class,
    /// e.g. "org.keepassxc.KeePassXC". Only applied where the desktop tells which app copied (GNOME).
    pub clipboard_ignored_apps: Vec<String>,
    /// How many unpinned entries the history keeps (`maxItems`: 300, never below 20).
    pub clipboard_max_items: usize,
    /// Entries older than this many days are removed; 0 keeps them (`autoDeleteDays`).
    pub clipboard_auto_delete_days: u32,
    /// Read the text of copied images with tesseract (`clipboardOCR`, on by default).
    pub clipboard_ocr: bool,
    /// Apps "Quit all apps" never closes (`neverQuit` on the Mac): desktop ids or program names.
    pub never_quit: Vec<String>,
    /// Metrics shown next to the tray icon (`menuBarMetrics`): see `sysmon::TRAY_METRICS`.
    pub tray_metrics: Vec<String>,
    /// The features that can be turned off completely (`Module` on the Mac). With the clipboard
    /// off, Menew does not even read it.
    pub module_clipboard: bool,
    pub module_actions: bool,
    pub module_notes: bool,
    /// `system` or a language code (`en`, `pt`, `de`, `es`, `zh`).
    pub language: String,
    /// Command ids on top of the Actions tab, in order (`actionFavorites`).
    pub favorites: Vec<String>,
    /// Check GitHub releases once a day (off until turned on, in the welcome tour or settings).
    pub update_check: bool,
    /// Unix time of the last update check.
    pub update_checked_at: u64,
    /// The welcome tour was seen.
    pub welcome_done: bool,
}

/// The Mac's default favorites, with the Linux command ids.
pub fn default_favorites() -> Vec<String> {
    ["qa.mic", "qa.awake", "qa.dark", "cap.area", "cap.ocr", "qa.color", "qa.lock", "qa.plain"].iter().map(|s| s.to_string()).collect()
}

impl Config {
    /// The language to use: the picked one, or the system's.
    pub fn effective_language(&self) -> crate::i18n::Language {
        match self.language.as_str() {
            "" | "system" => crate::i18n::Language::detect(),
            code => crate::i18n::Language::from_code(code),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            paste_mode: PasteMode::Auto,
            quick_links: None,
            world_clocks: vec![],
            focus_breaks: true,
            clipboard_ignored_apps: vec![],
            clipboard_max_items: crate::clip_item::DEFAULT_MAX_ITEMS,
            clipboard_auto_delete_days: 0,
            clipboard_ocr: true,
            never_quit: vec![],
            tray_metrics: vec![],
            module_clipboard: true,
            module_actions: true,
            module_notes: true,
            language: "system".into(),
            favorites: default_favorites(),
            update_check: false,
            update_checked_at: 0,
            welcome_done: false,
        }
    }
}

/// Resolve the user config directory from the environment values (pure, testable).
/// `xdg_config_home` is only honoured when it is an absolute path, per the XDG spec.
pub fn config_dir_from(xdg_config_home: Option<&str>, home: Option<&Path>) -> PathBuf {
    if let Some(x) = xdg_config_home {
        let p = Path::new(x);
        if p.is_absolute() {
            return p.to_path_buf();
        }
    }
    match home {
        Some(h) => h.join(".config"),
        None => PathBuf::from(".config"),
    }
}

/// The real user config directory (`$XDG_CONFIG_HOME` or `$HOME/.config`).
pub fn config_dir() -> PathBuf {
    let xdg = std::env::var("XDG_CONFIG_HOME").ok();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    config_dir_from(xdg.as_deref().filter(|s| !s.is_empty()), home.as_deref())
}

impl Config {
    pub fn path_in(config_dir: &Path) -> PathBuf {
        config_dir.join("menew").join("config.json")
    }

    pub fn default_path() -> PathBuf {
        Self::path_in(&config_dir())
    }

    /// Load from `path`; any problem yields the defaults.
    pub fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn load() -> Self {
        Self::load_from(&Self::default_path())
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(std::io::Error::other)?;
        std::fs::write(path, json + "\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_yields_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config::load_from(&Config::path_in(dir.path()));
        assert_eq!(cfg, Config::default());
        assert_eq!(cfg.paste_mode, PasteMode::Auto);
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = Config::path_in(dir.path());
        let cfg = Config { paste_mode: PasteMode::CopyOnly, ..Config::default() };
        cfg.save_to(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"copy-only\""), "{text}");
        assert_eq!(Config::load_from(&path), cfg);
    }

    #[test]
    fn parses_documented_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.json");
        std::fs::write(&path, r#"{"paste_mode": "auto"}"#).unwrap();
        assert_eq!(Config::load_from(&path).paste_mode, PasteMode::Auto);
        std::fs::write(&path, r#"{"paste_mode": "copy-only"}"#).unwrap();
        assert_eq!(Config::load_from(&path).paste_mode, PasteMode::CopyOnly);
    }

    #[test]
    fn malformed_or_unknown_falls_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.json");
        std::fs::write(&path, "not json {").unwrap();
        assert_eq!(Config::load_from(&path), Config::default());
        std::fs::write(&path, r#"{"paste_mode": "bogus"}"#).unwrap();
        assert_eq!(Config::load_from(&path), Config::default());
        // an object without the key keeps the default, unknown keys are ignored
        std::fs::write(&path, r#"{"other": 1}"#).unwrap();
        assert_eq!(Config::load_from(&path), Config::default());
    }

    #[test]
    fn config_dir_resolution() {
        let home = Path::new("/home/u");
        assert_eq!(config_dir_from(None, Some(home)), PathBuf::from("/home/u/.config"));
        assert_eq!(config_dir_from(Some("/x/cfg"), Some(home)), PathBuf::from("/x/cfg"));
        // relative XDG_CONFIG_HOME is ignored
        assert_eq!(config_dir_from(Some("rel"), Some(home)), PathBuf::from("/home/u/.config"));
    }
}
