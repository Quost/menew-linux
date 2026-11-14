//! Time zones in the palette (port of `WorldClock.swift`): "15h sp em lisboa", "3pm ny to london",
//! "hora em Tóquio", "lisboa".
//!
//! There is no time zone crate in the tree: zone names are checked against `/usr/share/zoneinfo`
//! and the UTC offset of a zone at an instant comes from `date` (coreutils) with `TZ` set, which
//! applies the DST rules of the installed tzdata. Nothing here touches the network.

use crate::format::format_time;
use crate::i18n::Language;
use crate::search::fold;
use crate::strings::tr;
use chrono::{Datelike, NaiveDate, TimeZone as _, Timelike};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// A time zone: the one of the computer, or a tz database name ("Europe/Lisbon").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Zone {
    Local,
    Named(String),
}

/// What "now" and "here" mean. In the app `local` is `None` (the computer's zone); tests pin both.
#[derive(Debug, Clone)]
pub struct TzCtx {
    pub now: i64,
    pub local: Option<String>,
}

impl TzCtx {
    pub fn system() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Self { now, local: None }
    }

    /// Identifier of the computer's zone ("America/Sao_Paulo"), if it can be found.
    pub fn local_id(&self) -> Option<String> {
        if let Some(id) = &self.local {
            return Some(id.clone());
        }
        if let Ok(tz) = std::env::var("TZ") {
            let tz = tz.trim_start_matches(':').to_string();
            if zone_exists(&tz) {
                return Some(tz);
            }
        }
        let link = std::fs::read_link("/etc/localtime").ok()?;
        let s = link.to_string_lossy().to_string();
        let id = s.split("zoneinfo/").nth(1)?.to_string();
        zone_exists(&id).then_some(id)
    }

    /// UTC offset in seconds of `zone` at the instant `ts`.
    pub fn offset(&self, zone: &Zone, ts: i64) -> Option<i32> {
        match zone {
            Zone::Named(id) => named_offset(id, ts),
            Zone::Local => match &self.local {
                Some(id) => named_offset(id, ts),
                None => chrono::Local.timestamp_opt(ts, 0).single().map(|d| d.offset().local_minus_utc()),
            },
        }
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() < 64
        && !id.contains("..")
        && id.chars().all(|c| c.is_ascii_alphanumeric() || "_/+-".contains(c))
}

fn named_offset(id: &str, ts: i64) -> Option<i32> {
    // `date` treats an unknown TZ as UTC, so the name must exist in the tz database first
    if !zone_exists(id) {
        return None;
    }
    static CACHE: Mutex<Option<HashMap<(String, i64), i32>>> = Mutex::new(None);
    let key = (id.to_string(), ts);
    if let Some(v) = CACHE.lock().ok()?.as_ref().and_then(|m| m.get(&key)) {
        return Some(*v);
    }
    let out = std::process::Command::new("date")
        .env("TZ", id)
        .args(["-d", &format!("@{ts}"), "+%z"])
        .output()
        .ok()?;
    let s = String::from_utf8(out.stdout).ok()?;
    let s = s.trim();
    if s.len() != 5 {
        return None;
    }
    let sign = match &s[..1] {
        "+" => 1,
        "-" => -1,
        _ => return None,
    };
    let h: i32 = s[1..3].parse().ok()?;
    let m: i32 = s[3..5].parse().ok()?;
    let v = sign * (h * 3600 + m * 60);
    CACHE.lock().ok()?.get_or_insert_with(HashMap::new).insert(key, v);
    Some(v)
}

// ---- names ----

