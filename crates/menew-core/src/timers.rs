//! Timers and alarms from the palette (port of `Timers.swift`): "timer 10m tea", "timer 1h30",
//! "alarm 7:30 meeting". They live only in memory: quitting Menew cancels them. This module is pure
//! state; the daemon ticks it, shows the countdown in the tray and fires the sound and the
//! notification (`crates/menew`).

use crate::format::{format_time, weekday_time};
use crate::i18n::Language;
use crate::palette::{Icon, ItemAction, Kind, PaletteItem};
use crate::search::fold;
use crate::strings::tr;
use crate::time_zones::{TzCtx, Zone};
use chrono::Timelike;

pub const TIMER_WORDS: &[&str] = &["timer", "temporizador", "cronometro", "cronômetro", "countdown", "contagem", "kurzzeitwecker", "计时"];
pub const ALARM_WORDS: &[&str] = &["alarme", "alarm", "despertador", "despertar", "wecker", "alarma", "闹钟"];

#[derive(Debug, Clone, PartialEq)]
pub struct Countdown {
    pub id: u64,
    pub label: String,
    /// Unix time, in seconds.
    pub end: f64,
    pub is_alarm: bool,
}

#[derive(Debug, Default)]
pub struct TimerStore {
    pub active: Vec<Countdown>,
    next_id: u64,
}

impl TimerStore {
    pub fn add(&mut self, label: &str, end: f64, is_alarm: bool) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.active.push(Countdown { id, label: label.to_string(), end, is_alarm });
        self.active.sort_by(|a, b| a.end.partial_cmp(&b.end).unwrap_or(std::cmp::Ordering::Equal));
        id
    }

    pub fn cancel(&mut self, id: u64) -> bool {
        let n = self.active.len();
        self.active.retain(|c| c.id != id);
        self.active.len() != n
    }

    /// Remove and return the ones that are due at `now`.
    pub fn take_due(&mut self, now: f64) -> Vec<Countdown> {
        let (due, rest): (Vec<_>, Vec<_>) = self.active.drain(..).partition(|c| c.end <= now);
        self.active = rest;
        due
    }

    /// The nearest one for the tray: (label such as "TIMER", value such as "9m").
    pub fn segment(&self, now: f64, lang: Language, local_offset: i32) -> Option<(String, String)> {
        let next = self.active.first()?;
        Some(segment_text(next, now, lang, local_offset))
    }
}

/// Minutes, and seconds in the last minute; an alarm more than an hour away shows its time.
pub fn segment_text(c: &Countdown, now: f64, lang: Language, local_offset: i32) -> (String, String) {
    let left = (c.end - now).ceil().max(0.0) as i64;
    if c.is_alarm && left >= 3600 {
        let t = chrono::DateTime::from_timestamp(c.end as i64 + local_offset as i64, 0).unwrap_or_default();
        return (tr("seg_alarm", lang, &[]), format_time(t.hour(), t.minute(), lang));
    }
    let value = if left >= 3600 {
        format!("{}h{:02}", left / 3600, left % 3600 / 60)
    } else if left >= 60 {
        format!("{}m", (left + 59) / 60)
    } else {
        format!("{left}s")
    };
    (tr(if c.is_alarm { "seg_alarm" } else { "seg_timer" }, lang, &[]), value)
}

/// "10", "10m", "90s", "1h", "1h30", "1:30" -> seconds
pub fn duration(s: &str) -> Option<f64> {
    let t = s.to_lowercase();
    let num = |x: &str| -> Option<f64> { x.parse::<f64>().ok().filter(|v| v.is_finite()) };
    if let Some(n) = num(&t) {
        return Some(n * 60.0);
    }
    if let Some(x) = t.strip_suffix("min") {
        if let Some(n) = num(x) {
            return Some(n * 60.0);
        }
    }
    if let Some(x) = t.strip_suffix('m') {
        if let Some(n) = num(x) {
            return Some(n * 60.0);
        }
    }
    if let Some(x) = t.strip_suffix('s') {
        if let Some(n) = num(x) {
            return Some(n);
        }
    }
    if t.contains('h') {
        let parts: Vec<&str> = t.split('h').collect();
        let h = num(parts[0])?;
        let m = if parts.len() > 1 && !parts[1].is_empty() { num(&parts[1].replace('m', "")).unwrap_or(-1.0) } else { 0.0 };
        return if m < 0.0 { None } else { Some(h * 3600.0 + m * 60.0) };
    }
    let hm: Vec<&str> = t.split(':').filter(|p| !p.is_empty()).collect();
    if hm.len() == 2 {
        if let (Some(h), Some(m)) = (num(hm[0]), num(hm[1])) {
            return Some(h * 3600.0 + m * 60.0);
        }
    }
    None
}

