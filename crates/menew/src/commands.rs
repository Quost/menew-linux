//! The daemon side of [`ItemAction::Command`]: builds the palette items of the other features for
//! the text typed and runs their commands. Ids:
//! - `qa.*` quick actions (`menew_core::quick_actions`)
//! - `win.<position>` window snapping (`menew_desktop::snap`)
//! - `cap.*` screenshots and screen text (`crate::capture`)
//! - `note.*` notes (`menew_ui::notes`)
//! - `open.*` the Menew window sections
//!
//! The same ids are used by the global shortcuts and by `menew run <command>`.

use gtk4::gio;
use gtk4::glib;
use menew_core::palette::PaletteItem;
use menew_core::quick_actions::{self as qa, KeepAwake, ToolState};
use menew_core::strings::tr;
use menew_core::sys::Desktop;
use menew_core::{current_language, Language};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use tracing::{info, warn};

/// Extra item providers registered by the other features (notes, capture, monitor, window).
pub type Provider = Rc<dyn Fn(&str, Language) -> Vec<PaletteItem>>;
/// Runner for a command prefix (`note.`, `cap.`, `open.`): returns true when it handled the id.
pub type Runner = Rc<dyn Fn(&str) -> bool>;

pub struct Commands {
    palette: Rc<menew_ui::PaletteWindow>,
    rt: Arc<tokio::runtime::Runtime>,
    desktop: Desktop,
    tools: RefCell<ToolState>,
    pub awake: RefCell<KeepAwake>,
    providers: RefCell<Vec<Provider>>,
    runners: RefCell<Vec<Runner>>,
    /// Called after something the tray or the popover shows changed (keep awake, mute).
    on_change: RefCell<Option<Rc<dyn Fn()>>>,
}

fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

impl Commands {
    pub fn new(palette: Rc<menew_ui::PaletteWindow>, rt: Arc<tokio::runtime::Runtime>) -> Rc<Self> {
        let me = Rc::new(Self {
            palette,
            rt,
            desktop: Desktop::detect(),
            tools: RefCell::new(ToolState::default()),
            awake: RefCell::new(KeepAwake::default()),
            providers: RefCell::new(vec![]),
            runners: RefCell::new(vec![]),
            on_change: RefCell::new(None),
        });
        let items_me = Rc::clone(&me);
        me.palette.set_extra_items(Rc::new(move |raw| items_me.items(raw)));
        let run_me = Rc::clone(&me);
        me.palette.set_command_runner(Rc::new(move |c| run_me.run(c)));
        let show_me = Rc::clone(&me);
        me.palette.set_on_show(Rc::new(move || show_me.refresh_tools()));
        me.refresh_tools();
        me
    }

    #[allow(dead_code)] // for the tray popover and the Menew window (next PR)
    pub fn add_provider(&self, p: Provider) {
        self.providers.borrow_mut().push(p);
    }

    pub fn add_runner(&self, r: Runner) {
        self.runners.borrow_mut().push(r);
    }

    #[allow(dead_code)] // for the tray popover and the Menew window (next PR)
    pub fn set_on_change(&self, f: Rc<dyn Fn()>) {
        *self.on_change.borrow_mut() = Some(f);
    }

    fn changed(&self) {
        let f = self.on_change.borrow().clone();
        if let Some(f) = f {
            f();
        }
    }

    #[allow(dead_code)] // for the tray popover and the Menew window (next PR)
    pub fn desktop(&self) -> Desktop {
        self.desktop
    }

    pub fn tools(&self) -> ToolState {
        let mut t = self.tools.borrow().clone();
        t.awake = self.awake.borrow_mut().is_on();
        t
    }

    #[allow(dead_code)] // for the tray popover and the Menew window (next PR)
    pub fn awake_until(&self) -> Option<f64> {
        self.awake.borrow().until
    }

    /// Read the quick action state off the GTK thread, then rebuild the open palette.
    pub fn refresh_tools(self: &Rc<Self>) {
        let me = Rc::clone(self);
        let desktop = self.desktop;
        glib::spawn_future_local(async move {
            if let Ok(st) = gio::spawn_blocking(move || ToolState::read(desktop)).await {
                *me.tools.borrow_mut() = st;
                me.palette.refresh_now();
                me.changed();
            }
        });
    }