/// Aliases and names in Portuguese, English, German, Spanish and Chinese.
const ALIASES: &[(&str, &str)] = &[
    ("sp", "America/Sao_Paulo"), ("sao paulo", "America/Sao_Paulo"), ("brasil", "America/Sao_Paulo"),
    ("brasilia", "America/Sao_Paulo"), ("rio", "America/Sao_Paulo"), ("brazil", "America/Sao_Paulo"),
    ("巴西", "America/Sao_Paulo"),
    ("ny", "America/New_York"), ("nyc", "America/New_York"), ("nova york", "America/New_York"),
    ("new york", "America/New_York"), ("纽约", "America/New_York"),
    ("sf", "America/Los_Angeles"), ("la", "America/Los_Angeles"), ("san francisco", "America/Los_Angeles"),
    ("california", "America/Los_Angeles"),
    ("londres", "Europe/London"), ("london", "Europe/London"), ("uk", "Europe/London"), ("伦敦", "Europe/London"),
    ("lisboa", "Europe/Lisbon"), ("lisbon", "Europe/Lisbon"), ("lissabon", "Europe/Lisbon"),
    ("portugal", "Europe/Lisbon"),
    // linux: the Mac has no Chinese name for Lisbon, but its own zh tip uses one
    ("里斯本", "Europe/Lisbon"),
    ("berlim", "Europe/Berlin"), ("berlin", "Europe/Berlin"), ("alemanha", "Europe/Berlin"),
    ("germany", "Europe/Berlin"), ("deutschland", "Europe/Berlin"),
    ("madri", "Europe/Madrid"), ("madrid", "Europe/Madrid"), ("espanha", "Europe/Madrid"),
    ("paris", "Europe/Paris"), ("franca", "Europe/Paris"),
    ("toquio", "Asia/Tokyo"), ("tokyo", "Asia/Tokyo"), ("tokio", "Asia/Tokyo"), ("japao", "Asia/Tokyo"),
    ("japan", "Asia/Tokyo"), ("东京", "Asia/Tokyo"),
    ("xangai", "Asia/Shanghai"), ("shanghai", "Asia/Shanghai"), ("pequim", "Asia/Shanghai"),
    ("beijing", "Asia/Shanghai"), ("china", "Asia/Shanghai"), ("上海", "Asia/Shanghai"), ("北京", "Asia/Shanghai"),
    ("india", "Asia/Kolkata"), ("bangalore", "Asia/Kolkata"), ("dubai", "Asia/Dubai"),
    ("sydney", "Australia/Sydney"), ("sidney", "Australia/Sydney"),
    ("mexico", "America/Mexico_City"), ("cdmx", "America/Mexico_City"),
    ("buenos aires", "America/Argentina/Buenos_Aires"),
    ("chicago", "America/Chicago"), ("toronto", "America/Toronto"),
    ("singapura", "Asia/Singapore"), ("singapore", "Asia/Singapore"),
    ("utc", "UTC"), ("gmt", "UTC"),
];

const ZONEINFO: &str = "/usr/share/zoneinfo";

fn zone_exists(id: &str) -> bool {
    valid_id(id) && (id == "UTC" || Path::new(ZONEINFO).join(id).is_file())
}

/// A zone from a tz database name found in the config ("Europe/Lisbon"); `None` if it is not installed.
pub fn named_zone(id: &str) -> Option<Zone> {
    zone_exists(id).then(|| Zone::Named(id.to_string()))
}

/// Identifiers of the tz database (`Region/City`), sorted.
fn known_ids() -> &'static Vec<String> {
    static IDS: OnceLock<Vec<String>> = OnceLock::new();
    IDS.get_or_init(|| {
        let mut out = Vec::new();
        for region in ["Africa", "America", "Antarctica", "Asia", "Atlantic", "Australia", "Europe", "Indian", "Pacific"] {
            let base = Path::new(ZONEINFO).join(region);
            for e in walkdir::WalkDir::new(&base).into_iter().filter_map(Result::ok) {
                if e.file_type().is_file() {
                    if let Ok(rel) = e.path().strip_prefix(ZONEINFO) {
                        out.push(rel.to_string_lossy().replace('\\', "/"));
                    }
                }
            }
        }
        out.sort();
        out
    })
}

fn city_of(id: &str) -> String {
    fold(id.rsplit('/').next().unwrap_or("")).replace('_', " ")
}

/// Zone of a typed name: alias, a city of the tz database ("denver", "auckland") or "aqui".
pub fn zone(text: &str) -> Option<Zone> {
    let q = fold(text.trim()).replace('_', " ");
    if q.chars().count() < 2 {
        return None;
    }
    if ["aqui", "here", "hier", "aca", "local", "这里"].contains(&q.as_str()) {
        return Some(Zone::Local);
    }
    if let Some((_, id)) = ALIASES.iter().find(|(a, _)| *a == q) {
        return zone_exists(id).then(|| Zone::Named((*id).to_string()));
    }
    let long = q.chars().count() >= 4;
    known_ids()
        .iter()
        .find(|id| {
            let city = city_of(id);
            city == q || (long && city.starts_with(&q))
        })
        .map(|id| Zone::Named(id.clone()))
}

