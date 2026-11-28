//! First run and the welcome window. The rules are in `menew_core::setup`; this file does the work on the
//! desktop, like the Mac turning "open at login" on for new users (`LaunchAtLogin.applyDefault`):
//! - start at login: on for a new user (the XDG autostart entry);
//! - GNOME: the Menew extension turned on in `org.gnome.shell enabled-extensions` (GNOME on Wayland loads it at
//!   the next login), its old name `menew-bridge@menew.web.app` replaced for everyone, and its files copied to
//!   `~/.local/share/gnome-shell/extensions` when Menew does not come from the .deb (AppImage, a build);
//! - GNOME without the GlobalShortcuts portal (45 to 47): the shortcuts created as GNOME custom shortcuts.
//!
//! Nothing here touches the network. With `MENEW_TEST=1` nothing is changed and the welcome never opens by itself.

use gtk4::gio::{self, prelude::*};
use gtk4::glib;
use libadwaita as adw;
use menew_core::setup::{self as rules, ExistingShortcut, SetupReport, Step};
use menew_core::strings::tr;
use menew_core::sys::Desktop;
use menew_core::{autostart, current_language};
use menew_ui::welcome::{WelcomeHooks, WelcomeWindow};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use tracing::{info, warn};

const EXT_JS: &str = include_str!("../../../gnome-extension/extension.js");
const EXT_META: &str = include_str!("../../../gnome-extension/metadata.json");
const SHELL_SCHEMA: &str = "org.gnome.shell";
const MEDIA_KEYS_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
const CUSTOM_KEY_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";

fn settings(schema: &str) -> Option<gio::Settings> {
    gio::SettingsSchemaSource::default()?.lookup(schema, true)?;
    Some(gio::Settings::new(schema))
}

fn strv(s: &gio::Settings, key: &str) -> Vec<String> {
    s.strv(key).iter().map(|g| g.to_string()).collect()
}

fn set_strv(s: &gio::Settings, key: &str, v: &[String]) -> bool {
    let refs: Vec<&str> = v.iter().map(String::as_str).collect();
    match s.set_strv(key, refs.as_slice()) {
        Ok(()) => true,
        Err(e) => {
            warn!("Setup: could not write {}: {}", key, e);
            false
        }
    }
}

/// The program the custom shortcuts run: the AppImage, a build run from its folder, or `menew` from the .deb.
fn program() -> String {
    if let Some(a) = std::env::var("APPIMAGE").ok().filter(|a| !a.trim().is_empty()) {
        return a;
    }
    match std::env::current_exe() {
        Ok(p) if p != std::path::Path::new("/usr/bin/menew") => p.to_string_lossy().into_owned(),
        _ => "menew".to_string(),
    }
}

fn system_extension_dir() -> PathBuf {
    PathBuf::from("/usr/share/gnome-shell/extensions").join(rules::BRIDGE_UUID)
}

fn user_extension_dir() -> PathBuf {
    gio::glib::user_data_dir().join("gnome-shell/extensions").join(rules::BRIDGE_UUID)
}

/// Make sure GNOME can find the extension: the .deb installs it for everyone; otherwise Menew writes its own
/// copy (the files are built into the binary) and keeps it up to date.
fn ensure_extension_files() -> bool {
    if system_extension_dir().join("metadata.json").is_file() && !user_extension_dir().exists() {
        return true;
    }
    let dir = user_extension_dir();
    let same = |name: &str, want: &str| std::fs::read_to_string(dir.join(name)).map(|s| s == want).unwrap_or(false);
    if same("metadata.json", EXT_META) && same("extension.js", EXT_JS) {
        return true;
    }
    let result = std::fs::create_dir_all(&dir)
        .and_then(|_| std::fs::write(dir.join("metadata.json"), EXT_META))
        .and_then(|_| std::fs::write(dir.join("extension.js"), EXT_JS));
    match result {
        Ok(()) => {
            info!("Setup: GNOME extension written to {}", dir.display());
            true
        }
        Err(e) => {
            warn!("Setup: could not write the GNOME extension to {}: {}", dir.display(), e);
            false
        }
    }
}

