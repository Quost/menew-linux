//! Runs every vector of `docs/screenshots-spec.md` through the real capture, QR and palette code.
//! The expected values in that file were derived by hand from the Mac code (`../menew/Sources/Menew`) and from
//! the QR standard (ISO/IEC 18004).

use menew_core::capture::{self, Mark, Point, Rgb, TextOutcome, Tool};
use menew_core::palette::{results, PaletteContext};
use menew_core::qr::{self, ScanOutcome};
use menew_core::recents::Recents;
use menew_core::strings;
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
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/screenshots-spec.md")).unwrap()
}

fn rows(prefix: char, columns: usize) -> Vec<Vec<String>> {
    let rows: Vec<Vec<String>> = spec()
        .lines()
        .filter(|l| l.strip_prefix("| ").map(|r| r.starts_with(prefix) && r[1..].chars().take_while(|c| *c != ' ').all(|c| c.is_ascii_digit())).unwrap_or(false))
        .map(cells)
        .collect();
    assert!(!rows.is_empty(), "no {prefix} vectors in docs/screenshots-spec.md");
    for r in &rows {
        assert_eq!(r.len(), columns, "bad row ({columns} columns expected): {r:?}");
    }
    rows
}

/// `\n`, `(e)` (empty), `-` (nothing) and `REPEAT:a:80`; a cell in backticks keeps its spaces and backslashes.
fn val(s: &str) -> String {
    if let Some(lit) = s.strip_prefix('`').and_then(|r| r.strip_suffix('`')) {
        return lit.replace("\\n", "\n");
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
    println!("screenshots-spec {name}: executed {executed} vectors of {total} in docs/screenshots-spec.md");
    assert_eq!(executed, total);
    assert!(failures.is_empty(), "{} vector(s) failed:\n{}", failures.len(), failures.join("\n"));
}

fn lang_of(code: &str) -> Language {
    Language::from_code(code)
}

fn pt(s: &str) -> Point {
    let (x, y) = s.split_once(',').unwrap();
    Point::new(x.parse().unwrap(), y.parse().unwrap())
}

fn num(s: &str) -> f64 {
    s.parse().unwrap()
}

fn close(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() <= eps
}

fn tool_of(s: &str) -> Tool {
    match s {
        "pen" => Tool::Pen,
        "arrow" => Tool::Arrow,
        "rect" => Tool::Rect,
        "text" => Tool::Text,
        "blur" => Tool::Blur,
        other => panic!("unknown tool {other}"),
    }
}

fn tool_name(t: Tool) -> &'static str {
    match t {
        Tool::Pen => "pen",
        Tool::Arrow => "arrow",
        Tool::Rect => "rect",
        Tool::Text => "text",
        Tool::Blur => "blur",
    }
}

#[test]
fn a_file_names() {
    let rows = rows('A', 6);
    let mut failures = vec![];
    for r in &rows {
        let (date, time) = r[2].split_once(' ').unwrap();
        let d: Vec<u32> = date.split('-').map(|x| x.parse().unwrap()).collect();
        let t: Vec<u32> = time.split(':').map(|x| x.parse().unwrap()).collect();
        let lang = lang_of(&r[1]);
        let saved = capture::screenshot_file_name(lang, d[0] as i32, d[1], d[2], t[0], t[1], t[2]);
        let dragged = capture::drag_file_name(lang);
        if saved != r[3] || dragged != r[4] {
            failures.push(format!("{}: got ({saved:?}, {dragged:?})", r[0]));
        }
    }
    finish("A", rows.len(), rows.len(), failures);
}

#[test]
fn b_marks_kept() {
    let rows = rows('B', 7);
    let mut failures = vec![];
    for r in &rows {
        let mut m = Mark::new(tool_of(&r[1]), pt(&r[2]), pt(&r[3]), capture::DEFAULT_COLOR);
        let n: usize = r[4].parse().unwrap();
        m.points = (0..n).map(|i| Point::new(i as f64, i as f64)).collect();
        let kept = if m.is_kept() { "yes" } else { "no" };
        let (x, y, w, h) = m.rect();
        let want: Vec<f64> = r[6].split(',').map(num).collect();
        let ok = kept == r[5] && close(x, want[0], 1e-9) && close(y, want[1], 1e-9) && close(w, want[2], 1e-9) && close(h, want[3], 1e-9);
        if !ok {
            failures.push(format!("{}: got ({kept}, {:?})", r[0], (x, y, w, h)));
        }
    }
    finish("B", rows.len(), rows.len(), failures);
}

