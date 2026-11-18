//! Automatic updates in the daemon (the counterpart of `Updater.swift`): the scheduled check, the discreet
//! reminder in the tray, the window with the notes (Install and Relaunch / Skip This Version / Later) and
//! the restart after installing. The work that is not GTK (versions, feed, signature, install) is in
//! `menew_core::updates`; the check and the install run on a blocking thread of the Tokio runtime.

use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use adw::prelude::MessageDialogExt;
use libadwaita as adw;
use menew_core::strings::tr;
use menew_core::updates::{self, Feed, Found, InstallError, InstallMethod, UpdateError, UpdateState, Version};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use tracing::{info, warn};

pub struct Updater {
    app: adw::Application,
    rt: Arc<tokio::runtime::Runtime>,
    feed: Feed,
    /// False in `--snapshot` and under `MENEW_TEST=1` without a test feed: nothing is ever requested.
    allowed: bool,
    auto: Cell<bool>,
    state: RefCell<UpdateState>,
    /// Found by a check, not installed, not skipped: the tray shows the dot and the menu item.
    pending: RefCell<Option<Found>>,
    busy: Cell<bool>,
    dialog: RefCell<Option<adw::MessageDialog>>,
    on_change: RefCell<Option<Rc<dyn Fn()>>>,
}

impl Updater {
    pub fn new(app: &adw::Application, rt: Arc<tokio::runtime::Runtime>, auto: bool) -> Rc<Self> {
        let feed = Feed::from_env();
        let test_mode = std::env::var("MENEW_TEST").map(|v| v == "1").unwrap_or(false);
        let allowed = updates::checks_allowed(false, test_mode, feed.overridden);
        if !allowed {
            info!("Updates: checks are off in test mode (no MENEW_UPDATE_FEED)");
        }
        Rc::new(Self {
            app: app.clone(),
            rt,
            feed,
            allowed,
            auto: Cell::new(auto),
            state: RefCell::new(UpdateState::load()),
            pending: RefCell::new(None),
            busy: Cell::new(false),
            dialog: RefCell::new(None),
            on_change: RefCell::new(None),
        })
    }

    /// Called whenever what the tray shows (the dot, the checkmark) changes.
    pub fn set_on_change(&self, f: Rc<dyn Fn()>) {
        *self.on_change.borrow_mut() = Some(f);
    }

    fn changed(&self) {
        let f = self.on_change.borrow().clone();
        if let Some(f) = f {
            f();
        }
    }

    pub fn pending_version(&self) -> Option<String> {
        self.pending.borrow().as_ref().map(|f| f.version.to_string())
    }

    pub fn auto_enabled(&self) -> bool {
        self.auto.get()
    }

    pub fn toggle_auto(&self) {
        let want = !self.auto.get();
        self.auto.set(want);
        let mut cfg = menew_core::Config::load();
        cfg.auto_update_check = want;
        if let Err(e) = cfg.save_to(&menew_core::Config::default_path()) {
            warn!("Updates: could not save the setting: {}", e);
        }
        info!("Updates: automatic check set to {}", want);
        self.changed();
    }

