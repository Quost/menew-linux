//! The tray popover, the Settings and Clipboard sections, the welcome tour, the update check and the
//! features that can be turned off.

use crate::alerts;
use crate::commands::Commands;
use crate::monitor::Sampler;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use menew_core::palette::ItemAction;
use menew_core::strings::tr;
use menew_core::sysmon;
use menew_core::time_zones::{self, TzCtx};
use menew_core::{current_language, updates, Config};
use menew_ui::popover::{ActionEntry, Hooks, NowData, Popover};
use menew_ui::window::Section;
use std::rc::Rc;
use tracing::{info, warn};

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The group of a command id in the Actions tab.
pub fn group_of(cmd: &str) -> &'static str {
    let name = cmd.split(':').next().unwrap_or(cmd);
    match name {
        _ if name.starts_with("cap.") => "grp_screen",
        "qa.color" => "grp_screen",
        _ if name.starts_with("win.") => "grp_windows",
        "qa.bright" => "grp_windows",
        "qa.mic" | "qa.sound" | "qa.out" | "qa.in" | "qa.awake" => "grp_sound",
        _ if name.starts_with("qa.") => "grp_system",
        _ => "grp_menew",
    }
}

const GROUPS: [&str; 5] = ["grp_screen", "grp_windows", "grp_sound", "grp_system", "grp_menew"];

fn icon_name(i: &menew_core::palette::Icon) -> String {
    match i {
        menew_core::palette::Icon::Symbol(s) => s.to_string(),
        menew_core::palette::Icon::App(Some(a)) => a.clone(),
        _ => "system-run-symbolic".to_string(),
    }
}

/// Every command the Actions tab offers, grouped.
pub fn action_groups(cmds: &Commands) -> Vec<(String, Vec<ActionEntry>)> {
    let lang = current_language();
    let items = cmds.items("");
    let mut out: Vec<(String, Vec<ActionEntry>)> = GROUPS.iter().map(|g| (tr(g, lang, &[]), vec![])).collect();
    for it in items {
        let ItemAction::Command(c) = &it.action else { continue };
        if c.is_empty() || c.starts_with("note.open") || c.starts_with("cap.qrshow") {
            continue;
        }
        let g = GROUPS.iter().position(|x| *x == group_of(c)).unwrap_or(4);
        if !out[g].1.iter().any(|a| a.command == *c) {
            out[g].1.push(ActionEntry { command: c.clone(), title: it.title.clone(), icon: icon_name(&it.icon) });
        }
    }
    // the windows group is long: keep the common ones
    out[1].1.retain(|a| !a.command.starts_with("win.") || ["win.left", "win.right", "win.max", "win.center", "win.display", "win.almost"].contains(&a.command.as_str()));
    out
}

fn now_data(state: &menew_ui::SharedState, cmds: &Commands, sampler: &Sampler) -> NowData {
    let lang = current_language();
    let s = sampler.last.borrow().clone();
    let warn = sysmon::attention(&s, lang);
    let mut metrics = vec![("CPU".to_string(), format!("{:.0}%", s.cpu)), (tr("mon_ram", lang, &[]), format!("{:.0}%", s.memory.used_percent()))];
    if let Some(d) = &s.disk {
        metrics.push((tr("mon_disk", lang, &[]), sysmon::bytes(d.free, lang)));
    }
    if let Some(b) = &s.battery {
        metrics.push((tr("mon_battery", lang, &[]), format!("{}{}%", if b.charging() { "⚡" } else { "" }, b.percent)));
    }
    let st = state.borrow();
    let mut running = alerts::tray_status(&st, alerts::now(), lang).lines;
    if cmds.tools().awake {
        running.push(match cmds.awake_until() {
            Some(u) => tr("pop_awake_until", lang, &[&menew_core::timers::clock(u, &TzCtx::system(), lang)]),
            None => tr("pop_awake_on", lang, &[]),
        });
    }
    let ctx = TzCtx::system();
    let ts = now_secs() as i64;
    let clocks = st.clocks.iter().map(|z| (time_zones::city(z, lang), time_zones::time(&ctx, ts, z, lang))).collect();
    NowData { status: if warn.is_empty() { tr("mon_all_good", lang, &[]) } else { warn.join(" · ") }, metrics, running, clocks }
}

/// Check GitHub releases now (`manual`) or when due. Notifies only when there is news, or always
/// when checked by hand.
pub fn check_updates(app: &adw::Application, manual: bool) {
    let mut cfg = Config::load();
    if !manual && (!cfg.update_check || !updates::due(cfg.update_checked_at as f64, now_secs() as f64)) {
        return;
    }
    cfg.update_checked_at = now_secs();
    let _ = cfg.save_to(&Config::default_path());
    info!("Update check (GitHub releases)");
    let app = app.clone();
    glib::spawn_future_local(async move {
        let lang = current_language();
        let latest = gio::spawn_blocking(updates::fetch_latest).await.ok().flatten();
        let current = env!("CARGO_PKG_VERSION");
        let n = match latest {
            Some(tag) if updates::is_newer(&tag, current) => {
                let n = gio::Notification::new(&tr("set_update_available", lang, &[tag.trim_start_matches('v')]));
                n.set_body(Some(&tr("set_update_how", lang, &[])));
                n.set_default_action_and_target_value("app.open-uri", Some(&updates::RELEASES_PAGE.to_variant()));
                Some(n)
            }
            Some(_) if manual => Some(gio::Notification::new(&tr("set_up_to_date", lang, &[current]))),
            None if manual => Some(gio::Notification::new(&tr("set_update_fail", lang, &[]))),
            _ => None,
        };
        if let Some(n) = n {
            app.send_notification(Some("menew-update"), &n);
        }
    });
}