#[test]
fn c_sizes() {
    let rows = rows('C', 8);
    let mut failures = vec![];
    for r in &rows {
        let (w, h) = (num(&r[1]), num(&r[2]));
        let lw = capture::line_width(w, h);
        let ok = close(lw, num(&r[3]), 1e-3)
            && close(capture::font_size(w, h), num(&r[4]), 1e-3)
            && close(capture::arrow_head_length(lw), num(&r[5]), 1e-3)
            && close(capture::rect_corner_radius(lw), num(&r[6]), 1e-3)
            && capture::pixelate_scale(w as usize) == r[7].parse::<usize>().unwrap();
        if !ok {
            failures.push(format!("{}: lw {lw}, font {}, block {}", r[0], capture::font_size(w, h), capture::pixelate_scale(w as usize)));
        }
    }
    finish("C", rows.len(), rows.len(), failures);
}

#[test]
fn d_arrow_head() {
    let rows = rows('D', 6);
    let mut failures = vec![];
    for r in &rows {
        let got = capture::arrow_head_points(pt(&r[1]), pt(&r[2]), num(&r[3]));
        let want = [pt(&r[4]), pt(&r[5])];
        for i in 0..2 {
            if !close(got[i].x, want[i].x, 0.01) || !close(got[i].y, want[i].y, 0.01) {
                failures.push(format!("{}: stroke {i} ends at {:?}, wanted {:?}", r[0], got[i], want[i]));
            }
        }
    }
    finish("D", rows.len(), rows.len(), failures);
}

#[test]
fn e_colors_outline_and_text() {
    let rows = rows('E', 4);
    let mut failures = vec![];
    let rgb = |c: Rgb| format!("{},{},{}", c.0, c.1, c.2);
    for r in &rows {
        let got = match r[1].as_str() {
            "colors" => capture::COLORS.iter().map(|c| rgb(*c)).collect::<Vec<_>>().join(" "),
            "default color" => rgb(capture::DEFAULT_COLOR),
            "default tool" => tool_name(capture::DEFAULT_TOOL).to_string(),
            "tools" => capture::TOOLS.iter().map(|t| tool_name(*t)).collect::<Vec<_>>().join(" "),
            "outline" => {
                let c: Vec<u8> = r[2].split(',').map(|x| x.parse().unwrap()).collect();
                let (o, a) = capture::text_outline(Rgb(c[0], c[1], c[2]));
                format!("{} {}", rgb(o), a)
            }
            "outline offset" => format!("{}", capture::text_outline_offset(num(&r[2]))),
            "typed text" => capture::clean_typed_text(&val(&r[2])).unwrap_or_default(),
            other => panic!("unknown E row {other}"),
        };
        if got != val(&r[3]) && !(r[3] == "-" && got.is_empty()) {
            failures.push(format!("{}: got {got:?}", r[0]));
        }
    }
    finish("E", rows.len(), rows.len(), failures);
}

const W: usize = 120;
const H: usize = 90;

fn test_image() -> Vec<u8> {
    let mut d = Vec::with_capacity(W * H * 4);
    for y in 0..H {
        for x in 0..W {
            d.extend_from_slice(&[(x * 2 % 256) as u8, (y * 3 % 256) as u8, ((x + y) % 256) as u8, 255]);
        }
    }
    d
}

/// The average of every 10 x 10 block, written out here so that it does not depend on `pixelate`.
fn block_average(img: &[u8], x: usize, y: usize, block: usize) -> [u8; 4] {
    let (bx, by) = (x / block * block, y / block * block);
    let mut sum = [0u32; 4];
    let mut n = 0u32;
    for yy in by..(by + block).min(H) {
        for xx in bx..(bx + block).min(W) {
            for c in 0..4 {
                sum[c] += img[(yy * W + xx) * 4 + c] as u32;
            }
            n += 1;
        }
    }
    sum.map(|s| ((s + n / 2) / n) as u8)
}