    /// The scheduled checks: one a few minutes after the start, then every 24 hours.
    pub fn start_schedule(self: &Rc<Self>) {
        if !self.allowed {
            return;
        }
        // the delay can be shortened only together with the test feed
        let delay = Some(self.feed.overridden)
            .filter(|o| *o)
            .and_then(|_| std::env::var("MENEW_UPDATE_STARTUP_DELAY").ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(updates::STARTUP_CHECK_DELAY_SECS);
        info!("Updates: first automatic check in {}s (enabled: {})", delay, self.auto.get());
        let me = Rc::clone(self);
        glib::timeout_add_seconds_local_once(delay, move || me.auto_check());
        let me = Rc::clone(self);
        glib::timeout_add_seconds_local(updates::AUTO_CHECK_INTERVAL_SECS, move || {
            me.auto_check();
            glib::ControlFlow::Continue
        });
    }

    fn auto_check(self: &Rc<Self>) {
        if self.auto.get() && !self.busy.get() {
            self.run_check(false);
        } else {
            info!("Updates: automatic check skipped (enabled: {}, busy: {})", self.auto.get(), self.busy.get());
        }
    }

    /// "Check for Updates…" (tray, palette): shows the update already found, or looks for one.
    pub fn check_manual(self: &Rc<Self>) {
        if !self.allowed {
            self.notify(&tr("upd_disabled", menew_core::current_language(), &[]), "");
            return;
        }
        if let Some(found) = self.pending.borrow().clone() {
            return self.show_window(found);
        }
        if !self.busy.get() {
            self.run_check(true);
        }
    }

    /// Click on the "Update available" item of the tray.
    pub fn show_pending(self: &Rc<Self>) {
        match self.pending.borrow().clone() {
            Some(found) => self.show_window(found),
            None => self.check_manual(),
        }
    }

    /// Test hook (`__updates-install`, only with `MENEW_TEST=1`): check and install without the window.
    pub fn test_check_and_install(self: &Rc<Self>) {
        let feed = self.feed.clone();
        let handle = self.rt.spawn_blocking(move || updates::check(&feed, &Version::current()));
        let me = Rc::clone(self);
        glib::spawn_future_local(async move {
            match handle.await.unwrap_or(Err(UpdateError::Network)) {
                Ok(Some(found)) => me.install(found),
                other => warn!("Updates: test install found nothing to install: {:?}", other.map(|o| o.map(|f| f.version.to_string()))),
            }
        });
    }

    fn run_check(self: &Rc<Self>, manual: bool) {
        self.busy.set(true);
        let feed = self.feed.clone();
        let handle = self.rt.spawn_blocking(move || updates::check(&feed, &Version::current()));
        let me = Rc::clone(self);
        glib::spawn_future_local(async move {
            let result = handle.await.unwrap_or(Err(UpdateError::Network));
            me.busy.set(false);
            me.on_checked(result, manual);
        });
    }

    fn on_checked(self: &Rc<Self>, result: Result<Option<Found>, UpdateError>, manual: bool) {
        let lang = menew_core::current_language();
        match result {
            Ok(Some(found)) => {
                info!("Updates: version {} found (manual={})", found.version, manual);
                if manual {
                    *self.pending.borrow_mut() = Some(found.clone());
                    self.changed();
                    self.show_window(found);
                } else if updates::announce_automatically(&self.state.borrow(), &found.version) {
                    *self.pending.borrow_mut() = Some(found);
                    self.changed();
                } else {
                    info!("Updates: {} was skipped by the user, staying quiet", found.version);
                }
            }
            Ok(None) => {
                info!("Updates: up to date");
                *self.pending.borrow_mut() = None;
                self.changed();
                if manual {
                    self.notify(&tr("upd_uptodate", lang, &[&Version::current().to_string()]), "");
                }
            }
            // an offline or failing feed is a log line, never an error in the user's face
            Err(e) => {
                warn!("Updates: check failed: {}", e);
                if manual {
                    self.notify(&tr("upd_check_failed", lang, &[]), "");
                }
            }
        }
    }

    fn show_window(self: &Rc<Self>, found: Found) {
        if let Some(open) = self.dialog.borrow().as_ref() {
            open.present();
            return;
        }
        let lang = menew_core::current_language();
        let version = found.version.to_string();
        let dialog = adw::MessageDialog::new(
            None::<&gtk4::Window>,
            Some(&tr("upd_title", lang, &[&version])),
            Some(&tr("upd_body", lang, &[&Version::current().to_string()])),
        );
        let notes = updates::plain_notes(&found.manifest.notes);
        if !notes.is_empty() {
            let label = gtk4::Label::new(Some(&notes));
            label.set_wrap(true);
            label.set_xalign(0.0);
            label.set_selectable(true);
            label.set_margin_top(6);
            label.set_margin_bottom(6);
            let scroll = gtk4::ScrolledWindow::new();
            scroll.set_child(Some(&label));
            scroll.set_min_content_width(420);
            scroll.set_min_content_height(160);
            scroll.set_max_content_height(320);
            scroll.set_propagate_natural_height(true);
            scroll.set_has_frame(true);
            dialog.set_extra_child(Some(&scroll));
        }
        dialog.add_response("later", &tr("upd_later", lang, &[]));
        dialog.add_response("skip", &tr("upd_skip", lang, &[]));
        dialog.add_response("install", &tr("upd_install", lang, &[]));
        dialog.set_response_appearance("install", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("install"));
        dialog.set_close_response("later");
        let me = Rc::clone(self);
        dialog.connect_response(None, move |_, response| {
            *me.dialog.borrow_mut() = None;
            match response {
                "install" => me.install(found.clone()),
                "skip" => {
                    me.state.borrow_mut().skipped = Some(found.version.to_string());
                    if let Err(e) = me.state.borrow().save() {
                        warn!("Updates: could not save the skipped version: {}", e);
                    }
                    *me.pending.borrow_mut() = None;
                    me.changed();
                    info!("Updates: skipping {}", found.version);
                }
                // "Later": the dot stays, the next scheduled check reminds again
                _ => {}
            }
        });
        *self.dialog.borrow_mut() = Some(dialog.clone());
        dialog.present();
    }

    fn install(self: &Rc<Self>, found: Found) {
        let lang = menew_core::current_language();
        let version = found.version.to_string();
        let method = updates::detect_install(&self.feed);
        if method == InstallMethod::Manual {
            // built from source or from an unknown place: tell, with the link of the release
            self.open_uri(&found.page_url());
            self.notify(&tr("upd_manual", lang, &[&version]), "");
            return;
        }
        if self.busy.get() {
            return;
        }
        self.busy.set(true);
        self.notify(&tr("upd_downloading", lang, &[&version]), "");
        let feed = self.feed.clone();
        let (f2, m2) = (found.clone(), method.clone());
        let handle = self.rt.spawn_blocking(move || updates::install(&f2, &m2, &feed, &|stage| info!("Updates: {}", stage)));
        let me = Rc::clone(self);
        glib::spawn_future_local(async move {
            let result = handle.await.unwrap_or_else(|e| Err(InstallError::Failed(e.to_string())));
            me.busy.set(false);
            me.on_installed(result, &found, &method);
        });
    }

    fn on_installed(self: &Rc<Self>, result: Result<(), InstallError>, found: &Found, method: &InstallMethod) {
        let lang = menew_core::current_language();
        match result {
            Ok(()) => {
                info!("Updates: {} installed, restarting", found.version);
                if let Some(exe) = updates::restart_target(method) {
                    let mut cmd = updates::restart_command(&exe, std::process::id());
                    if let Err(e) = cmd.spawn() {
                        warn!("Updates: could not start the new version: {}", e);
                    }
                }
                self.app.quit();
            }
            Err(InstallError::NotWritable) => {
                warn!("Updates: the folder of the AppImage is not writable");
                self.open_uri(&found.page_url());
                self.notify(&tr("upd_err_dir", lang, &[]), "");
            }
            Err(InstallError::Declined) | Err(InstallError::Failed(_)) | Err(InstallError::Manual) => {
                warn!("Updates: install did not finish");
                self.open_uri(&found.page_url());
                self.notify(&tr("upd_err_install", lang, &[]), "");
            }
            Err(InstallError::Verify(UpdateError::Network)) => {
                warn!("Updates: download failed");
                self.notify(&tr("upd_err_download", lang, &[]), "");
            }
            Err(InstallError::Verify(e)) => {
                // nothing was installed and the downloaded file is gone
                warn!("Updates: verification failed: {}", e);
                self.notify(&tr("upd_err_verify", lang, &[]), "");
            }
        }
    }

    fn notify(&self, title: &str, body: &str) {
        let n = gio::Notification::new(title);
        if !body.is_empty() {
            n.set_body(Some(body));
        }
        self.app.send_notification(Some("menew-update"), &n);
        info!("Notification sent: {} {}", title, body);
    }

    fn open_uri(&self, uri: &str) {
        if let Err(e) = gio::AppInfo::launch_default_for_uri(uri, None::<&gio::AppLaunchContext>) {
            warn!("Updates: could not open {}: {}", uri, e);
        }
    }
}
