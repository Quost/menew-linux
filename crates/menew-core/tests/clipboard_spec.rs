//! Runs every vector of `docs/clipboard-spec.md` through the real clipboard code.
//! The expected values in that file were derived from the Mac code (`../menew/Sources/Menew`).

use chrono::NaiveDate;
use menew_core::clip_item::{file_paths, hex_color, row_title, ClipFilter};
use menew_core::clip_policy::{image_file_name, is_ignored_app, parse_uri_list, plan, should_capture, Candidate, Capture, PauseState};
use menew_core::clip_smart;
use menew_core::clipboard_db::{ClipKind, ClipboardDatabase, ClipboardEntry};
use menew_core::palette::{results, PaletteContext};
use menew_core::paste_queue::{PasteQueue, QueueEntry};
use menew_core::quick_links::defaults;
use menew_core::recents::Recents;
use menew_core::snippets::{self, Snippet};
use menew_core::time_zones::TzCtx;
use menew_core::timers::TimerStore;
use menew_core::Language;

fn unescape(s: &str) -> String {
    s.replace("\\n", "\n")
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

fn spec() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/clipboard-spec.md")).unwrap()
}

/// The rows whose id is `prefix` and digits (`A01`...).
fn rows(prefix: char, columns: usize) -> Vec<Vec<String>> {
    let rows: Vec<Vec<String>> = spec()
        .lines()
        .filter(|l| {
            l.strip_prefix("| ").map(|r| r.starts_with(prefix) && r[1..].chars().take_while(|c| *c != ' ').all(|c| c.is_ascii_digit())).unwrap_or(false)
        })
        .map(cells)
        .collect();
    assert!(!rows.is_empty(), "no {prefix} vectors in docs/clipboard-spec.md");
    for r in &rows {
        assert_eq!(r.len(), columns, "bad row ({columns} columns expected): {r:?}");
    }
    rows
}

fn lang_of(code: &str) -> Language {
    Language::from_code(code)
}

fn none(s: &str) -> String {
    if s == "-" {
        String::new()
    } else {
        unescape(s)
    }
}

fn finish(name: &str, executed: usize, total: usize, failures: Vec<String>) {
    println!("clipboard-spec {name}: executed {executed} vectors of {total} in docs/clipboard-spec.md");
    assert_eq!(executed, total);
    assert!(failures.is_empty(), "{} vector(s) failed:\n{}", failures.len(), failures.join("\n"));
}

fn expand_repeat(s: &str) -> String {
    match s.strip_prefix("REPEAT:") {
        Some(rest) => {
            let (ch, n) = rest.split_once(':').unwrap();
            ch.repeat(n.parse().unwrap())
        }
        None => unescape(s),
    }
}

#[test]
fn smart_actions_match_the_spec() {
    let rows = rows('A', 7);
    let mut failures = vec![];
    for r in &rows {
        let region = if r[2] == "-" { None } else { Some(r[2].as_str()) };
        let acts = clip_smart::actions(&expand_repeat(&r[3]), lang_of(&r[1]), region);
        let got = acts.iter().map(|a| format!("{}~{}~{}~{}", a.id, a.title, a.subtitle, a.run.describe())).collect::<Vec<_>>().join(" ;; ");
        if got != none(&r[4]) {
            failures.push(format!("{}: got\n  {got}\nexpected\n  {}", r[0], none(&r[4])));
        }
    }
    finish("smart actions", rows.len(), rows.len(), failures);
}

#[test]
fn snippet_variables_match_the_spec() {
    let rows = rows('B', 7);
    let now = NaiveDate::from_ymd_opt(2026, 11, 14).unwrap().and_hms_opt(15, 5, 0).unwrap();
    let mut failures = vec![];
    for r in &rows {
        let (text, back) = snippets::expand_with_cursor(&unescape(&r[3]), &now, &none(&r[2]), lang_of(&r[1]));
        if text != unescape(&r[4]) || back.to_string() != r[5] {
            failures.push(format!("{}: got ({text:?}, {back}) expected ({:?}, {})", r[0], unescape(&r[4]), r[5]));
        }
    }
    finish("snippet variables", rows.len(), rows.len(), failures);
}