/// Apply the features switched on or off.
pub fn apply_modules(cfg: &Config, palette: &menew_ui::PaletteWindow, cmds: &Commands) {
    palette.set_clipboard_enabled(cfg.module_clipboard);
    cmds.set_modules(cfg.module_actions, cfg.module_notes);
}

pub fn restart() {
    if let Ok(exe) = std::env::current_exe() {
        // the new process becomes the primary instance once this one is gone
        let _ = std::process::Command::new("sh").arg("-c").arg("sleep 1; exec \"$0\"").arg(exe).spawn();
    }
    if let Some(app) = gio::Application::default() {
        app.quit();
    }
}

pub struct ShellParts {
    pub settings: Section,
    pub clipboard: Section,
}

/// The Settings and Clipboard sections.
pub fn sections(app: &adw::Application, palette: &Rc<menew_ui::PaletteWindow>, cmds: &Rc<Commands>, on_config: Rc<dyn Fn(&Config)>, gnome_ext: Option<bool>) -> ShellParts {
    let lang = current_language();
    let mut shortcuts = vec![
        (tr("shortcut_palette_r1", lang, &[]), "Ctrl+Alt+Space".to_string(), "menew toggle".to_string()),
        (menew_core::t("shortcut_clipboard_desc").to_string(), "Ctrl+Alt+V".to_string(), "menew clipboard".to_string()),
        (menew_core::t("shortcut_paste_next_desc").to_string(), "Ctrl+Alt+N".to_string(), "menew paste-next".to_string()),
    ];
    for (_, key, trigger, cmd) in menew_shortcuts::COMMAND_SHORTCUTS {
        shortcuts.push((tr(key, lang, &[]), trigger.replace("Control", "Ctrl"), format!("menew run {cmd}")));
    }
    let app2 = app.clone();
    let app3 = app.clone();
    let oc = Rc::clone(&on_config);
    let view = menew_ui::settings::SettingsView::new(menew_ui::settings::SettingsHooks {
        changed: Rc::new(move |c| oc(c)),
        language: Rc::new(move || {
            let lang = Config::load().effective_language();
            menew_ui::dialogs::confirm(None, &tr("set_lang_restart", lang, &[]), "", &tr("set_restart", lang, &[]), &tr("set_later", lang, &[]), false, restart);
        }),
        check_updates: Rc::new(move || check_updates(&app2, true)),
        welcome: Rc::new(move || {
            let oc = Rc::clone(&on_config);
            menew_ui::welcome::show(&app3, Rc::new(move |c| oc(c)));
        }),
        shortcuts,
        gnome_extension: gnome_ext,
    });
    let settings = Section { id: "settings", title: tr("set_title", lang, &[]), icon: "emblem-system-symbolic", page: view.root.clone(), header: vec![], on_show: None };

    let open = gtk4::Button::builder().label(tr("act_clipboard", lang, &[])).halign(gtk4::Align::Center).css_classes(["pill", "suggested-action"]).build();
    let p = Rc::clone(palette);
    open.connect_clicked(move |_| p.show_mode(menew_core::PaletteMode::Clipboard));
    let page = adw::StatusPage::builder().icon_name("edit-paste-symbolic").title(tr("act_clipboard", lang, &[])).description(tr("act_sub_clipboard", lang, &[])).child(&open).build();
    let clipboard = Section { id: "clipboard", title: tr("kind_clip", lang, &[]), icon: "edit-paste-symbolic", page: page.upcast(), header: vec![], on_show: None };

    let _ = cmds;
    ShellParts { settings, clipboard }
}

pub fn popover(app: &adw::Application, cmds: &Rc<Commands>, palette: &Rc<menew_ui::PaletteWindow>, state: &menew_ui::SharedState, sampler: &Rc<Sampler>, open_window: Rc<dyn Fn()>) -> Rc<Popover> {
    let (c1, c2, c3) = (Rc::clone(cmds), Rc::clone(cmds), Rc::clone(cmds));
    let p = Rc::clone(palette);
    let (st, smp) = (Rc::clone(state), Rc::clone(sampler));
    Popover::new(
        app,
        Hooks {
            run: Rc::new(move |c| c1.run(c)),
            search: Rc::new(move |t| p.show_with_text(t)),
            groups: Rc::new(move || action_groups(&c2)),
            favorites: Rc::new(|| Config::load().favorites),
            set_favorites: Rc::new(|f| {
                let mut c = Config::load();
                c.favorites = f;
                if let Err(e) = c.save_to(&Config::default_path()) {
                    warn!("Could not save the favorites: {}", e);
                }
            }),
            now: Rc::new(move || now_data(&st, &c3, &smp)),
            open_window,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_like_the_mac() {
        assert_eq!(group_of("cap.area"), "grp_screen");
        assert_eq!(group_of("qa.color"), "grp_screen");
        assert_eq!(group_of("win.left"), "grp_windows");
        assert_eq!(group_of("qa.out:x"), "grp_sound");
        assert_eq!(group_of("qa.trash"), "grp_system");
        assert_eq!(group_of("note.new"), "grp_menew");
    }
}