/// Cities to pick in the settings, from what was typed (up to 6).
pub fn search(text: &str) -> Vec<Zone> {
    let q = fold(text.trim());
    if q.chars().count() < 2 {
        return vec![];
    }
    let mut out: Vec<Zone> = vec![];
    for (alias, id) in ALIASES {
        if alias.starts_with(&q) && zone_exists(id) {
            let z = Zone::Named((*id).to_string());
            if !out.contains(&z) {
                out.push(z);
            }
        }
    }
    for id in known_ids() {
        let city = city_of(id);
        if city.starts_with(&q) || city.contains(&format!(" {q}")) {
            let z = Zone::Named(id.clone());
            if !out.contains(&z) {
                out.push(z);
            }
        }
    }
    out.truncate(6);
    out
}

/// "America/Sao_Paulo" -> "São Paulo"; the well-known ones are translated.
pub fn city(zone: &Zone, lang: Language) -> String {
    let id = match zone {
        Zone::Local => return String::new(),
        Zone::Named(id) => id.as_str(),
    };
    let key = match id {
        "America/New_York" => Some("city_new_york"),
        "Asia/Tokyo" => Some("city_tokyo"),
        "Europe/London" => Some("city_london"),
        "Europe/Lisbon" => Some("city_lisbon"),
        "Asia/Shanghai" => Some("city_shanghai"),
        "Europe/Berlin" => Some("city_berlin"),
        "America/Mexico_City" => Some("city_mexico_city"),
        "Asia/Kolkata" => Some("city_india"),
        _ => None,
    };
    if let Some(k) = key {
        return tr(k, lang, &[]);
    }
    match id {
        "America/Sao_Paulo" => "São Paulo".into(),
        "America/Los_Angeles" => "Los Angeles".into(),
        "UTC" => "UTC".into(),
        other => other.rsplit('/').next().unwrap_or(other).replace('_', " "),
    }
}

/// The tz database name of a zone (the computer's one for `Local`).
pub fn resolve(ctx: &TzCtx, zone: &Zone) -> Option<String> {
    match zone {
        Zone::Named(id) => Some(id.clone()),
        Zone::Local => ctx.local_id(),
    }
}

/// City name of a zone; for `Local`, the city of the computer's zone.
pub fn city_of_zone(ctx: &TzCtx, zone: &Zone, lang: Language) -> String {
    match zone {
        Zone::Named(_) => city(zone, lang),
        Zone::Local => ctx.local_id().map(|id| city(&Zone::Named(id), lang)).unwrap_or_default(),
    }
}

// ---- civil time ----

fn civil(ts: i64, off: i32) -> chrono::NaiveDateTime {
    chrono::DateTime::from_timestamp(ts + off as i64, 0).unwrap_or_default().naive_utc()
}

/// "8:00 PM" / "20:00" of the instant `ts` in `zone`.
pub fn time(ctx: &TzCtx, ts: i64, zone: &Zone, lang: Language) -> String {
    let off = ctx.offset(zone, ts).unwrap_or(0);
    let c = civil(ts, off);
    format_time(c.hour(), c.minute(), lang)
}

/// "+4 h", "−3 h", "+5:30" or "same time"
pub fn offset_text(ctx: &TzCtx, zone: &Zone, ts: i64, from: &Zone, lang: Language) -> String {
    let diff = ctx.offset(zone, ts).unwrap_or(0) - ctx.offset(from, ts).unwrap_or(0);
    if diff == 0 {
        return tr("same_time", lang, &[]);
    }
    let sign = if diff > 0 { "+" } else { "\u{2212}" };
    let h = diff.abs() / 3600;
    let m = diff.abs() % 3600 / 60;
    if m == 0 {
        format!("{sign}{h} h")
    } else {
        format!("{sign}{h}:{m:02}")
    }
}

/// "tomorrow", "yesterday" or nothing, comparing the day there with the day in `from`.
pub fn day_note(ctx: &TzCtx, ts: i64, zone: &Zone, from: &Zone, lang: Language) -> Option<String> {
    let x = civil(ts, ctx.offset(zone, ts)?).date();
    let y = civil(ts, ctx.offset(from, ts)?).date();
    if x == y {
        None
    } else if x > y {
        Some(tr("tomorrow", lang, &[]))
    } else {
        Some(tr("yesterday", lang, &[]))
    }
}