#[test]
fn snippet_fields_match_the_spec() {
    let rows = rows('C', 6);
    let mut failures = vec![];
    for r in &rows {
        let text = unescape(&r[1]);
        let fields = snippets::fields_in(&text).join(",");
        let values: Vec<(String, String)> =
            if r[3] == "-" { vec![] } else { r[3].split(';').map(|kv| kv.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())).unwrap()).collect() };
        let applied = snippets::apply(&text, &values);
        if fields != none(&r[2]) || applied != unescape(&r[4]) {
            failures.push(format!("{}: fields {fields:?} applied {applied:?}", r[0]));
        }
    }
    finish("snippet fields", rows.len(), rows.len(), failures);
}

#[test]
fn entries_filters_and_titles_match_the_spec() {
    let rows = rows('D', 9);
    let mut failures = vec![];
    for r in &rows {
        let kind = match r[2].as_str() {
            "image" => ClipKind::Image,
            "file" => ClipKind::File,
            _ => ClipKind::Text,
        };
        let mut e = ClipboardEntry::text_entry(1, &unescape(&r[4]), r[3] == "yes", 0);
        e.kind = kind;
        let filters: Vec<&str> = ClipFilter::ALL.iter().filter(|f| **f != ClipFilter::Snippets && f.matches(&e)).map(|f| f.id()).collect();
        let title = row_title(&e, lang_of(&r[1]));
        let color = hex_color(&e).map(|(a, b, c)| format!("{a},{b},{c}")).unwrap_or_else(|| "-".into());
        if filters.join(",") != r[5] || title != unescape(&r[6]) || color != r[7] {
            failures.push(format!("{}: filters {filters:?} title {title:?} color {color:?}", r[0]));
        }
        if kind == ClipKind::File && file_paths(&e).is_empty() {
            failures.push(format!("{}: no paths", r[0]));
        }
    }
    finish("entries and filters", rows.len(), rows.len(), failures);
}

#[test]
fn capture_plans_match_the_spec() {
    let rows = rows('E', 4);
    let mut failures = vec![];
    for r in &rows {
        let types: Vec<String> = r[1].split(',').map(str::to_string).collect();
        let got = match plan(&types) {
            Err(_) => "skip".to_string(),
            Ok(p) if p.is_empty() => "-".to_string(),
            Ok(p) => p
                .iter()
                .map(|c| match c {
                    Candidate::Files(t) => format!("files:{t}"),
                    Candidate::Text(t) => format!("text:{t}"),
                    Candidate::Image(t) => format!("image:{t}"),
                })
                .collect::<Vec<_>>()
                .join(","),
        };
        if got != r[2] {
            failures.push(format!("{}: plan {got:?} expected {:?}", r[0], r[2]));
        }
    }
    finish("capture plans", rows.len(), rows.len(), failures);
}

#[test]
fn file_addresses_match_the_spec() {
    let rows = rows('F', 4);
    let mut failures = vec![];
    for r in &rows {
        let got = parse_uri_list(&unescape(&r[1])).join("\n");
        if got != none(&r[2]) {
            failures.push(format!("{}: {got:?} expected {:?}", r[0], none(&r[2])));
        }
    }
    finish("file addresses", rows.len(), rows.len(), failures);
}

#[test]
fn image_names_match_the_spec() {
    let rows = rows('G', 5);
    let mut failures = vec![];
    for r in &rows {
        let data: &[u8] = if r[1] == "(empty)" { b"" } else { r[1].as_bytes() };
        let got = image_file_name(data, &r[2]);
        if got != r[3] {
            failures.push(format!("{}: {got} expected {}", r[0], r[3]));
        }
    }
    finish("image names", rows.len(), rows.len(), failures);
}

fn pause_ms() {
    // `created_at` has millisecond resolution: let the order of the operations show
    std::thread::sleep(std::time::Duration::from_millis(3));
}