    /// The items for the text typed.
    pub fn items(&self, raw: &str) -> Vec<PaletteItem> {
        let lang = current_language();
        let mut out = qa::items(&self.tools(), lang);
        out.extend(menew_core::windows::items(lang));
        let q = raw.trim();
        if !q.is_empty() {
            let apps = self.palette.apps();
            let never = menew_core::Config::load().never_quit;
            out.extend(qa::typed_items(q, &|| qa::running_apps(&apps, &never), &qa::listening_ports, lang));
        }
        for p in self.providers.borrow().iter() {
            out.extend(p(raw, lang));
        }
        out
    }

    fn toast(&self, key: &str, args: &[&str]) {
        self.palette.notify("menew-action", &tr(key, current_language(), args), None);
    }

    /// Run a command id. Unknown ids are logged and ignored.
    pub fn run(self: &Rc<Self>, cmd: &str) {
        info!("Command: {}", cmd);
        let lang = current_language();
        let (name, arg) = match cmd.split_once(':') {
            Some((n, a)) => (n, Some(a)),
            None => (cmd, None),
        };
        match name {
            "qa.mic" => match qa::toggle_mute(true) {
                Some(m) => {
                    self.toast(if m { "qa_mic_muted" } else { "qa_mic_live" }, &[]);
                    self.tools.borrow_mut().mic_muted = Some(m);
                    self.changed();
                }
                None => self.toast("qa_cant_mute", &[]),
            },
            "qa.sound" => match qa::toggle_mute(false) {
                Some(m) => {
                    self.toast(if m { "qa_sound_muted" } else { "qa_sound_live" }, &[]);
                    self.tools.borrow_mut().sound_muted = Some(m);
                    self.changed();
                }
                None => self.toast("qa_cant_mute", &[]),
            },
            "qa.dark" => {
                let dark = self.tools.borrow().dark != Some(true);
                if qa::set_dark(self.desktop, dark) {
                    self.tools.borrow_mut().dark = Some(dark);
                    self.changed();
                } else {
                    self.toast("qa_failed", &[]);
                }
            }
            "qa.out" | "qa.in" => {
                let input = name == "qa.in";
                let dev = arg.unwrap_or_default();
                if qa::set_default_device(input, dev) {
                    let st = self.tools.borrow();
                    let list = if input { &st.inputs } else { &st.outputs };
                    let label = list.iter().find(|d| d.name == dev).map(|d| d.description.clone()).unwrap_or_else(|| dev.to_string());
                    drop(st);
                    self.toast("qa_switched", &[&label]);
                    self.refresh_tools();
                } else {
                    self.toast("qa_failed", &[]);
                }
            }
            "qa.awake" => {
                let minutes = arg.and_then(|a| a.parse::<u32>().ok());
                let on = self.awake.borrow_mut().is_on();
                if on && minutes.is_none() {
                    self.awake.borrow_mut().stop();
                    self.toast("qa_awake_stopped", &[]);
                } else {
                    match self.awake.borrow_mut().start(minutes, now(), &tr("qa_awake", lang, &[])) {
                        Ok(()) => self.toast("qa_awake_started", &[]),
                        Err(qa::AwakeError::Missing) => self.toast("qa_awake_missing", &[]),
                        Err(qa::AwakeError::Refused) => self.toast("qa_awake_refused", &[]),
                    }
                }
                self.changed();
            }
            "qa.lock" => {
                if !qa::lock_screen() {
                    self.toast("qa_failed", &[]);
                }
            }
            "qa.trash" => {
                let me = Rc::clone(self);
                glib::spawn_future_local(async move {
                    let n = gio::spawn_blocking(qa::trash_count).await.ok().flatten();
                    if n == Some(0) {
                        me.toast("qa_trash_already", &[]);
                        return;
                    }
                    let me2 = Rc::clone(&me);
                    menew_ui::dialogs::confirm(
                        None,
                        &tr("qa_trash_ask", lang, &[]),
                        &tr("qa_trash_body", lang, &[]),
                        &tr("qa_trash_empty_btn", lang, &[]),
                        &tr("cancel", lang, &[]),
                        true,
                        move || {
                            let me3 = Rc::clone(&me2);
                            glib::spawn_future_local(async move {
                                let ok = gio::spawn_blocking(qa::empty_trash).await.unwrap_or(false);
                                me3.toast(if ok { "qa_trash_done" } else { "qa_failed" }, &[]);
                            });
                        },
                    );
                });
            }
            "qa.color" => {
                let me = Rc::clone(self);
                let job = self.rt.spawn(menew_desktop::pick_color());
                glib::spawn_future_local(async move {
                    match job.await {
                        Ok(Ok(hex)) => me.palette.copied(&hex),
                        Ok(Err(e)) => {
                            warn!("Color picker: {}", e);
                            me.toast("qa_color_fail", &[]);
                        }
                        Err(_) => {}
                    }
                });
            }
            "qa.plain" => self.palette.plain_text_clipboard(),
            "qa.eject" => {
                let me = Rc::clone(self);
                glib::spawn_future_local(async move {
                    let n = gio::spawn_blocking(qa::eject_all).await.unwrap_or(0);
                    if n > 0 {
                        me.toast("qa_ejected", &[&n.to_string()]);
                    } else {
                        me.toast("qa_eject_fail", &[]);
                    }
                    me.refresh_tools();
                });
            }
            "qa.bright" => {
                let p = arg.and_then(|a| a.parse::<u32>().ok()).unwrap_or(50);
                let me = Rc::clone(self);
                glib::spawn_future_local(async move {
                    let ok = gio::spawn_blocking(move || qa::set_brightness(p)).await.unwrap_or(false);
                    if !ok {
                        me.toast("qa_brightness_fail", &[]);
                    }
                });
            }
            "qa.quit" | "qa.force" => {
                let pids: Vec<u32> = arg.unwrap_or_default().split(',').filter_map(|p| p.parse().ok()).collect();
                if !qa::quit_pids(&pids, name == "qa.force") {
                    self.toast("qa_failed", &[]);
                }
            }
            "qa.quitall" => {
                let apps = self.palette.apps();
                let never = menew_core::Config::load().never_quit;
                let running = qa::running_apps(&apps, &never);
                let n = running.iter().filter(|a| qa::quit_pids(&a.pids, false)).count();
                self.toast("qa_quit_done", &[&n.to_string()]);
            }
            "qa.port" => {
                let pid = arg.and_then(|a| a.parse::<u32>().ok()).unwrap_or(0);
                if menew_core::sys::signal(pid, 15) {
                    self.toast("qa_port_stopped", &[]);
                } else {
                    self.toast("qa_failed", &[]);
                }
            }
            _ if name.starts_with("win.") => {
                let Some(snap) = menew_desktop::Snap::from_id(&name[4..]) else { return };
                // the palette is closing: give the compositor a moment to focus the window again
                let me = Rc::clone(self);
                let rt = Arc::clone(&self.rt);
                glib::timeout_add_local_once(std::time::Duration::from_millis(250), move || {
                    let job = rt.spawn(menew_desktop::snap(snap));
                    glib::spawn_future_local(async move {
                        match job.await {
                            Ok(Ok(())) => {}
                            Ok(Err(menew_desktop::SnapError::Unsupported)) => me.toast("win_unsupported", &[]),
                            Ok(Err(menew_desktop::SnapError::Failed(e))) => {
                                warn!("Snap failed: {}", e);
                                me.toast("win_failed", &[]);
                            }
                            Err(_) => {}
                        }
                    });
                });
            }
            _ => {
                let runners = self.runners.borrow().clone();
                if !runners.iter().any(|r| r(cmd)) {
                    warn!("Unknown command {}", cmd);
                }
            }
        }
    }

    /// Once a second: keep awake for a time ends by itself.
    pub fn tick(&self) {
        if self.awake.borrow_mut().tick(now()) {
            self.toast("qa_awake_stopped", &[]);
            self.changed();
        }
    }
}