/// "15h", "15:30", "15h30", "3pm", "9am"
fn parse_time(s: &str) -> Option<(u32, u32)> {
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
    let h0: u32 = parts.first()?.parse().ok().filter(|h| *h <= 23)?;
    let m: i64 = if parts.len() > 1 && !parts[1].is_empty() { parts[1].parse().ok().unwrap_or(-1) } else { 0 };
    if !(0..60).contains(&m) {
        return None;
    }
    // a lone number only counts as a time with "h", ":" or am/pm, so it is not mistaken for math
    if !(s.contains('h') || s.contains(':') || pm || am) {
        return None;
    }
    let mut h = h0;
    if pm && h < 12 {
        h += 12;
    }
    if am && h == 12 {
        h = 0;
    }
    Some((h, m as u32))
}

/// A parsed time query: the instant, the zone it was typed in and the one to show it in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeQuery {
    pub ts: i64,
    pub from: Zone,
    pub to: Zone,
}

const JOINERS: &[&str] = &["em", "in", "to", "para", "pra", "nach", "en", "a", "→", "->", "到"];
const HOUR_WORDS: &[&str] = &["hora", "horas", "time", "uhrzeit", "zeit", "時間", "时间"];

/// "15h sp em lisboa", "15:30 em nova york", "3pm ny to london", "hora em tóquio", "lisboa".
/// `q` is the folded query.
pub fn palette_query(ctx: &TzCtx, q: &str) -> Option<TimeQuery> {
    let words: Vec<&str> = q.split(' ').filter(|s| !s.is_empty()).collect();
    if words.is_empty() {
        return None;
    }
    // just the city ("lisboa") or "hora em lisboa": the time there now
    if !q.chars().any(|c| c.is_numeric()) {
        if let Some(tz) = zone(q) {
            return Some(TimeQuery { ts: ctx.now, from: Zone::Local, to: tz });
        }
    }
    if HOUR_WORDS.contains(&words[0]) && words.len() >= 3 && JOINERS.contains(&words[1]) {
        if let Some(tz) = zone(&words[2..].join(" ")) {
            return Some(TimeQuery { ts: ctx.now, from: Zone::Local, to: tz });
        }
    }

    // starts with a time
    let (h, m) = parse_time(words[0])?;
    let mut rest: Vec<&str> = words[1..].to_vec();
    if let Some(first) = rest.first() {
        if *first == "am" || *first == "pm" {
            let suffix = rest.remove(0);
            let (h2, m2) = parse_time(&format!("{}{}", words[0], suffix))?;
            return convert(ctx, h2, m2, &rest);
        }
    }
    return convert(ctx, h, m, &rest);

    fn convert(ctx: &TzCtx, h: u32, m: u32, rest: &[&str]) -> Option<TimeQuery> {
        let j = rest.iter().position(|w| JOINERS.contains(w))?;
        if j + 1 >= rest.len() {
            return None;
        }
        let to = zone(&rest[j + 1..].join(" "))?;
        let from_text = rest[..j].join(" ");
        let from = if from_text.is_empty() { Zone::Local } else { zone(&from_text).unwrap_or(Zone::Local) };
        // today at h:m in the `from` zone
        let off_now = ctx.offset(&from, ctx.now)?;
        let today = civil(ctx.now, off_now).date();
        let naive = NaiveDate::from_ymd_opt(today.year(), today.month(), today.day())?.and_hms_opt(h, m, 0)?;
        let wall = naive.and_utc().timestamp();
        let mut ts = wall - off_now as i64;
        let off_then = ctx.offset(&from, ts)?;
        if off_then != off_now {
            ts = wall - off_then as i64;
        }
        Some(TimeQuery { ts, from, to })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_time_forms() {
        assert_eq!(parse_time("15h"), Some((15, 0)));
        assert_eq!(parse_time("15:30"), Some((15, 30)));
        assert_eq!(parse_time("15h30"), Some((15, 30)));
        assert_eq!(parse_time("3pm"), Some((15, 0)));
        assert_eq!(parse_time("12am"), Some((0, 0)));
        assert_eq!(parse_time("12pm"), Some((12, 0)));
        // a lone number is math, not a time
        assert_eq!(parse_time("15"), None);
        assert_eq!(parse_time("25h"), None);
        assert_eq!(parse_time("10:75"), None);
    }

    /// 2026-11-14 12:00 UTC, the computer in Sao Paulo (UTC-3)
    fn ctx() -> TzCtx {
        TzCtx { now: 1794657600, local: Some("America/Sao_Paulo".into()) }
    }

    fn named(id: &str) -> Zone {
        Zone::Named(id.into())
    }

    // The vectors V022 to V031 of docs/palette-spec.md
    #[test]
    fn spec_vectors() {
        let c = ctx();
        let en = Language::En;
        let q = palette_query(&c, "3pm ny to lisbon").unwrap();
        assert_eq!((q.from.clone(), q.to.clone()), (named("America/New_York"), named("Europe/Lisbon")));
        assert_eq!(q.ts, 1794657600 + 8 * 3600); // 20:00 UTC
        assert_eq!(time(&c, q.ts, &q.to, en), "8:00 PM");
        assert_eq!(time(&c, q.ts, &q.from, en), "3:00 PM");
        assert_eq!(offset_text(&c, &q.to, q.ts, &q.from, en), "+5 h");
        assert_eq!(day_note(&c, q.ts, &q.to, &q.from, en), None);

        let q = palette_query(&c, "15h em lisboa").unwrap();
        assert_eq!((q.from, q.to.clone()), (Zone::Local, named("Europe/Lisbon")));
        assert_eq!(time(&c, q.ts, &q.to, Language::Pt), "18:00");

        let q = palette_query(&c, "time in tokyo").unwrap();
        assert_eq!((q.ts, q.to.clone()), (c.now, named("Asia/Tokyo")));
        assert_eq!(time(&c, q.ts, &q.to, en), "9:00 PM");
        assert_eq!(offset_text(&c, &q.to, q.ts, &Zone::Local, en), "+12 h");

        // the day changes: 11pm in New York is tomorrow in Tokyo
        let q = palette_query(&c, "11pm ny to tokyo").unwrap();
        assert_eq!(day_note(&c, q.ts, &q.to, &q.from, en).as_deref(), Some("tomorrow"));
        let q = palette_query(&c, "1am tokyo to ny").unwrap();
        assert_eq!(day_note(&c, q.ts, &q.to, &q.from, en).as_deref(), Some("yesterday"));
    }

    #[test]
    fn half_hour_offsets_and_same_time() {
        let c = ctx();
        let q = palette_query(&c, "9am india to sp").unwrap();
        assert_eq!(offset_text(&c, &q.from, q.ts, &q.to, Language::En), "+8:30");
        assert_eq!(offset_text(&c, &named("America/Sao_Paulo"), c.now, &Zone::Local, Language::En), "same time");
    }

    #[test]
    fn dst_is_applied_with_the_date() {
        // New York is UTC-4 in July and UTC-5 in November
        let c = ctx();
        let july = 1783944000; // 2026-07-13 12:00 UTC
        assert_eq!(c.offset(&named("America/New_York"), july), Some(-4 * 3600));
        assert_eq!(c.offset(&named("America/New_York"), c.now), Some(-5 * 3600));
        assert_eq!(c.offset(&named("Asia/Kolkata"), c.now), Some(19800));
        assert_eq!(c.offset(&named("No/Such_Zone"), c.now), None);
    }

    #[test]
    fn names_aliases_and_cities() {
        assert_eq!(zone("sp"), Some(named("America/Sao_Paulo")));
        assert_eq!(zone("Nova York"), Some(named("America/New_York")));
        assert_eq!(zone("纽约"), Some(named("America/New_York")));
        assert_eq!(zone("里斯本"), Some(named("Europe/Lisbon")));
        assert_eq!(zone("aqui"), Some(Zone::Local));
        // a city of the tz database, by its name or a 4 letter prefix
        assert_eq!(zone("denver"), Some(named("America/Denver")));
        assert_eq!(zone("aucklan"), Some(named("Pacific/Auckland")));
        assert_eq!(zone("x"), None);
        assert_eq!(zone("zzzzzz"), None);
        assert_eq!(city(&named("America/Sao_Paulo"), Language::En), "São Paulo");
        assert_eq!(city(&named("America/New_York"), Language::Pt), "Nova York");
        assert_eq!(city(&named("America/Denver"), Language::En), "Denver");
    }

    #[test]
    fn not_a_time_query() {
        let c = ctx();
        assert_eq!(palette_query(&c, "12*7.5"), None);
        assert_eq!(palette_query(&c, "5 km to mi"), None);
        assert_eq!(palette_query(&c, "firefox"), None);
        assert_eq!(palette_query(&c, "3pm"), None);
        assert_eq!(palette_query(&c, "3pm to nowhere"), None);
    }

    #[test]
    fn invalid_ids_are_rejected_before_any_process_is_started() {
        assert_eq!(named_offset("../etc/passwd", 0), None);
        assert_eq!(named_offset("a b", 0), None);
        assert_eq!(named_offset("", 0), None);
    }
}
