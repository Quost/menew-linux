//! Runs every vector of `docs/notes-spec.md` through the real notes code.
//! The expected values in that file were derived by hand from the Mac code (`../menew/Sources/Menew`).

use menew_core::markdown::{self, Block};
use menew_core::notes::{password_notes, secret_clear_secs, secret_value, Kind, Note, SecretField};
use menew_core::palette::{results, PaletteContext};
use menew_core::recents::Recents;
use menew_core::time_zones::TzCtx;
use menew_core::timers::TimerStore;
use menew_core::Language;

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
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/notes-spec.md")).unwrap()
}

fn rows(prefix: char, columns: usize) -> Vec<Vec<String>> {
    let rows: Vec<Vec<String>> = spec()
        .lines()
        .filter(|l| l.strip_prefix("| ").map(|r| r.starts_with(prefix) && r[1..].chars().take_while(|c| *c != ' ').all(|c| c.is_ascii_digit())).unwrap_or(false))
        .map(cells)
        .collect();
    assert!(!rows.is_empty(), "no {prefix} vectors in docs/notes-spec.md");
    for r in &rows {
        assert_eq!(r.len(), columns, "bad row ({columns} columns expected): {r:?}");
    }
    rows
}

/// `\n`, `(e)` (empty), `-` (nothing) and `REPEAT:a:80`; a cell in backticks is taken literally.
fn val(s: &str) -> String {
    if let Some(lit) = s.strip_prefix('`').and_then(|r| r.strip_suffix('`')) {
        return lit.to_string();
    }
    if s == "(e)" || s == "-" {
        return String::new();
    }
    if let Some(rest) = s.strip_prefix("REPEAT:") {
        let (ch, n) = rest.split_once(':').unwrap();
        return ch.repeat(n.parse().unwrap());
    }
    s.replace("\\n", "\n")
}

fn finish(name: &str, executed: usize, total: usize, failures: Vec<String>) {
    println!("notes-spec {name}: executed {executed} vectors of {total} in docs/notes-spec.md");
    assert_eq!(executed, total);
    assert!(failures.is_empty(), "{} vector(s) failed:\n{}", failures.len(), failures.join("\n"));
}

fn list(s: &str) -> Vec<String> {
    if s == "-" {
        vec![]
    } else {
        s.split('¶').map(val).collect()
    }
}

fn pairs(s: &str) -> Vec<SecretField> {
    if s == "-" {
        return vec![];
    }
    s.split('¶').map(val).map(|p| match p.split_once('=') {
        Some((k, v)) => SecretField::new(k, v),
        None => SecretField::new(&p, ""),
    }).collect()
}

fn note_of(text: &str, kind: &str) -> Note {
    let mut n = Note::new(&val(text));
    match kind {
        "secure" => n.secure = true,
        "passwords" => {
            n.secure = true;
            n.kind = Some(Kind::Passwords);
        }
        _ => {}
    }
    n
}

#[test]
fn a_title_preview_and_tags() {
    let rows = rows('A', 5);
    let mut failures = vec![];
    for r in &rows {
        let n = Note::new(&val(&r[1]));
        let (title, preview, tags) = (n.title(Language::En), n.preview(Language::En), n.tags().join(","));
        let want_tags = if r[4] == "-" { String::new() } else { r[4].clone() };
        if title != val(&r[2]) || preview != val(&r[3]) || tags != want_tags {
            failures.push(format!("{}: got ({title:?}, {preview:?}, {tags:?})", r[0]));
        }
    }
    finish("A", rows.len(), rows.len(), failures);
}

#[test]
fn b_secret_fields_parse() {
    let rows = rows('B', 5);
    let mut failures = vec![];
    for r in &rows {
        let (title, fields) = SecretField::parse(&val(&r[1]));
        let names: Vec<String> = fields.iter().map(|f| f.key.clone()).collect();
        let values: Vec<String> = fields.iter().map(|f| f.value.clone()).collect();
        if title != val(&r[2]) || names != list(&r[3]) || values != list(&r[4]) {
            failures.push(format!("{}: got ({title:?}, {names:?}, {values:?})", r[0]));
        }
    }
    finish("B", rows.len(), rows.len(), failures);
}