#[test]
fn history_operations_match_the_spec() {
    let rows = rows('H', 4);
    let mut failures = vec![];
    for r in &rows {
        let db = ClipboardDatabase::open_in_memory().unwrap();
        for op in r[1].split(';') {
            let (name, arg) = op.split_once(':').unwrap_or((op, ""));
            match name {
                "t" => {
                    db.save_text(arg).unwrap();
                }
                "f" => {
                    db.save_files(arg, None).unwrap();
                }
                "i" => {
                    db.save_image(arg, 1, 1, None).unwrap();
                }
                "pin" => {
                    let all = db.get_recent(10_000).unwrap();
                    let e = all.iter().find(|e| e.text_content == arg || e.image_file.as_deref() == Some(arg)).unwrap();
                    db.toggle_pinned(e.id).unwrap();
                }
                "clear" => {
                    db.clear_unpinned().unwrap();
                }
                "n" => {
                    for i in 0..arg.parse::<usize>().unwrap() {
                        db.save_text(&format!("n{i}")).unwrap();
                    }
                }
                "prune" => {
                    db.prune(arg.parse().unwrap(), 0).unwrap();
                }
                other => panic!("{}: unknown operation {other}", r[0]),
            }
            pause_ms();
        }
        let all = db.get_recent(10_000).unwrap();
        let got = if r[2].starts_with('#') {
            format!("#{}", all.len())
        } else {
            all.iter()
                .map(|e| {
                    let (k, v) = match e.kind {
                        ClipKind::Text => ("t", e.text_content.clone()),
                        ClipKind::File => ("f", e.text_content.clone()),
                        ClipKind::Image => ("i", e.image_file.clone().unwrap()),
                    };
                    format!("{k}:{v}{}", if e.is_pinned { "*" } else { "" })
                })
                .collect::<Vec<_>>()
                .join(",")
        };
        if got != r[2] {
            failures.push(format!("{}: {got} expected {}", r[0], r[2]));
        }
    }
    finish("history operations", rows.len(), rows.len(), failures);
}

#[test]
fn pause_matches_the_spec() {
    let rows = rows('I', 5);
    let mut failures = vec![];
    for r in &rows {
        let mut p = PauseState::default();
        for op in r[1].split(';') {
            if op == "resume" {
                p.resume();
            } else {
                let (m, at) = op.strip_prefix("pause:").unwrap().split_once('@').unwrap();
                p.pause(if m == "-" { None } else { Some(m.parse().unwrap()) }, at.parse().unwrap());
            }
        }
        let got = if p.is_paused(r[2].parse().unwrap()) { "yes" } else { "no" };
        if got != r[3] {
            failures.push(format!("{}: {got} expected {}", r[0], r[3]));
        }
    }
    finish("pause", rows.len(), rows.len(), failures);
}

#[test]
fn ignored_apps_match_the_spec() {
    let rows = rows('J', 5);
    let mut failures = vec![];
    for r in &rows {
        let list: Vec<String> = if r[1] == "-" { vec![] } else { r[1].split(',').map(str::to_string).collect() };
        let app = if r[2] == "-" { None } else { Some(r[2].as_str()) };
        let got = match should_capture(&PauseState::default(), 0.0, &list, app) {
            Capture::Yes => "yes",
            Capture::Ignored => "ignored",
            Capture::Paused => "paused",
        };
        if got != r[3] {
            failures.push(format!("{}: {got} expected {}", r[0], r[3]));
        }
        if let Some(a) = app {
            assert_eq!(is_ignored_app(a, &list), got == "ignored");
        }
    }
    finish("ignored apps", rows.len(), rows.len(), failures);
}