pub struct Setup {
    app: adw::Application,
    desktop: Desktop,
    data_dir: PathBuf,
    /// Changes the desktop and opens the welcome by itself (false with `MENEW_TEST=1`).
    active: bool,
    /// A new user whose welcome was not closed yet.
    pending: Cell<bool>,
    report: RefCell<SetupReport>,
    welcome: RefCell<Option<Rc<WelcomeWindow>>>,
    open_menew: RefCell<Option<Rc<dyn Fn()>>>,
}

impl Setup {
    /// At the start of the primary instance, before the database is opened (that creates it).
    pub fn new(app: &adw::Application, test_mode: bool) -> Rc<Self> {
        let data_dir = menew_core::clipboard_db::data_dir();
        let config_dir = menew_core::config::config_dir();
        let new_user = rules::is_new_user(&data_dir, &config_dir);
        let done = rules::welcome_done_in(&data_dir);
        let desktop = Desktop::detect();
        let active = !test_mode;
        let me = Rc::new(Self {
            app: app.clone(),
            desktop,
            data_dir,
            active,
            pending: Cell::new(active && new_user && !done),
            report: RefCell::new(SetupReport { gnome: desktop == Desktop::Gnome, ..Default::default() }),
            welcome: RefCell::new(None),
            open_menew: RefCell::new(None),
        });
        if !active {
            return me;
        }
        if !new_user && !done {
            // who already used Menew never gets the welcome by itself (the palette still opens it)
            let _ = rules::mark_welcome_done_in(&me.data_dir);
        }
        if desktop == Desktop::Gnome {
            me.migrate_extension();
        }
        if me.pending.get() {
            info!("Setup: first run on {}", desktop.name());
            if let Err(e) = autostart::set_enabled(true) {
                warn!("Setup: could not turn on start at login: {}", e);
            }
            if desktop == Desktop::Gnome {
                me.turn_on_extension();
            }
        }
        me.refresh();
        me
    }

    pub fn set_open_menew(&self, f: Rc<dyn Fn()>) {
        *self.open_menew.borrow_mut() = Some(f);
    }

    /// True until a new user closes the welcome window.
    pub fn pending(&self) -> bool {
        self.pending.get()
    }

    fn migrate_extension(&self) {
        let Some(s) = settings(SHELL_SCHEMA) else { return };
        if let Some(list) = rules::migrate_bridge(&strv(&s, "enabled-extensions")) {
            if set_strv(&s, "enabled-extensions", &list) {
                gio::Settings::sync();
                info!("Setup: the GNOME extension is now {}", rules::BRIDGE_UUID);
            }
        }
    }

