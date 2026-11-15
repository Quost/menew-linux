//! The Settings section (port of `SettingsWindow.swift`): general (language, start at login,
//! turning features off completely, updates, the welcome tour), clipboard, world clocks, focus,
//! quick actions and the global shortcuts. Every change is saved to `config.json` right away.

use crate::window;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use menew_core::config::Config;
use menew_core::strings::tr;
use menew_core::{current_language, time_zones, PasteMode};
use std::cell::RefCell;
use std::rc::Rc;

pub struct SettingsHooks {
    /// Called after the config changed and was saved (the daemon applies it).
    pub changed: Rc<dyn Fn(&Config)>,
    /// The language changed: offer to restart.
    pub language: Rc<dyn Fn()>,
    pub check_updates: Rc<dyn Fn()>,
    pub welcome: Rc<dyn Fn()>,
    /// The global shortcuts: (description, trigger, command for a custom system shortcut).
    pub shortcuts: Vec<(String, String, String)>,
    /// Whether the Menew GNOME extension answers (`None` on other desktops).
    pub gnome_extension: Option<bool>,
}

type Refill = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

pub struct SettingsView {
    pub root: gtk4::Widget,
}

const LANGS: [(&str, &str); 6] = [("system", ""), ("en", "English"), ("pt", "Português"), ("de", "Deutsch"), ("es", "Español"), ("zh", "中文")];

fn save(cfg: &Rc<RefCell<Config>>, hooks: &Rc<SettingsHooks>) {
    let c = cfg.borrow().clone();
    if let Err(e) = c.save_to(&Config::default_path()) {
        tracing::warn!("Settings: could not save config.json: {}", e);
    }
    (hooks.changed)(&c);
}

fn switch(group: &adw::PreferencesGroup, title: &str, sub: Option<&str>, on: bool, f: impl Fn(bool) + 'static) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder().title(title).active(on).build();
    if let Some(s) = sub {
        row.set_subtitle(s);
    }
    row.connect_active_notify(move |r| f(r.is_active()));
    group.add(&row);
    row
}