fn boxes(s: &str) -> Vec<(usize, usize, usize, usize)> {
    match s {
        "-" => vec![],
        "all" => vec![(0, 0, W, H)],
        _ => s
            .split(';')
            .map(|b| {
                let v: Vec<usize> = b.split(',').map(|x| x.parse().unwrap()).collect();
                (v[0], v[1], v[2], v[3])
            })
            .collect(),
    }
}

#[test]
fn f_blur() {
    let rows = rows('F', 4);
    let img = test_image();
    let pix = capture::pixelate(&img, W, H);
    let mut failures = vec![];
    for r in &rows {
        let marks: Vec<Mark> = r[1]
            .split(';')
            .map(|m| {
                let v: Vec<f64> = m.split(',').map(num).collect();
                Mark::new(Tool::Blur, Point::new(v[0], v[1]), Point::new(v[2], v[3]), capture::DEFAULT_COLOR)
            })
            .collect();
        let covered = boxes(&r[2]);
        // the pixels a mark covers
        let found: Vec<_> = marks.iter().filter_map(|m| capture::blur_pixels(m, W, H)).collect();
        if found != covered {
            failures.push(format!("{}: blur_pixels gave {found:?}, wanted {covered:?}", r[0]));
        }
        let out = capture::apply_blur(&img, &pix, W, H, &marks);
        let (mut inside, mut changed) = (0usize, 0usize);
        for y in 0..H {
            for x in 0..W {
                let i = (y * W + x) * 4;
                let is_in = covered.iter().any(|&(x0, y0, x1, y1)| x >= x0 && x < x1 && y >= y0 && y < y1);
                if is_in {
                    inside += 1;
                    if out[i..i + 4] != img[i..i + 4] {
                        changed += 1;
                    }
                    if out[i..i + 4] != block_average(&img, x, y, 10) {
                        failures.push(format!("{}: pixel ({x},{y}) is not the average of its block", r[0]));
                        break;
                    }
                } else if out[i..i + 4] != img[i..i + 4] {
                    failures.push(format!("{}: pixel ({x},{y}) outside the boxes changed", r[0]));
                    break;
                }
            }
        }
        if inside > 0 && changed * 10 < inside * 9 {
            failures.push(format!("{}: only {changed} of {inside} pixels inside changed", r[0]));
        }
    }
    finish("F", rows.len(), rows.len(), failures);
}