    /// Turn the extension on. GNOME reads `enabled-extensions` at login; a running shell that already knows the
    /// extension (it was there at login) is asked to load it now too.
    fn turn_on_extension(&self) -> bool {
        if !ensure_extension_files() {
            return false;
        }
        let Some(s) = settings(SHELL_SCHEMA) else {
            warn!("Setup: the org.gnome.shell settings are not installed");
            return false;
        };
        if let Some(list) = rules::undisable_bridge(&strv(&s, "disabled-extensions")) {
            set_strv(&s, "disabled-extensions", &list);
        }
        if let Some(list) = rules::enable_bridge(&strv(&s, "enabled-extensions")) {
            if !set_strv(&s, "enabled-extensions", &list) {
                return false;
            }
        }
        gio::Settings::sync();
        info!("Setup: GNOME extension {} turned on", rules::BRIDGE_UUID);
        if let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) {
            bus.call(
                Some("org.gnome.Shell.Extensions"),
                "/org/gnome/Shell/Extensions",
                "org.gnome.Shell.Extensions",
                "EnableExtension",
                Some(&(rules::BRIDGE_UUID,).to_variant()),
                None,
                gio::DBusCallFlags::NO_AUTO_START,
                2000,
                gio::Cancellable::NONE,
                |_| {},
            );
        }
        true
    }

    fn custom_shortcuts() -> Option<(gio::Settings, Vec<ExistingShortcut>)> {
        let mk = settings(MEDIA_KEYS_SCHEMA)?;
        gio::SettingsSchemaSource::default()?.lookup(CUSTOM_KEY_SCHEMA, true)?;
        let existing = strv(&mk, "custom-keybindings")
            .into_iter()
            .map(|path| {
                let s = gio::Settings::with_path(CUSTOM_KEY_SCHEMA, &path);
                ExistingShortcut { command: s.string("command").to_string(), binding: s.string("binding").to_string(), path }
            })
            .collect();
        Some((mk, existing))
    }

    fn has_own_shortcuts() -> bool {
        let prefix = format!("{}menew-", rules::CUSTOM_KEYBINDINGS_DIR);
        Self::custom_shortcuts().map(|(_, e)| e.iter().any(|s| s.path.starts_with(&prefix))).unwrap_or(false)
    }

    /// Create the GNOME custom shortcuts that are missing. Returns false when GNOME's settings are not there.
    fn create_shortcuts(&self) -> bool {
        let Some((mk, existing)) = Self::custom_shortcuts() else {
            warn!("Setup: GNOME custom shortcuts are not available");
            return false;
        };
        let lang = current_language();
        let program = program();
        let mut paths: Vec<String> = existing.iter().map(|e| e.path.clone()).collect();
        let add = rules::shortcuts_to_add(&existing);
        for s in &add {
            let path = rules::keybinding_path(s.id);
            let k = gio::Settings::with_path(CUSTOM_KEY_SCHEMA, &path);
            let name = format!("Menew: {}", tr(s.name_key, lang, &[]));
            let ok = k.set_string("name", &name).is_ok()
                && k.set_string("command", &rules::shortcut_command(&program, s)).is_ok()
                && k.set_string("binding", s.binding).is_ok();
            if ok && !paths.contains(&path) {
                paths.push(path);
            }
        }
        let ok = set_strv(&mk, "custom-keybindings", &paths);
        gio::Settings::sync();
        info!("Setup: {} GNOME custom shortcuts created", add.len());
        ok
    }

    /// The answer of the GlobalShortcuts portal (`true`: the shortcuts are bound).
    pub fn portal_status(self: &Rc<Self>, bound: bool) {
        let gnome = self.desktop == Desktop::Gnome;
        let (step, custom) = if bound {
            (Step::Done, false)
        } else if gnome && (Self::has_own_shortcuts() || (self.active && self.pending.get() && self.create_shortcuts())) {
            (Step::Done, true)
        } else {
            (Step::Missing, false)
        };
        {
            let mut r = self.report.borrow_mut();
            r.shortcuts = step;
            r.shortcuts_custom = custom;
        }
        self.push();
    }

    /// Read the state of the clipboard and the tray again, then show it.
    pub fn refresh(self: &Rc<Self>) {
        self.report.borrow_mut().autostart = autostart::is_enabled();
        if self.desktop != Desktop::Gnome {
            let mut r = self.report.borrow_mut();
            if menew_core::sys::has("wl-paste") {
                r.clipboard = Step::Done;
                r.clipboard_hint = "";
            } else {
                r.clipboard = Step::Missing;
                r.clipboard_hint = "setup_clip_need_wl";
            }
        }
        let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else {
            self.push();
            return;
        };
        // the tray: someone hosts StatusNotifierItems (KDE always; GNOME with the AppIndicator extension)
        let me = Rc::downgrade(self);
        bus.call(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "NameHasOwner",
            Some(&("org.kde.StatusNotifierWatcher",).to_variant()),
            None,
            gio::DBusCallFlags::NO_AUTO_START,
            2000,
            gio::Cancellable::NONE,
            move |res| {
                let Some(me) = me.upgrade() else { return };
                let owned = res.ok().and_then(|v| v.get::<(bool,)>()).map(|(b,)| b).unwrap_or(false);
                me.report.borrow_mut().tray = if owned { Step::Done } else { Step::Missing };
                me.push();
            },
        );
        if self.desktop != Desktop::Gnome {
            self.push();
            return;
        }
        let enabled = settings(SHELL_SCHEMA).map(|s| strv(&s, "enabled-extensions").iter().any(|u| u == rules::BRIDGE_UUID)).unwrap_or(false);
        let me = Rc::downgrade(self);
        bus.call(
            Some("org.gnome.Shell.Extensions"),
            "/org/gnome/Shell/Extensions",
            "org.gnome.Shell.Extensions",
            "GetExtensionInfo",
            Some(&(rules::BRIDGE_UUID,).to_variant()),
            None,
            gio::DBusCallFlags::NO_AUTO_START,
            2000,
            gio::Cancellable::NONE,
            move |res| {
                let Some(me) = me.upgrade() else { return };
                // state 1 is ENABLED (ACTIVE since GNOME 46); an empty answer means the shell does not know it yet
                let state = res
                    .ok()
                    .and_then(|v| glib::VariantDict::new(Some(&v.child_value(0))).lookup_value("state", None))
                    .and_then(|s| s.get::<f64>());
                let (step, hint) = match (enabled, state) {
                    (_, Some(1.0)) => (Step::Done, ""),
                    (true, Some(s)) if s == 3.0 || s == 4.0 => (Step::Missing, "setup_clip_failed"),
                    (true, _) => (Step::NeedsLogout, ""),
                    (false, _) => (Step::Missing, "setup_clip_failed"),
                };
                {
                    let mut r = me.report.borrow_mut();
                    r.clipboard = step;
                    r.clipboard_hint = hint;
                }
                me.push();
            },
        );
    }

    fn push(&self) {
        let w = self.welcome.borrow().clone();
        if let Some(w) = w {
            w.set_report(&self.report.borrow());
        }
    }

    /// Open the welcome window (the setup page first when `setup_first`).
    pub fn show_welcome(self: &Rc<Self>, setup_first: bool) {
        if self.welcome.borrow().is_none() {
            let weak: Weak<Self> = Rc::downgrade(self);
            let w1 = weak.clone();
            let w2 = weak.clone();
            let w3 = weak.clone();
            let w4 = weak.clone();
            let w5 = weak.clone();
            let hooks = WelcomeHooks {
                autostart_on: Rc::new(autostart::is_enabled),
                set_autostart: Rc::new(move |on| {
                    if autostart::is_enabled() != on {
                        if let Err(e) = autostart::set_enabled(on) {
                            warn!("Setup: could not change start at login: {}", e);
                        }
                    }
                    if let Some(me) = w1.upgrade() {
                        me.report.borrow_mut().autostart = autostart::is_enabled();
                    }
                }),
                turn_on_clipboard: Rc::new(move || {
                    if let Some(me) = w2.upgrade() {
                        me.turn_on_extension();
                        me.refresh();
                    }
                }),
                create_shortcuts: Rc::new(move || {
                    if let Some(me) = w3.upgrade() {
                        let ok = me.create_shortcuts();
                        {
                            let mut r = me.report.borrow_mut();
                            r.shortcuts = if ok { Step::Done } else { Step::Missing };
                            r.shortcuts_custom = ok;
                        }
                        me.push();
                    }
                }),
                open_menew: Rc::new(move || {
                    if let Some(me) = w4.upgrade() {
                        let f = me.open_menew.borrow().clone();
                        if let Some(f) = f {
                            // after the welcome window is gone, so the palette gets the focus
                            glib::timeout_add_local_once(std::time::Duration::from_millis(150), move || f());
                        }
                    }
                }),
                closed: Rc::new(move || {
                    if let Some(me) = w5.upgrade() {
                        me.pending.set(false);
                        if let Err(e) = rules::mark_welcome_done_in(&me.data_dir) {
                            warn!("Setup: could not save that the welcome was shown: {}", e);
                        }
                        let me2 = Rc::clone(&me);
                        glib::idle_add_local_once(move || {
                            me2.welcome.borrow_mut().take();
                        });
                    }
                }),
            };
            *self.welcome.borrow_mut() = Some(WelcomeWindow::new(&self.app, current_language(), hooks));
        }
        let w = self.welcome.borrow().clone();
        if let Some(w) = w {
            w.set_report(&self.report.borrow());
            w.show(setup_first);
        }
        self.refresh();
    }
}