#[test]
fn paste_queue_matches_the_spec() {
    let rows = rows('K', 5);
    let mut failures = vec![];
    let entry = |name: &str| QueueEntry { id: name.bytes().next().unwrap() as i64, preview: name.to_string() };
    for r in &rows {
        let mut q = PasteQueue::default();
        let mut log: Vec<String> = vec![];
        for op in r[1].split(';') {
            let (name, arg) = op.split_once(':').unwrap_or((op, ""));
            match name {
                "start" => {
                    q.start();
                    log.push("on".into());
                }
                "stop" => {
                    q.stop();
                    log.push("off".into());
                }
                "cap" => match q.captured(entry(arg)) {
                    Some(n) => log.push(format!("+{arg}={n}")),
                    None => log.push(format!("-{arg}")),
                },
                "add" => {
                    let n = q.add(entry(arg));
                    log.push(format!("+{arg}={n}"));
                }
                "rm" => log.push(if q.remove(entry(arg).id) { "rm".into() } else { "no".into() }),
                "next" => match q.paste_next() {
                    Some(p) => log.push(format!(">{}({})", p.entry.preview, p.remaining)),
                    None => log.push("none".into()),
                },
                other => panic!("{}: unknown operation {other}", r[0]),
            }
        }
        let state = if q.is_on() { format!("on[{}]", q.items().iter().map(|e| e.preview.clone()).collect::<Vec<_>>().join(",")) } else { "off".to_string() };
        if log.join(",") != r[2] || state != r[3] {
            failures.push(format!("{}: log {} state {state} expected {} / {}", r[0], log.join(","), r[2], r[3]));
        }
    }
    finish("paste queue", rows.len(), rows.len(), failures);
}

#[test]
fn palette_items_match_the_spec() {
    let rows = rows('P', 8);
    let links = defaults();
    let timers = TimerStore::default();
    let snippet = vec![Snippet { id: "s1".into(), title: "Hello".into(), text: "Hi {input:Name}".into(), abbreviation: None }];
    let mut failures = vec![];
    for r in &rows {
        let rec = Recents::default();
        let ctx = PaletteContext {
            lang: lang_of(&r[1]),
            apps: &[],
            recents: &rec,
            clipboard_text: if r[2] == "-" { None } else { Some(r[2].clone()) },
            tz: TzCtx { now: 1794657600, local: Some("America/Sao_Paulo".into()) },
            links: &links,
            world_clocks: &[],
            timers: &timers,
            focus_running: false,
            scripts: &[],
            autostart_on: false,
            clip: menew_core::palette::ClipContext {
                region: None,
                snippets: &snippet,
                paused: r[3] == "paused",
                queue_on: r[3] == "queue",
                queue_len: if r[3] == "queue" { 2 } else { 0 },
            },
            extra: &[],
        };
        let input = if r[4] == "(empty)" { String::new() } else { r[4].clone() };
        let out = results(&ctx, &input, &|_| vec![]);
        let ids: Vec<&str> = out.iter().map(|i| i.id.as_str()).collect();
        let (want, open_ended) = match r[5].strip_suffix(",...") {
            Some(w) => (w, true),
            None => (r[5].as_str(), false),
        };
        let want: Vec<&str> = if want == "-" { vec![] } else { want.split(',').collect() };
        let ok_prefix = ids.len() >= want.len() && ids[..want.len()] == want[..];
        if !ok_prefix {
            failures.push(format!("{}: results {ids:?} do not start with {want:?}", r[0]));
        }
        let _ = open_ended;
        if r[6] != "-" && ids.contains(&r[6].as_str()) {
            failures.push(format!("{}: {} must not be there ({ids:?})", r[0], r[6]));
        }
    }
    finish("palette items", rows.len(), rows.len(), failures);
}

#[test]
fn every_vector_row_belongs_to_a_table_that_is_run() {
    let all = spec().lines().filter(|l| l.starts_with("| ") && l.chars().nth(2).map(|c| c.is_ascii_uppercase()).unwrap_or(false) && l.chars().nth(3).map(|c| c.is_ascii_digit()).unwrap_or(false)).count();
    let run: usize = "ABCDEFGHIJKP".chars().map(rows_count).sum();
    assert_eq!(all, run, "a table of docs/clipboard-spec.md is not executed by the tests");
    println!("clipboard-spec: {all} vectors in total");
}

fn rows_count(prefix: char) -> usize {
    spec().lines().filter(|l| l.strip_prefix("| ").map(|r| r.starts_with(prefix) && r[1..].chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)).unwrap_or(false)).count()
}