/// "7", "7h", "7:30", "7h30", "19:00", "7am", "7pm" -> the next time that clock time happens (Unix seconds).
pub fn alarm_time(s: &str, ctx: &TzCtx) -> Option<f64> {
    let mut t = s.to_lowercase();
    let (mut pm, mut am) = (false, false);
    if t.ends_with("pm") {
        pm = true;
        t.truncate(t.len() - 2);
    } else if t.ends_with("am") {
        am = true;
        t.truncate(t.len() - 2);
    }
    let t = t.replace('h', ":");
    let parts: Vec<&str> = t.split(':').collect();
    let mut h: u32 = parts.first()?.parse().ok().filter(|h| *h <= 23)?;
    let m: i64 = if parts.len() > 1 && !parts[1].is_empty() { parts[1].parse().ok().unwrap_or(-1) } else { 0 };
    if !(0..60).contains(&m) {
        return None;
    }
    if pm && h < 12 {
        h += 12;
    }
    if am && h == 12 {
        h = 0;
    }
    let off_now = ctx.offset(&Zone::Local, ctx.now)?;
    let now_civil = chrono::DateTime::from_timestamp(ctx.now + off_now as i64, 0)?.naive_utc();
    let mut day = now_civil.date();
    for _ in 0..2 {
        let wall = day.and_hms_opt(h, m as u32, 0)?.and_utc().timestamp();
        let mut ts = wall - off_now as i64;
        if let Some(off) = ctx.offset(&Zone::Local, ts) {
            ts = wall - off as i64;
        }
        if ts > ctx.now {
            return Some(ts as f64);
        }
        day = day.succ_opt()?;
    }
    None
}

/// "9:10 AM" / "09:10" of a Unix time in the computer's zone.
pub fn clock(ts: f64, ctx: &TzCtx, lang: Language) -> String {
    let off = ctx.offset(&Zone::Local, ts as i64).unwrap_or(0);
    let t = chrono::DateTime::from_timestamp(ts as i64 + off as i64, 0).unwrap_or_default();
    format_time(t.hour(), t.minute(), lang)
}

/// "10 min", "1 h 30 min", "45 s"
pub fn span_text(secs: f64) -> String {
    let total = secs as i64;
    let mins = total / 60;
    let rest = total % 60;
    if mins >= 60 {
        format!("{} h {} min", mins / 60, mins % 60)
    } else if mins > 0 {
        format!("{mins} min") + &if rest > 0 { format!(" {rest} s") } else { String::new() }
    } else {
        format!("{rest} s")
    }
}