#[test]
fn c_secret_fields_format_and_round_trip() {
    let rows = rows('C', 4);
    let mut failures = vec![];
    for r in &rows {
        let fields = pairs(&r[2]);
        let text = SecretField::format(&val(&r[1]), &fields);
        if text != val(&r[3]) {
            failures.push(format!("{}: got {text:?}", r[0]));
        }
    }
    // testFormatRoundTrip of the Mac
    let fields = vec![SecretField::new("Usuário", "ana"), SecretField::new("Senha", "x y")];
    let (title, parsed) = SecretField::parse(&SecretField::format("Wi-Fi", &fields));
    if title != "Wi-Fi" || parsed != fields {
        failures.push(format!("round trip: got {title:?} {parsed:?}"));
    }
    finish("C", rows.len(), rows.len(), failures);
}

#[test]
fn d_picks_password_and_username_in_any_language() {
    let rows = rows('D', 4);
    let mut failures = vec![];
    for r in &rows {
        let fields = pairs(&r[1]);
        let pw = secret_value(&fields, false);
        let user = secret_value(&fields, true);
        let want = |s: &str| if s == "-" { None } else { Some(val(s)) };
        if pw != want(&r[2]) || user != want(&r[3]) {
            failures.push(format!("{}: got ({pw:?}, {user:?})", r[0]));
        }
    }
    finish("D", rows.len(), rows.len(), failures);
}

fn ser(b: &Block) -> String {
    match b {
        Block::Heading(l, t) => format!("H{l}:{t}"),
        Block::Task { done, text, line } => format!("T{}{line}:{text}", if *done { '+' } else { '-' }),
        Block::Bullet { text, indent } => format!("B{indent}:{text}"),
        Block::Numbered(n, t) => format!("N{n}:{t}"),
        Block::Quote(t) => format!("Q:{t}"),
        Block::Code(c) => format!("C:{c}"),
        Block::Rule => "R".into(),
        Block::Paragraph(t) => format!("P:{t}"),
        Block::Space => "S".into(),
    }
}

#[test]
fn e_markdown_blocks() {
    let rows = rows('E', 3);
    let mut failures = vec![];
    for r in &rows {
        let got = markdown::blocks(&val(&r[1])).iter().map(ser).collect::<Vec<_>>().join(" ¦ ");
        let want = if r[2] == "-" { String::new() } else { val(&r[2]) };
        if got != want {
            failures.push(format!("{}: got {got:?}\n      want {want:?}", r[0]));
        }
    }
    finish("E", rows.len(), rows.len(), failures);
}

#[test]
fn f_toggle_task() {
    let rows = rows('F', 4);
    let mut failures = vec![];
    for r in &rows {
        let got = markdown::toggle_task(&val(&r[1]), r[2].parse().unwrap());
        if got != val(&r[3]) {
            failures.push(format!("{}: got {got:?}", r[0]));
        }
    }
    finish("F", rows.len(), rows.len(), failures);
}

#[test]
fn g_inline_markdown() {
    let rows = rows('G', 3);
    let mut failures = vec![];
    for r in &rows {
        let got = markdown::inline_pango(&val(&r[1]));
        if got != val(&r[2]) {
            failures.push(format!("{}: got {got:?}", r[0]));
        }
    }
    finish("G", rows.len(), rows.len(), failures);
}

fn palette_notes(notes: &[Note], query: &str) -> Vec<menew_core::palette::PaletteItem> {
    let rec = Recents::default();
    let timers = TimerStore::default();
    let ctx = PaletteContext {
        lang: Language::En,
        apps: &[],
        recents: &rec,
        clipboard_text: None,
        tz: TzCtx { now: 1794657600, local: Some("America/Sao_Paulo".into()) },
        links: &[],
        world_clocks: &[],
        timers: &timers,
        focus_running: false,
        scripts: &[],
        autostart_on: false,
        clip: Default::default(),
        notes,
        extra: &[],
    };
    results(&ctx, query, &|_| vec![])
}