#[test]
fn g_blur_rules() {
    let rows = rows('G', 5);
    let mut failures = vec![];
    for r in &rows {
        let ok = match r[0].as_str() {
            "G01" => {
                let (w, h) = (1800usize, 20usize);
                let mut d = Vec::with_capacity(w * h * 4);
                for _y in 0..h {
                    for x in 0..w {
                        let v = (x * 7 % 251) as u8;
                        d.extend_from_slice(&[v, v, v, 255]);
                    }
                }
                let p = capture::pixelate(&d, w, h);
                let px = |x: usize| p[x * 4];
                (0..20).all(|x| px(x) == px(0)) && px(20) != px(19)
            }
            "G02" => {
                let img = test_image();
                let once = capture::pixelate(&img, W, H);
                capture::pixelate(&once, W, H) == once
            }
            "G03" | "G04" => {
                let mut d = vec![0u8; 10 * 10 * 4];
                let n = if r[0] == "G03" { 1 } else { 50 };
                for i in 0..n {
                    d[i * 4..i * 4 + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
                let want: u8 = r[3].parse().unwrap();
                capture::pixelate(&d, 10, 10)[0] == want
            }
            other => panic!("unknown G row {other}"),
        };
        if !ok {
            failures.push(format!("{}: {}", r[0], r[1]));
        }
    }
    finish("G", rows.len(), rows.len(), failures);
}

#[test]
fn h_wifi_payload() {
    let rows = rows('H', 4);
    let mut failures = vec![];
    for r in &rows {
        let got = qr::wifi_payload(&val(&r[1]), &val(&r[2]));
        if got != val(&r[3]) {
            failures.push(format!("{}: got {got:?}", r[0]));
        }
    }
    finish("H", rows.len(), rows.len(), failures);
}

#[test]
fn i_window_payload() {
    let rows = rows('I', 6);
    let mut failures = vec![];
    for r in &rows {
        let got = qr::window_payload(r[1] == "wifi", &val(&r[2]), &val(&r[3]), &val(&r[4]));
        if got != val(&r[5]) {
            failures.push(format!("{}: got {got:?}", r[0]));
        }
    }
    finish("I", rows.len(), rows.len(), failures);
}

#[test]
fn j_codes_round_trip_and_sizes() {
    let rows = rows('J', 5);
    let mut failures = vec![];
    for r in &rows {
        let text = val(&r[1]);
        let m = qr::encode(&text).unwrap();
        let scale = qr::scale_for(&m, 1024);
        let (w, h, px) = qr::render_gray(&m, scale);
        let back = qr::decode_gray(w, h, &px);
        let ok = m.size == r[2].parse::<usize>().unwrap() && scale == r[3].parse::<usize>().unwrap() && w == r[4].parse::<usize>().unwrap() && h == w && back == vec![text.clone()];
        if !ok {
            failures.push(format!("{}: {} modules, scale {scale}, side {w}, read back {:?}", r[0], m.size, back.iter().map(|b| b.len()).collect::<Vec<_>>()));
        }
        // the same code as a 4-bytes-per-pixel image (what the screen capture gives)
        let rgba: Vec<u8> = px.iter().flat_map(|v| [*v, *v, *v, 255]).collect();
        if qr::decode_bgra(w, h, &rgba) != vec![text] {
            failures.push(format!("{}: not read back from a 4-byte image", r[0]));
        }
    }
    finish("J", rows.len(), rows.len(), failures);
}

#[test]
fn k_scan_outcome() {
    let rows = rows('K', 4);
    let mut failures = vec![];
    for r in &rows {
        let found: Vec<String> = if r[1] == "(e)" { vec![] } else { val(&r[1]).split('¶').map(String::from).collect() };
        let outcome = qr::scan_outcome(&found);
        let (kind, text) = match &outcome {
            ScanOutcome::Nothing => ("none", None),
            ScanOutcome::Link(t) => ("link", Some(t.clone())),
            ScanOutcome::Content(t) => ("content", Some(t.clone())),
        };
        let want = if r[3] == "-" { None } else { Some(val(&r[3])) };
        if kind != r[2] || text != want || outcome.clipboard_text().map(String::from) != want {
            failures.push(format!("{}: got ({kind}, {text:?})", r[0]));
        }
    }
    finish("K", rows.len(), rows.len(), failures);
}

/// The scan reads and copies. Nothing in the code of the window may open an address or run a program.
#[test]
fn the_scan_code_never_opens_anything() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../menew-ui/src/screen.rs")).expect("crates/menew-ui/src/screen.rs");
    // the comments may talk about it
    let code: String = src.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
    for forbidden in ["launch_default_for_uri", "open_uri", "xdg-open", "Command::new", "show_uri", "UriLauncher", "OpenURI"] {
        assert!(!code.contains(forbidden), "screen.rs mentions {forbidden}");
    }
}

#[test]
fn l_text_outcome() {
    let rows = rows('L', 5);
    let mut failures = vec![];
    for r in &rows {
        let got = capture::text_outcome(&val(&r[1]));
        let ok = match (&got, r[2].as_str()) {
            (TextOutcome::Nothing, "none") => true,
            (TextOutcome::Copy { text, lines }, "copy") => *text == val(&r[3]) && *lines == r[4].parse::<usize>().unwrap(),
            _ => false,
        };
        if !ok {
            failures.push(format!("{}: got {got:?}", r[0]));
        }
    }
    finish("L", rows.len(), rows.len(), failures);
}

fn palette_find(lang: Language, query: &str, id: &str) -> Option<menew_core::palette::PaletteItem> {
    let rec = Recents::default();
    let timers = TimerStore::default();
    let ctx = PaletteContext {
        lang,
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
        notes: &[],
    };
    results(&ctx, query, &|_| vec![]).into_iter().find(|i| i.id == id)
}

#[test]
fn m_palette() {
    let rows = rows('M', 8);
    let mut failures = vec![];
    for r in &rows {
        match palette_find(lang_of(&r[1]), &val(&r[2]), &r[3]) {
            None => failures.push(format!("{}: {} not found for {:?}", r[0], r[3], r[2])),
            Some(i) => {
                let sub = i.subtitle.clone().unwrap_or_default();
                let sc = i.shortcut.clone().unwrap_or_default();
                if i.title != r[4] || sub != val(&r[5]) || i.action.describe() != r[6] || sc != val(&r[7]) {
                    failures.push(format!("{}: got ({:?}, {sub:?}, {}, {sc:?})", r[0], i.title, i.action.describe()));
                }
            }
        }
    }
    finish("M", rows.len(), rows.len(), failures);
}

#[test]
fn n_every_new_string_is_in_the_five_languages() {
    let rows = rows('N', 5);
    let mut failures = vec![];
    for r in &rows {
        let key = &r[1];
        let (en, pt) = (strings::tr(key, Language::En, &[]), strings::tr(key, Language::Pt, &[]));
        if en != r[2] || pt != r[3] {
            failures.push(format!("{}: got ({en:?}, {pt:?})", r[0]));
        }
        for lang in [Language::En, Language::Pt, Language::De, Language::Es, Language::Zh] {
            if strings::lookup(key, lang).map(str::is_empty).unwrap_or(true) {
                failures.push(format!("{}: {key} has no text in {lang:?}", r[0]));
            }
        }
    }
    // every key of the screen feature has the five columns (the table is typed `[&str; 5]`) and no empty one
    let own: Vec<&str> = strings::keys().filter(|k| k.starts_with("cap_") || k.starts_with("qr_") || k.starts_with("act_qr") || k.starts_with("act_capture") || *k == "act_ocr" || *k == "smart_qr" || k.starts_with("shortcut_capture")).collect();
    assert!(own.len() >= 38, "found only {} keys of the screen feature", own.len());
    for k in own {
        for lang in [Language::En, Language::Pt, Language::De, Language::Es, Language::Zh] {
            if strings::lookup(k, lang).map(str::is_empty).unwrap_or(true) {
                failures.push(format!("{k} is empty in {lang:?}"));
            }
        }
    }
    finish("N", rows.len(), rows.len(), failures);
}

/// A real read of an image made here with a known text (needs `tesseract` and ImageMagick; skipped, loudly, without).
#[test]
fn ocr_reads_a_generated_image() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("text.png");
    let made = std::process::Command::new("convert").args(["-background", "white", "-fill", "black", "-pointsize", "48", "label:MENEW CAPTURE"]).arg(&png).status();
    if !matches!(made, Ok(s) if s.success()) || !menew_core::ocr::available() {
        println!("SKIP ocr_reads_a_generated_image: needs ImageMagick (convert) and tesseract");
        return;
    }
    let text = menew_core::ocr::recognize(&png).unwrap();
    println!("OCR of the generated image: {text:?}");
    assert!(text.contains("MENEW"), "got {text:?}");
    assert_eq!(capture::text_outcome(&text), TextOutcome::Copy { text: text.trim().to_string(), lines: 1 });
}

/// `ScreenCapture.save` opens the panel in the Desktop folder: here the Desktop folder of the user (XDG), else home.
#[test]
fn default_save_dir_is_the_desktop_folder() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join("Desktop")).unwrap();
    std::fs::create_dir_all(home.path().join(".config")).unwrap();
    std::fs::write(home.path().join(".config/user-dirs.dirs"), "XDG_DESKTOP_DIR=\"$HOME/Desktop\"\n").unwrap();
    std::env::set_var("HOME", home.path());
    std::env::set_var("XDG_CONFIG_HOME", home.path().join(".config"));
    assert_eq!(capture::default_save_dir(), home.path().join("Desktop"));
    std::fs::remove_dir_all(home.path().join("Desktop")).unwrap();
    assert_eq!(capture::default_save_dir(), home.path());
    println!("default_save_dir: the Desktop folder when it exists, the home folder otherwise");
}
