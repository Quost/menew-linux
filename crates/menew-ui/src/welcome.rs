//! The welcome window, the port of `Onboarding.swift`: three pages shown once, on the first run, and again from
//! the palette ("Welcome and setup"). The Mac asks for one permission on its setup page; here the setup page
//! shows what Menew did on the desktop by itself (start at login, the GNOME extension, the shortcuts, the tray)
//! and offers a button where something is still missing. The work itself is the daemon's (`WelcomeHooks`).

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use menew_core::setup::{SetupReport, Step};
use menew_core::strings::tr;
use menew_core::Language;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// What the buttons and the switch of the window do.
pub struct WelcomeHooks {
    pub autostart_on: Rc<dyn Fn() -> bool>,
    pub set_autostart: Rc<dyn Fn(bool)>,
    /// Turn on the GNOME extension (the button of the clipboard row).
    pub turn_on_clipboard: Rc<dyn Fn()>,
    /// Create the GNOME custom shortcuts (the button of the shortcuts row).
    pub create_shortcuts: Rc<dyn Fn()>,
    /// "Open Menew" on the last page.
    pub open_menew: Rc<dyn Fn()>,
    /// The window closed, whatever the way: the welcome is done.
    pub closed: Rc<dyn Fn()>,
}

struct StatusRow {
    row: adw::ActionRow,
    icon: gtk4::Image,
    button: gtk4::Button,
}

impl StatusRow {
    fn new(title: &str, button_label: &str) -> Self {
        let row = adw::ActionRow::builder().title(title).subtitle_lines(3).build();
        let icon = gtk4::Image::from_icon_name("content-loading-symbolic");
        icon.set_valign(gtk4::Align::Center);
        row.add_prefix(&icon);
        let button = gtk4::Button::with_label(button_label);
        button.set_valign(gtk4::Align::Center);
        button.set_visible(false);
        row.add_suffix(&button);
        Self { row, icon, button }
    }

    fn set(&self, step: Step, text: &str, offer_button: bool) {
        let (icon, class) = match step {
            Step::Checking => ("content-loading-symbolic", "dim-label"),
            Step::Done | Step::NotNeeded => ("emblem-ok-symbolic", "success"),
            Step::NeedsLogout => ("system-log-out-symbolic", "accent"),
            Step::Missing => ("dialog-warning-symbolic", "warning"),
        };
        self.icon.set_icon_name(Some(icon));
        for c in ["dim-label", "success", "accent", "warning"] {
            self.icon.remove_css_class(c);
        }
        self.icon.add_css_class(class);
        self.row.set_subtitle(&glib_escape(text));
        self.button.set_visible(offer_button);
    }
}

/// Row subtitles are Pango markup.
fn glib_escape(s: &str) -> String {
    gtk4::glib::markup_escape_text(s).to_string()
}

fn label(text: &str, classes: &[&str]) -> gtk4::Label {
    let l = gtk4::Label::builder().label(text).wrap(true).justify(gtk4::Justification::Center).build();
    l.set_max_width_chars(46);
    for c in classes {
        l.add_css_class(c);
    }
    l
}

const APP_ICON_SVG: &[u8] = include_bytes!("../../../data/icons/hicolor/scalable/apps/menew.svg");

/// The Menew icon: from the icon theme (the .deb installs it), else the copy built into the binary (AppImage, builds).
fn app_icon(size: i32) -> gtk4::Image {
    let themed = gtk4::gdk::Display::default()
        .map(|d| gtk4::IconTheme::for_display(&d).has_icon("menew"))
        .unwrap_or(false);
    let img = if themed {
        gtk4::Image::from_icon_name("menew")
    } else {
        let stream = gtk4::gio::MemoryInputStream::from_bytes(&gtk4::glib::Bytes::from_static(APP_ICON_SVG));
        match gtk4::gdk_pixbuf::Pixbuf::from_stream_at_scale(&stream, size * 2, size * 2, true, gtk4::gio::Cancellable::NONE) {
            #[allow(deprecated)]
            Ok(p) => gtk4::Image::from_paintable(Some(&gtk4::gdk::Texture::for_pixbuf(&p))),
            Err(_) => gtk4::Image::from_icon_name("edit-paste-symbolic"),
        }
    };
    img.set_pixel_size(size);
    img
}

fn feature_row(icon: &str, title: &str, detail: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(title).subtitle(glib_escape(detail)).subtitle_lines(2).build();
    let img = gtk4::Image::from_icon_name(icon);
    img.set_pixel_size(24);
    img.add_css_class("accent");
    row.add_prefix(&img);
    row
}

fn key_row(keys: &str, title: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(title).build();
    let k = gtk4::Label::new(Some(keys));
    k.add_css_class("dim-label");
    k.add_css_class("monospace");
    row.add_suffix(&k);
    row
}

