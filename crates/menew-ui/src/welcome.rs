//! The welcome tour (port of `Onboarding.swift`): what Menew does, the shortcut, what the clipboard
//! needs on this desktop, the features to keep, and the update check (off unless turned on here).

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use menew_core::config::Config;
use menew_core::strings::tr;
use menew_core::sys::Desktop;
use menew_core::current_language;
use std::rc::Rc;

fn page(icon: &str, title: &str, body: &str) -> adw::StatusPage {
    adw::StatusPage::builder().icon_name(icon).title(title).description(body).vexpand(true).build()
}

/// Show the tour. `done(config)` runs when it is closed with the choices saved.
pub fn show(app: &adw::Application, done: Rc<dyn Fn(&Config)>) {
    let lang = current_language();
    let cfg = Config::load();
    let carousel = adw::Carousel::builder().hexpand(true).vexpand(true).allow_scroll_wheel(true).build();
    carousel.append(&page("menew", &tr("wel_title", lang, &[]), &tr("wel_body", lang, &[])));
    carousel.append(&page("input-keyboard-symbolic", &tr("wel_shortcut", lang, &[]), &tr("wel_shortcut_body", lang, &[])));
    let clip_body = match Desktop::detect() {
        Desktop::Gnome => tr("wel_clip_gnome", lang, &[]),
        Desktop::Kde => tr("wel_clip_kde", lang, &[]),
        Desktop::Other => tr("wel_clip_other", lang, &[]),
    };
    carousel.append(&page("edit-paste-symbolic", &tr("wel_clip", lang, &[]), &clip_body));

    // features and updates, with switches
    let choices = adw::PreferencesGroup::new();
    let sw = |title: &str, sub: &str, on: bool| {
        let r = adw::SwitchRow::builder().title(title).subtitle(sub).active(on).build();
        choices.add(&r);
        r
    };
    let clip = sw(&tr("set_mod_clipboard", lang, &[]), &tr("set_mod_clipboard_sub", lang, &[]), cfg.module_clipboard);
    let actions = sw(&tr("set_mod_actions", lang, &[]), &tr("set_mod_actions_sub", lang, &[]), cfg.module_actions);
    let notes = sw(&tr("set_mod_notes", lang, &[]), &tr("set_mod_notes_sub", lang, &[]), cfg.module_notes);
    let upd = sw(&tr("set_updates", lang, &[]), &tr("set_updates_sub", lang, &[]), cfg.update_check);
    let login = sw(&tr("tray_start_at_login_r1", lang, &[]), "", menew_core::autostart::is_enabled());
    let last = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(18).margin_start(24).margin_end(24).valign(gtk4::Align::Center).build();
    last.append(&gtk4::Label::builder().label(tr("wel_choose", lang, &[])).css_classes(["title-2"]).wrap(true).build());
    last.append(&gtk4::Label::builder().label(tr("wel_privacy", lang, &[])).wrap(true).justify(gtk4::Justification::Center).css_classes(["dim-label"]).build());
    last.append(&adw::Clamp::builder().maximum_size(480).child(&choices).build());
    let start = gtk4::Button::builder().label(tr("wel_start", lang, &[])).halign(gtk4::Align::Center).css_classes(["pill", "suggested-action"]).build();
    last.append(&start);
    carousel.append(&gtk4::ScrolledWindow::builder().child(&last).hscrollbar_policy(gtk4::PolicyType::Never).build());

    let dots = adw::CarouselIndicatorDots::builder().carousel(&carousel).build();
    let next = gtk4::Button::builder().label(tr("wel_next", lang, &[])).halign(gtk4::Align::End).css_classes(["suggested-action"]).build();
    let c2 = carousel.clone();
    next.connect_clicked(move |_| {
        let i = c2.position().round() as u32;
        if i + 1 < c2.n_pages() {
            c2.scroll_to(&c2.nth_page(i + 1), true);
        }
    });
    let n2 = next.clone();
    carousel.connect_position_notify(move |c| n2.set_visible(c.position().round() as u32 + 1 < c.n_pages()));
    let bottom = gtk4::CenterBox::builder().margin_top(6).margin_bottom(12).margin_start(12).margin_end(12).center_widget(&dots).end_widget(&next).build();
    let body = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).build();
    body.append(&carousel);
    body.append(&bottom);
    let tv = adw::ToolbarView::new();
    tv.add_top_bar(&adw::HeaderBar::builder().show_title(false).build());
    tv.set_content(Some(&body));
    let win = adw::ApplicationWindow::builder().application(app).title(tr("wel_title", lang, &[])).default_width(620).default_height(560).content(&tv).build();

    let w = win.clone();
    start.connect_clicked(move |_| {
        let mut cfg = Config::load();
        cfg.module_clipboard = clip.is_active();
        cfg.module_actions = actions.is_active();
        cfg.module_notes = notes.is_active();
        cfg.update_check = upd.is_active();
        cfg.welcome_done = true;
        let _ = menew_core::autostart::set_enabled(login.is_active());
        let _ = cfg.save_to(&Config::default_path());
        done(&cfg);
        w.close();
    });
    win.present();
}
