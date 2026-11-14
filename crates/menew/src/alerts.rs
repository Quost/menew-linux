//! Side effects of timers, alarms and the focus timer: the countdown in the tray, the notification
//! and the sound when time is up. The state itself (`TimerStore`, `FocusTimer`) is pure and lives
//! in `menew-core`; the palette (`menew-ui`) starts and cancels them and this module ticks them.

use gtk4::gio;
use gtk4::prelude::*;
use libadwaita as adw;
use menew_core::focus::{FocusEvent, NoHooks};
use menew_core::strings::tr;
use menew_core::time_zones::{TzCtx, Zone};
use menew_core::timers::{clock, segment_text};
use menew_core::Language;
use menew_tray::TrayStatus;
use menew_ui::PaletteState;
use std::process::{Command, Stdio};
use tracing::{info, warn};

pub fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// What the tray shows now: "FOCUS 24m · TIMER 9m" in the title and one line per item in the tooltip.
pub fn tray_status(state: &PaletteState, now: f64, lang: Language) -> TrayStatus {
    let off = TzCtx::system().offset(&Zone::Local, now as i64).unwrap_or(0);
    let mut parts: Vec<String> = vec![];
    let mut lines: Vec<String> = vec![];
    if let Some((label, value)) = state.focus.segment(now, lang) {
        parts.push(format!("{label} {value}"));
        lines.push(format!("{label} {value}"));
    }
    for (i, c) in state.timers.active.iter().enumerate() {
        let (label, value) = segment_text(c, now, lang, off);
        if i == 0 {
            parts.push(format!("{label} {value}"));
        }
        lines.push(if c.label.is_empty() { format!("{label} {value}") } else { format!("{label} {value}: {}", c.label) });
    }
    TrayStatus { summary: parts.join(" \u{b7} "), lines }
}

/// Fire what is due at `now`. Returns true when something ended (the tray needs a refresh).
pub fn tick(app: &adw::Application, state: &mut PaletteState, now: f64, lang: Language) -> bool {
    let mut changed = false;
    let off_ctx = TzCtx::system();
    for c in state.timers.take_due(now) {
        changed = true;
        let title = tr(if c.is_alarm { "alarm" } else { "timer_done" }, lang, &[]);
        let title = if c.label.is_empty() { title } else { format!("{title}: {}", c.label) };
        let body = if c.is_alarm { clock(c.end, &off_ctx, lang) } else { tr("time_up", lang, &[]) };
        notify(app, &format!("timer-{}", c.id), &title, &body);
        play_sound();
    }
    if let Some(ev) = state.focus.tick(now, &mut NoHooks) {
        changed = true;
        match ev {
            FocusEvent::FocusFinished { rest_minutes } => {
                let body = tr(if rest_minutes > 0 { "break_5" } else { "nice_work" }, lang, &[]);
                notify(app, "focus", &tr("focus_done", lang, &[]), &body);
            }
            FocusEvent::BreakFinished => notify(app, "focus", &tr("break_over", lang, &[]), &tr("ready_again", lang, &[])),
        }
        play_sound();
    }
    changed
}

fn notify(app: &adw::Application, id: &str, title: &str, body: &str) {
    let n = gio::Notification::new(title);
    n.set_body(Some(body));
    n.set_priority(gio::NotificationPriority::High);
    app.send_notification(Some(id), &n);
    info!("Notification sent: {} / {}", title, body);
}

/// The sound of the freedesktop sound theme: `canberra-gtk-play` if present, else `paplay` with the
/// theme file, else a beep. No new crate; the method used is logged.
pub fn play_sound() {
    fn detached(mut cmd: Command) -> bool {
        match cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            Ok(mut child) => {
                // reap the child without blocking the main loop
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                true
            }
            Err(_) => false,
        }
    }
    let mut c = Command::new("canberra-gtk-play");
    c.args(["-i", "complete", "-d", "Menew"]);
    if detached(c) {
        info!("Sound: canberra-gtk-play (event sound 'complete')");
        return;
    }
    for file in ["/usr/share/sounds/freedesktop/stereo/complete.oga", "/usr/share/sounds/freedesktop/stereo/bell.oga"] {
        if std::path::Path::new(file).exists() {
            let mut p = Command::new("paplay");
            p.arg(file);
            if detached(p) {
                info!("Sound: paplay {}", file);
                return;
            }
        }
    }
    if let Some(display) = gtk4::gdk::Display::default() {
        display.beep();
        info!("Sound: display beep (no sound player found)");
    } else {
        warn!("Sound: nothing to play it with");
    }
}