fn boxed_list() -> gtk4::ListBox {
    let list = gtk4::ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::None);
    list.add_css_class("boxed-list");
    list
}

fn page() -> gtk4::Box {
    let b = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    b.set_margin_top(12);
    b.set_margin_bottom(12);
    b.set_margin_start(28);
    b.set_margin_end(28);
    b.set_valign(gtk4::Align::Center);
    b
}

pub struct WelcomeWindow {
    pub window: adw::ApplicationWindow,
    stack: gtk4::Stack,
    step: Cell<usize>,
    back: gtk4::Button,
    skip: gtk4::Button,
    next: gtk4::Button,
    login: gtk4::Switch,
    clipboard: StatusRow,
    shortcuts: StatusRow,
    tray: StatusRow,
    logout_note: gtk4::Label,
    lang: Language,
    hooks: Rc<WelcomeHooks>,
    report: RefCell<SetupReport>,
}

const PAGES: [&str; 3] = ["hello", "setup", "done"];

impl WelcomeWindow {
    pub fn new(app: &adw::Application, lang: Language, hooks: WelcomeHooks) -> Rc<Self> {
        let t = |k: &str| tr(k, lang, &[]);
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Menew")
            .default_width(500)
            .default_height(640)
            .resizable(false)
            .build();

        // page 1: hello
        let hello = page();
        hello.append(&app_icon(96));
        hello.append(&label(&t("welcome_title"), &["title-1"]));
        hello.append(&label(&t("welcome_sub"), &["dim-label"]));
        let features = boxed_list();
        features.set_margin_top(12);
        features.append(&feature_row("system-search-symbolic", &t("welcome_f1_t"), &t("welcome_f1_d")));
        features.append(&feature_row("edit-paste-symbolic", &t("welcome_f2_t"), &t("welcome_f2_d")));
        features.append(&feature_row("camera-photo-symbolic", &t("welcome_f3_t"), &t("welcome_f3_d")));
        hello.append(&features);

        // page 2: what Menew set up
        let setup = page();
        setup.append(&label(&t("setup_title"), &["title-2"]));
        setup.append(&label(&t("setup_sub"), &["dim-label"]));
        let list = boxed_list();
        list.set_margin_top(12);
        let login_row = adw::ActionRow::builder().title(t("setup_login_t")).subtitle(glib_escape(&t("setup_login_d"))).subtitle_lines(2).build();
        let login_icon = gtk4::Image::from_icon_name("system-run-symbolic");
        login_icon.set_valign(gtk4::Align::Center);
        login_row.add_prefix(&login_icon);
        let login = gtk4::Switch::new();
        login.set_valign(gtk4::Align::Center);
        login.set_active((hooks.autostart_on)());
        login_row.add_suffix(&login);
        login_row.set_activatable_widget(Some(&login));
        list.append(&login_row);
        let clipboard = StatusRow::new(&t("setup_clip_t"), &t("setup_turn_on"));
        let shortcuts = StatusRow::new(&t("setup_keys_t"), &t("setup_keys_create"));
        let tray = StatusRow::new(&t("setup_tray_t"), "");
        for r in [&clipboard, &shortcuts, &tray] {
            r.set(Step::Checking, &t("setup_checking"), false);
            list.append(&r.row);
        }
        setup.append(&list);

        // page 3: all set
        let done = page();
        done.append(&app_icon(72));
        done.append(&label(&t("welcome_done_title"), &["title-1"]));
        done.append(&label(&t("welcome_done_sub"), &["dim-label"]));
        let keys = boxed_list();
        keys.set_margin_top(6);
        keys.append(&key_row("Ctrl+Alt+Space", &t("welcome_key_open")));
        keys.append(&key_row("Ctrl+Alt+V", &t("setup_clip_t")));
        keys.append(&key_row("Ctrl+Alt+Shift+S", &t("welcome_key_capture")));
        done.append(&keys);
        let logout_note = label(&t("welcome_logout_note"), &["warning"]);
        logout_note.set_visible(false);
        done.append(&logout_note);
        done.append(&label(&t("welcome_tray_hint"), &["dim-label", "caption"]));

        let stack = gtk4::Stack::new();
        stack.set_transition_type(gtk4::StackTransitionType::SlideLeftRight);
        stack.set_vexpand(true);
        stack.add_named(&hello, Some(PAGES[0]));
        stack.add_named(&setup, Some(PAGES[1]));
        stack.add_named(&done, Some(PAGES[2]));

        // footer: Back, Skip ... Continue / Open Menew
        let footer = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        footer.set_margin_start(20);
        footer.set_margin_end(20);
        footer.set_margin_bottom(20);
        let back = gtk4::Button::with_label(&t("btn_back"));
        let skip = gtk4::Button::with_label(&t("btn_skip"));
        skip.add_css_class("flat");
        let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        let next = gtk4::Button::with_label(&t("btn_continue"));
        next.add_css_class("suggested-action");
        next.add_css_class("pill");
        footer.append(&back);
        footer.append(&skip);
        footer.append(&spacer);
        footer.append(&next);

        let header = adw::HeaderBar::new();
        header.add_css_class("flat");
        header.set_title_widget(Some(&gtk4::Label::new(None)));
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        content.append(&header);
        content.append(&stack);
        content.append(&footer);
        window.set_content(Some(&content));

        let me = Rc::new(Self {
            window,
            stack,
            step: Cell::new(0),
            back,
            skip,
            next,
            login,
            clipboard,
            shortcuts,
            tray,
            logout_note,
            lang,
            hooks: Rc::new(hooks),
            report: RefCell::new(SetupReport::default()),
        });

        let m = Rc::downgrade(&me);
        me.back.connect_clicked(move |_| {
            if let Some(m) = m.upgrade() {
                m.go(m.step.get().saturating_sub(1));
            }
        });
        let m = Rc::downgrade(&me);
        me.skip.connect_clicked(move |_| {
            if let Some(m) = m.upgrade() {
                m.window.close();
            }
        });
        let m = Rc::downgrade(&me);
        me.next.connect_clicked(move |_| {
            if let Some(m) = m.upgrade() {
                if m.step.get() + 1 < PAGES.len() {
                    m.go(m.step.get() + 1);
                } else {
                    m.window.close();
                    (m.hooks.open_menew)();
                }
            }
        });
        let h = Rc::clone(&me.hooks);
        me.login.connect_active_notify(move |s| (h.set_autostart)(s.is_active()));
        let h = Rc::clone(&me.hooks);
        me.clipboard.button.connect_clicked(move |b| {
            b.set_sensitive(false);
            (h.turn_on_clipboard)();
        });
        let h = Rc::clone(&me.hooks);
        me.shortcuts.button.connect_clicked(move |b| {
            b.set_sensitive(false);
            (h.create_shortcuts)();
        });
        let h = Rc::clone(&me.hooks);
        me.window.connect_close_request(move |_| {
            (h.closed)();
            gtk4::glib::Propagation::Proceed
        });
        me.go(0);
        me
    }