#[test]
fn h_notes_in_the_palette() {
    let rows = rows('H', 5);
    let mut failures = vec![];
    for r in &rows {
        let mut notes = vec![];
        for spec in r[1].split('¶') {
            let mut p = spec.splitn(3, ':');
            let (id, kind, text) = (p.next().unwrap(), p.next().unwrap(), p.next().unwrap());
            let mut n = note_of(text, kind);
            n.id = id.to_string();
            notes.push(n);
        }
        let out = palette_notes(&notes, &val(&r[2]));
        let found: Vec<String> = out.iter().filter(|i| i.id.starts_with("note.")).map(|i| i.id.clone()).collect();
        let sub = out.iter().find(|i| i.id.starts_with("note.")).and_then(|i| i.subtitle.clone()).unwrap_or_default();
        let want: Vec<String> = if r[3] == "-" { vec![] } else { r[3].split(',').map(String::from).collect() };
        if found != want || sub != val(&r[4]) {
            failures.push(format!("{}: got ({found:?}, {sub:?})", r[0]));
        }
        // a protected note never gives its content to the palette: only the title is in the item
        for it in out.iter().filter(|i| i.id.starts_with("note.")) {
            let n = notes.iter().find(|n| it.id == format!("note.{}", n.id)).unwrap();
            if n.secure && !it.keywords.is_empty() {
                failures.push(format!("{}: protected note has keywords {:?}", r[0], it.keywords));
            }
        }
    }
    finish("H", rows.len(), rows.len(), failures);
}

#[test]
fn h_new_note_actions_are_in_the_palette() {
    let out = palette_notes(&[], "new note");
    assert!(out.iter().any(|i| i.id == "action.newnote" && i.action.describe() == "new-note"), "{:?}", out.iter().map(|i| &i.id).collect::<Vec<_>>());
    let out = palette_notes(&[], "nova nota");
    assert!(out.iter().any(|i| i.id == "action.newnote"));
}

#[test]
fn i_clipboard_passwords_filter() {
    let rows = rows('I', 4);
    let mut failures = vec![];
    for r in &rows {
        let mut notes = vec![];
        for spec in r[1].split('¶') {
            let mut p = spec.splitn(4, ':');
            let (id, kind, text, upd) = (p.next().unwrap(), p.next().unwrap(), p.next().unwrap(), p.next().unwrap());
            let mut n = note_of(text, kind);
            n.id = id.to_string();
            n.updated = upd.parse().unwrap();
            notes.push(n);
        }
        let got: Vec<String> = password_notes(&notes, &val(&r[2]), Language::En).iter().map(|n| n.id.clone()).collect();
        let want: Vec<String> = if r[3] == "-" { vec![] } else { r[3].split(',').map(String::from).collect() };
        if got != want {
            failures.push(format!("{}: got {got:?}", r[0]));
        }
    }
    finish("I", rows.len(), rows.len(), failures);
}

#[test]
fn k_secret_clear_delay() {
    let rows = rows('K', 4);
    let mut failures = vec![];
    for r in &rows {
        let over = if r[2] == "-" { None } else { Some(r[2].as_str()) };
        let got = secret_clear_secs(r[1] == "yes", over);
        if got.to_string() != r[3] {
            failures.push(format!("{}: got {got}", r[0]));
        }
    }
    finish("K", rows.len(), rows.len(), failures);
}

#[test]
fn l_the_file_of_the_mac() {
    let rows = rows('L', 3);
    let mut failures = vec![];
    for r in &rows {
        let notes: Vec<Note> = serde_json::from_str(&r[1]).unwrap();
        let n = &notes[0];
        let kind = match n.kind {
            Some(Kind::Passwords) => "passwords",
            _ => "text",
        };
        let got = format!("{},{},{kind},{},{}", n.id, n.secure, n.created + menew_core::notes::SWIFT_EPOCH_OFFSET, n.updated + menew_core::notes::SWIFT_EPOCH_OFFSET);
        if got != r[2] {
            failures.push(format!("{}: got {got}", r[0]));
        }
        // writing back leaves `kind` out when it was not there
        let back = serde_json::to_string(&notes).unwrap();
        if !r[1].contains("\"kind\"") && back.contains("kind") {
            failures.push(format!("{}: wrote kind back: {back}", r[0]));
        }
        let again: Vec<Note> = serde_json::from_str(&back).unwrap();
        if again != notes {
            failures.push(format!("{}: does not round trip", r[0]));
        }
    }
    finish("L", rows.len(), rows.len(), failures);
}