impl SettingsView {
    pub fn new(hooks: SettingsHooks) -> Rc<Self> {
        let lang = current_language();
        let hooks = Rc::new(hooks);
        let cfg = Rc::new(RefCell::new(Config::load()));
        let (scroll, body) = window::page();

        // ---- General ----
        let g = adw::PreferencesGroup::builder().title(tr("set_general", lang, &[])).build();
        let names: Vec<String> = LANGS.iter().map(|(c, n)| if *c == "system" { tr("set_lang_system", lang, &[]) } else { n.to_string() }).collect();
        let model = gtk4::StringList::new(&names.iter().map(String::as_str).collect::<Vec<_>>());
        let lang_row = adw::ComboRow::builder().title(tr("set_language", lang, &[])).model(&model).build();
        let cur = cfg.borrow().language.clone();
        lang_row.set_selected(LANGS.iter().position(|(c, _)| *c == cur).unwrap_or(0) as u32);
        let (c, h) = (Rc::clone(&cfg), Rc::clone(&hooks));
        lang_row.connect_selected_notify(move |r| {
            let code = LANGS.get(r.selected() as usize).map(|l| l.0).unwrap_or("system");
            c.borrow_mut().language = code.to_string();
            save(&c, &h);
            (h.language)();
        });
        g.add(&lang_row);
        switch(&g, &tr("tray_start_at_login_r1", lang, &[]), None, menew_core::autostart::is_enabled(), |on| {
            let _ = menew_core::autostart::set_enabled(on);
        });
        let (c, h) = (Rc::clone(&cfg), Rc::clone(&hooks));
        let upd = switch(&g, &tr("set_updates", lang, &[]), Some(&tr("set_updates_sub", lang, &[])), cfg.borrow().update_check, move |on| {
            c.borrow_mut().update_check = on;
            save(&c, &h);
        });
        let check = gtk4::Button::builder().label(tr("set_check_now", lang, &[])).valign(gtk4::Align::Center).css_classes(["flat"]).build();
        let h = Rc::clone(&hooks);
        check.connect_clicked(move |_| (h.check_updates)());
        upd.add_suffix(&check);
        let tour = adw::ActionRow::builder().title(tr("set_welcome", lang, &[])).activatable(true).build();
        tour.add_suffix(&gtk4::Image::from_icon_name("go-next-symbolic"));
        let h = Rc::clone(&hooks);
        tour.connect_activated(move |_| (h.welcome)());
        g.add(&tour);
        body.append(&g);

        // ---- Features ----
        let g = adw::PreferencesGroup::builder().title(tr("set_modules", lang, &[])).description(tr("set_modules_sub", lang, &[])).build();
        for (key, sub, get, set) in [
            ("set_mod_clipboard", "set_mod_clipboard_sub", (|c: &Config| c.module_clipboard) as fn(&Config) -> bool, (|c: &mut Config, v| c.module_clipboard = v) as fn(&mut Config, bool)),
            ("set_mod_actions", "set_mod_actions_sub", |c: &Config| c.module_actions, |c: &mut Config, v| c.module_actions = v),
            ("set_mod_notes", "set_mod_notes_sub", |c: &Config| c.module_notes, |c: &mut Config, v| c.module_notes = v),
        ] {
            let (c, h) = (Rc::clone(&cfg), Rc::clone(&hooks));
            let on = get(&cfg.borrow());
            switch(&g, &tr(key, lang, &[]), Some(&tr(sub, lang, &[])), on, move |v| {
                set(&mut c.borrow_mut(), v);
                save(&c, &h);
            });
        }
        body.append(&g);

        // ---- Clipboard ----
        let g = adw::PreferencesGroup::builder().title(tr("kind_clip", lang, &[])).build();
        let (c, h) = (Rc::clone(&cfg), Rc::clone(&hooks));
        switch(&g, &tr("set_paste_auto", lang, &[]), Some(&tr("set_paste_auto_sub", lang, &[])), cfg.borrow().paste_mode == PasteMode::Auto, move |on| {
            c.borrow_mut().paste_mode = if on { PasteMode::Auto } else { PasteMode::CopyOnly };
            save(&c, &h);
        });
        let (c, h) = (Rc::clone(&cfg), Rc::clone(&hooks));
        switch(&g, &tr("set_clip_ocr", lang, &[]), Some(&tr("set_clip_ocr_sub", lang, &[])), cfg.borrow().clipboard_ocr, move |on| {
            c.borrow_mut().clipboard_ocr = on;
            save(&c, &h);
        });
        let size = adw::SpinRow::with_range(20.0, 5000.0, 10.0);
        size.set_title(&tr("set_clip_max", lang, &[]));
        size.set_value(cfg.borrow().clipboard_max_items as f64);
        let (c, h) = (Rc::clone(&cfg), Rc::clone(&hooks));
        size.connect_value_notify(move |r| {
            c.borrow_mut().clipboard_max_items = r.value() as usize;
            save(&c, &h);
        });
        g.add(&size);
        let days = adw::SpinRow::with_range(0.0, 365.0, 1.0);
        days.set_title(&tr("set_clip_days", lang, &[]));
        days.set_subtitle(&tr("set_clip_days_sub", lang, &[]));
        days.set_value(cfg.borrow().clipboard_auto_delete_days as f64);
        let (c, h) = (Rc::clone(&cfg), Rc::clone(&hooks));
        days.connect_value_notify(move |r| {
            c.borrow_mut().clipboard_auto_delete_days = r.value() as u32;
            save(&c, &h);
        });
        g.add(&days);
        let ignored = adw::EntryRow::builder().title(tr("set_clip_ignored", lang, &[])).text(cfg.borrow().clipboard_ignored_apps.join(", ")).show_apply_button(true).build();
        let (c, h) = (Rc::clone(&cfg), Rc::clone(&hooks));
        ignored.connect_apply(move |e| {
            c.borrow_mut().clipboard_ignored_apps = e.text().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            save(&c, &h);
        });
        g.add(&ignored);
        g.add(&adw::ActionRow::builder().title(tr("set_clip_ignored_sub", lang, &[])).css_classes(["dim-label"]).build());
        if let Some(on) = hooks.gnome_extension {
            let row = adw::ActionRow::builder().title(tr("set_gnome_ext", lang, &[])).subtitle(tr(if on { "set_gnome_ext_on" } else { "set_gnome_ext_off" }, lang, &[])).build();
            row.add_prefix(&gtk4::Image::from_icon_name(if on { "emblem-ok-symbolic" } else { "dialog-warning-symbolic" }));
            g.add(&row);
        }
        body.append(&g);

        // ---- World clocks ----
        let g = adw::PreferencesGroup::builder().title(tr("set_clocks", lang, &[])).description(tr("set_clocks_sub", lang, &[])).build();
        let list = gtk4::ListBox::builder().css_classes(["boxed-list"]).selection_mode(gtk4::SelectionMode::None).build();
        let refill: Refill = Rc::new(RefCell::new(None));
        {
            let (c, h, l, rf) = (Rc::clone(&cfg), Rc::clone(&hooks), list.clone(), Rc::clone(&refill));
            let f: Rc<dyn Fn()> = Rc::new(move || {
                while let Some(ch) = l.first_child() {
                    l.remove(&ch);
                }
                for id in c.borrow().world_clocks.clone() {
                    let row = adw::ActionRow::builder().title(id.rsplit('/').next().unwrap_or(&id).replace('_', " ")).subtitle(&id).build();
                    let b = gtk4::Button::builder().icon_name("list-remove-symbolic").valign(gtk4::Align::Center).css_classes(["flat"]).build();
                    let (c2, h2, rf2) = (Rc::clone(&c), Rc::clone(&h), Rc::clone(&rf));
                    b.connect_clicked(move |_| {
                        c2.borrow_mut().world_clocks.retain(|x| *x != id);
                        save(&c2, &h2);
                        if let Some(f) = rf2.borrow().clone() {
                            f();
                        }
                    });
                    row.add_suffix(&b);
                    l.append(&row);
                }
            });
            *refill.borrow_mut() = Some(Rc::clone(&f));
            f();
        }
        g.add(&list);
        let add = adw::EntryRow::builder().title(tr("set_clock_add", lang, &[])).show_apply_button(true).build();
        let (c, h, rf) = (Rc::clone(&cfg), Rc::clone(&hooks), Rc::clone(&refill));
        add.connect_apply(move |e| {
            let text = e.text().to_string();
            let found = time_zones::named_zone(text.trim()).or_else(|| time_zones::zone(text.trim()));
            if let Some(time_zones::Zone::Named(id)) = found {
                if !c.borrow().world_clocks.contains(&id) {
                    c.borrow_mut().world_clocks.push(id);
                    save(&c, &h);
                }
                e.set_text("");
                if let Some(f) = rf.borrow().clone() {
                    f();
                }
            } else {
                e.add_css_class("error");
            }
        });
        g.add(&add);
        body.append(&g);

        // ---- Focus and actions ----
        let g = adw::PreferencesGroup::builder().title(tr("set_focus_actions", lang, &[])).build();
        let (c, h) = (Rc::clone(&cfg), Rc::clone(&hooks));
        switch(&g, &tr("set_focus_breaks", lang, &[]), None, cfg.borrow().focus_breaks, move |on| {
            c.borrow_mut().focus_breaks = on;
            save(&c, &h);
        });
        for (key, get, set) in [
            ("set_focus_mute", (|c: &Config| c.focus_mute_mic) as fn(&Config) -> bool, (|c: &mut Config, v| c.focus_mute_mic = v) as fn(&mut Config, bool)),
            ("set_focus_awake", |c: &Config| c.focus_keep_awake, |c: &mut Config, v| c.focus_keep_awake = v),
            ("set_focus_pause", |c: &Config| c.focus_pause_clipboard, |c: &mut Config, v| c.focus_pause_clipboard = v),
        ] {
            let (c, h) = (Rc::clone(&cfg), Rc::clone(&hooks));
            let on = get(&cfg.borrow());
            switch(&g, &tr(key, lang, &[]), None, on, move |v| {
                set(&mut c.borrow_mut(), v);
                save(&c, &h);
            });
        }
        let never = adw::EntryRow::builder().title(tr("set_never_quit", lang, &[])).text(cfg.borrow().never_quit.join(", ")).show_apply_button(true).build();
        let (c, h) = (Rc::clone(&cfg), Rc::clone(&hooks));
        never.connect_apply(move |e| {
            c.borrow_mut().never_quit = e.text().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            save(&c, &h);
        });
        g.add(&never);
        body.append(&g);

        // ---- Shortcuts ----
        let g = adw::PreferencesGroup::builder().title(tr("set_shortcuts", lang, &[])).description(tr("set_shortcuts_sub", lang, &[])).build();
        for (desc, trigger, cmd) in &hooks.shortcuts {
            let row = adw::ActionRow::builder().title(desc).subtitle(cmd).subtitle_selectable(true).build();
            row.add_suffix(&gtk4::Label::builder().label(trigger).css_classes(["dim-label", "monospace"]).build());
            g.add(&row);
        }
        body.append(&g);

        // ---- Privacy ----
        let g = adw::PreferencesGroup::builder().title(tr("set_privacy", lang, &[])).build();
        g.add(&adw::ActionRow::builder().title(tr("set_privacy_body", lang, &[])).title_lines(0).build());
        g.add(&adw::ActionRow::builder().title(tr("set_config_file", lang, &[])).subtitle(Config::default_path().display().to_string()).subtitle_selectable(true).build());
        body.append(&g);

        Rc::new(Self { root: scroll.upcast() })
    }
}
