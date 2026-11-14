//! Runs every vector of `docs/palette-spec.md` through the real palette engine.
//! The expected values in that file were derived from the Mac code (`../menew/Sources/Menew`).

use menew_core::desktop_entry::DesktopApp;
use menew_core::palette::{results, PaletteContext};
use menew_core::quick_links::{defaults, QuickLink};
use menew_core::recents::Recents;
use menew_core::script_commands::Script;
use menew_core::time_zones::{TzCtx, Zone};
use menew_core::timers::TimerStore;
use menew_core::Language;
use std::path::PathBuf;

fn app(id: &str, name: &str) -> DesktopApp {
    DesktopApp {
        id: id.into(),
        name: name.into(),
        generic_name: None,
        comment: None,
        exec: id.into(),
        icon: None,
        categories: vec![],
        keywords: vec![],
        terminal: false,
        file_path: PathBuf::from(format!("/usr/share/applications/{id}.desktop")),
    }
}

fn unescape(s: &str) -> String {
    s.replace("\\n", "\n").replace("\\u00a0", "\u{a0}")
}

/// Cells of a table row: split on `|`, strip one space on each side.
fn cells(line: &str) -> Vec<String> {
    let inner = line.trim_start().strip_prefix('|').unwrap().trim_end().strip_suffix('|').unwrap();
    inner
        .split('|')
        .map(|c| {
            let c = c.strip_prefix(' ').unwrap_or(c);
            c.strip_suffix(' ').unwrap_or(c).to_string()
        })
        .collect()
}

fn lang_of(code: &str) -> Language {
    Language::from_code(code)
}

#[test]
fn every_vector_of_the_spec_matches() {
    // dates inside a JWT are shown in the process zone
    std::env::set_var("TZ", "UTC");

    let spec = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/palette-spec.md")).unwrap();
    let rows: Vec<Vec<String>> = spec.lines().filter(|l| l.starts_with("| V")).map(cells).collect();
    assert!(!rows.is_empty(), "no vectors found in docs/palette-spec.md");

    let apps = vec![app("firefox", "Firefox"), app("teams", "Microsoft Teams")];
    let mut links = defaults();
    links.push(QuickLink::new("jira", "Jira", "https://company.atlassian.net/browse/{q}"));
    let timers = TimerStore::default();
    let scripts: Vec<Script> = vec![];
    let clocks: Vec<Zone> = vec![];

    let mut executed = 0;
    let mut failures = vec![];
    for r in &rows {
        assert_eq!(r.len(), 10, "bad row (10 columns expected): {r:?}");
        let (id, lang, clip, recents, input, item, title, subtitle, action) =
            (&r[0], lang_of(&r[1]), &r[2], &r[3], &r[4], &r[5], &r[6], &r[7], &r[8]);
        let input = if input == "(empty)" { String::new() } else { unescape(input) };
        let mut rec = Recents::default();
        if recents != "-" {
            for id in recents.split(',').rev() {
                rec.remember(id.trim());
            }
        }
        let ctx = PaletteContext {
            lang,
            apps: &apps,
            recents: &rec,
            clipboard_text: if clip == "-" { None } else { Some(unescape(clip)) },
            tz: TzCtx { now: 1794657600, local: Some("America/Sao_Paulo".into()) },
            links: &links,
            world_clocks: &clocks,
            timers: &timers,
            focus_running: false,
            scripts: &scripts,
            autostart_on: false,
        };
        let out = results(&ctx, &input, &|_| vec![]);
        executed += 1;
        let failed_before = failures.len();
        let ids: Vec<&str> = out.iter().map(|i| i.id.as_str()).collect();

        if let Some(absent) = item.strip_prefix('!') {
            if ids.contains(&absent) {
                failures.push(format!("{id}: item {absent} must not be there (results: {ids:?})"));
            }
            println!("{id} {}", if failures.len() == failed_before { "ok" } else { "FAILED" });
            continue;
        }
        let found = if let Some(n) = item.strip_prefix('@') {
            out.get(n.parse::<usize>().unwrap())
        } else {
            out.iter().find(|i| &i.id == item)
        };
        let Some(found) = found else {
            failures.push(format!("{id}: item {item} not found (results: {ids:?})"));
            println!("{id} FAILED");
            continue;
        };
        let t = unescape(title);
        if t != "*" && found.title != t {
            failures.push(format!("{id}: title {:?} != expected {:?}", found.title, t));
        }
        let s = unescape(subtitle);
        match (s.as_str(), &found.subtitle) {
            ("*", _) => {}
            ("(none)", None) => {}
            ("(none)", Some(got)) => failures.push(format!("{id}: subtitle {got:?} but none expected")),
            (exp, Some(got)) if got == exp => {}
            (exp, got) => failures.push(format!("{id}: subtitle {got:?} != expected {exp:?}")),
        }
        let a = unescape(action);
        if a != "*" && found.action.describe() != a {
            failures.push(format!("{id}: action {:?} != expected {:?}", found.action.describe(), a));
        }
        println!("{id} {}", if failures.len() == failed_before { "ok" } else { "FAILED" });
    }
    println!("palette-spec: executed {executed} vectors of {} in docs/palette-spec.md", rows.len());
    assert_eq!(executed, rows.len());
    assert!(failures.is_empty(), "{} vector(s) failed:\n{}", failures.len(), failures.join("\n"));
}