    fn go(&self, step: usize) {
        let step = step.min(PAGES.len() - 1);
        self.step.set(step);
        self.stack.set_visible_child_name(PAGES[step]);
        self.back.set_visible(step > 0);
        self.skip.set_visible(step == 0);
        let last = step + 1 == PAGES.len();
        self.next.set_label(&tr(if last { "welcome_key_open" } else { "btn_continue" }, self.lang, &[]));
    }

    /// Show the first page (or the setup page, `setup_first`), in front.
    pub fn show(&self, setup_first: bool) {
        self.login.set_active((self.hooks.autostart_on)());
        self.go(if setup_first { 1 } else { 0 });
        self.window.present();
    }

    pub fn report(&self) -> SetupReport {
        self.report.borrow().clone()
    }

    /// Show what the daemon found or did.
    pub fn set_report(&self, r: &SetupReport) {
        *self.report.borrow_mut() = r.clone();
        let t = |k: &str| tr(k, self.lang, &[]);
        if self.login.is_active() != r.autostart {
            self.login.set_active(r.autostart);
        }
        let clip_text = match (r.clipboard, r.gnome) {
            (Step::Checking, _) => t("setup_checking"),
            (Step::Done, true) => t("setup_clip_gnome_done"),
            (Step::NeedsLogout, _) => t("setup_clip_logout"),
            (Step::Missing, _) => t(if r.clipboard_hint.is_empty() { "setup_clip_failed" } else { r.clipboard_hint }),
            _ => t("setup_clip_ready"),
        };
        self.clipboard.set(r.clipboard, &clip_text, r.gnome && r.clipboard == Step::Missing && r.clipboard_hint == "setup_clip_failed");
        let keys_text = match r.shortcuts {
            Step::Checking => t("setup_checking"),
            Step::Done | Step::NotNeeded if r.shortcuts_custom => t("setup_keys_custom"),
            Step::Done | Step::NotNeeded | Step::NeedsLogout => t("setup_keys_portal"),
            Step::Missing => t("setup_keys_missing"),
        };
        self.shortcuts.set(r.shortcuts, &keys_text, r.gnome && r.shortcuts == Step::Missing);
        let tray_text = match r.tray {
            Step::Checking => t("setup_checking"),
            Step::Missing => t("setup_tray_missing"),
            _ => t("setup_tray_done"),
        };
        self.tray.set(r.tray, &tray_text, false);
        self.clipboard.button.set_sensitive(true);
        self.shortcuts.button.set_sensitive(true);
        self.logout_note.set_visible(r.clipboard == Step::NeedsLogout);
    }
}