/// Items for "timer 10m tea", "alarm 7:30" and for cancelling the running ones.
pub fn items(store: &TimerStore, raw: &str, ctx: &TzCtx, lang: Language) -> Vec<PaletteItem> {
    let words: Vec<&str> = raw.trim().split(' ').filter(|s| !s.is_empty()).collect();
    let mut out = vec![];
    if let Some(first) = words.first().map(|w| w.to_lowercase()) {
        if words.len() >= 2 {
            let label = words[2..].join(" ");
            let first_folded = fold(&first);
            if TIMER_WORDS.contains(&first_folded.as_str()) {
                if let Some(secs) = duration(words[1]).filter(|s| *s > 0.0 && *s <= 24.0 * 3600.0) {
                    let span = span_text(secs);
                    let title = if label.is_empty() {
                        tr("timer_span", lang, &[&span])
                    } else {
                        tr("timer_span_label", lang, &[&span, &label])
                    };
                    let ends = clock(ctx.now as f64 + secs, ctx, lang);
                    out.push(
                        PaletteItem::new("timer.new", Kind::Calc, &title)
                            .subtitle(&tr("ends_at", lang, &[&ends]))
                            .kw(raw)
                            .icon(Icon::Symbol("alarm-symbolic"))
                            .forget()
                            .act(ItemAction::StartTimer { label: label.clone(), secs }),
                    );
                }
            }
            if ALARM_WORDS.contains(&first_folded.as_str()) {
                if let Some(end) = alarm_time(words[1], ctx) {
                    let off = ctx.offset(&Zone::Local, end as i64).unwrap_or(0);
                    let dt = chrono::DateTime::from_timestamp(end as i64 + off as i64, 0).unwrap_or_default().naive_utc();
                    let when = weekday_time(&dt, lang);
                    let title = if label.is_empty() {
                        tr("alarm_at", lang, &[&when])
                    } else {
                        tr("alarm_at_label", lang, &[&when, &label])
                    };
                    out.push(
                        PaletteItem::new("alarm.new", Kind::Calc, &title)
                            .subtitle(&tr("alarm_sub", lang, &[]))
                            .kw(raw)
                            .icon(Icon::Symbol("alarm-symbolic"))
                            .forget()
                            .act(ItemAction::StartAlarm { label, end }),
                    );
                }
            }
        }
    }
    // running timers: "timer" or "alarm" lists them to cancel
    let q = fold(raw.trim());
    if q.chars().count() >= 3
        && TIMER_WORDS.iter().chain(ALARM_WORDS.iter()).chain(["cancelar", "cancel"].iter()).any(|w| w.starts_with(&q) || q.starts_with(w))
    {
        for c in &store.active {
            let when = clock(c.end, ctx, lang);
            let word = tr(if c.is_alarm { "word_alarm" } else { "word_timer" }, lang, &[]);
            let mut it = PaletteItem::new(&format!("timer.cancel.{}", c.id), Kind::Action, &tr("cancel_timer", lang, &[&word, &when]))
                .kw(raw)
                .icon(Icon::Symbol("edit-delete-symbolic"))
                .forget()
                .act(ItemAction::CancelTimer(c.id));
            if !c.label.is_empty() {
                it = it.subtitle(&c.label);
            }
            out.push(it);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(duration("10"), Some(600.0));
        assert_eq!(duration("10m"), Some(600.0));
        assert_eq!(duration("10min"), Some(600.0));
        assert_eq!(duration("90s"), Some(90.0));
        assert_eq!(duration("1h"), Some(3600.0));
        assert_eq!(duration("1h30"), Some(5400.0));
        assert_eq!(duration("1h30m"), Some(5400.0));
        assert_eq!(duration("1:30"), Some(5400.0));
        assert_eq!(duration("abc"), None);
        assert_eq!(duration("xh"), None);
    }

    #[test]
    fn spans() {
        assert_eq!(span_text(600.0), "10 min");
        assert_eq!(span_text(5400.0), "1 h 30 min");
        assert_eq!(span_text(90.0), "1 min 30 s");
        assert_eq!(span_text(45.0), "45 s");
    }

    #[test]
    fn store_orders_by_end_and_fires_due_ones() {
        let mut s = TimerStore::default();
        let b = s.add("b", 200.0, false);
        let a = s.add("a", 100.0, false);
        assert_eq!(s.active[0].id, a);
        assert_eq!(s.take_due(150.0).len(), 1);
        assert_eq!(s.active.len(), 1);
        assert!(s.cancel(b));
        assert!(!s.cancel(b));
    }

    #[test]
    fn segment_formats() {
        let c = |end: f64| Countdown { id: 1, label: String::new(), end, is_alarm: false };
        assert_eq!(segment_text(&c(1000.0 + 5400.0), 1000.0, Language::En, 0).1, "1h30");
        assert_eq!(segment_text(&c(1000.0 + 540.0), 1000.0, Language::En, 0).1, "9m");
        assert_eq!(segment_text(&c(1000.0 + 59.0), 1000.0, Language::En, 0).1, "59s");
    }
}
