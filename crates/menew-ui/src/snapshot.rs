//! `menew --snapshot <dir>`: renders the palette screens to PNG (light and dark) and exits. The port of
//! `Snapshot.swift`. It uses **fake data only**: an in-memory clipboard database, demo snippets, a fixed
//! list of apps and a fixed copied text. It never reads the user's clipboard, apps, scripts, notes,
//! recents or config, and never writes outside the folder it was given (and its own temporary images).

use crate::palette::PaletteWindow;
use crate::{PaletteState, SharedState};
use gtk4::cairo;
use gtk4::gio::prelude::ApplicationExt;
use gtk4::glib;
use libadwaita as adw;
use menew_core::clip_item::ClipFilter;
use menew_core::focus::FocusTimer;
use menew_core::paste_queue::PasteQueue;
use menew_core::quick_links;
use menew_core::recents::Recents;
use menew_core::snippets::{Snippet, SnippetStore};
use menew_core::timers::TimerStore;
use menew_core::{ClipboardDatabase, DesktopApp, Language};
use std::cell::{Cell, RefCell};
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

/// How long a screen is given to lay out before it is drawn.
const SETTLE_MS: u64 = 700;

/// The screens, in the order they are drawn. Names are the file names (without `-light`/`-dark.png`).
/// Add the "notes" screen here once the notes are merged (Phase 6).
pub const SCREENS: &[&str] = &[
    "palette",
    "palette-apps",
    "palette-conversion",
    "palette-emoji",
    "palette-transform",
    "clipboard",
    "clipboard-images",
    "clipboard-snippets",
    "capture-editor",
    "qr-code",
    "qr-wifi",
];

/// The screens that are a window of their own (the annotation editor and the QR window), not the palette.
fn is_window_screen(screen: &str) -> bool {
    matches!(screen, "capture-editor" | "qr-code" | "qr-wifi")
}

const DEMO_COPIED_TEXT: &str = "Hello Palette World";

fn pick(lang: Language, v: [&'static str; 5]) -> &'static str {
    v[match lang {
        Language::En => 0,
        Language::Pt => 1,
        Language::De => 2,
        Language::Es => 3,
        Language::Zh => 4,
    }]
}

/// Fixed apps: never the `.desktop` files of the machine.
pub fn demo_apps(lang: Language) -> Vec<DesktopApp> {
    let apps: [(&str, [&'static str; 5], &str, &str); 10] = [
        ("files", ["Files", "Arquivos", "Dateien", "Archivos", "文件"], "system-file-manager", "FileManager"),
        ("terminal", ["Terminal", "Terminal", "Terminal", "Terminal", "终端"], "utilities-terminal", "TerminalEmulator"),
        ("browser", ["Web Browser", "Navegador", "Webbrowser", "Navegador web", "网页浏览器"], "web-browser", "WebBrowser"),
        ("editor", ["Text Editor", "Editor de texto", "Texteditor", "Editor de texto", "文本编辑器"], "accessories-text-editor", "TextEditor"),
        ("calculator", ["Calculator", "Calculadora", "Rechner", "Calculadora", "计算器"], "accessories-calculator", "Calculator"),
        ("settings", ["Settings", "Configurações", "Einstellungen", "Ajustes", "设置"], "preferences-system", "Settings"),
        ("calendar", ["Calendar", "Calendário", "Kalender", "Calendario", "日历"], "x-office-calendar", "Calendar"),
        ("music", ["Music", "Música", "Musik", "Música", "音乐"], "multimedia-audio-player", "Audio"),
        ("photos", ["Photos", "Fotos", "Fotos", "Fotos", "照片"], "image-x-generic", "Graphics"),
        ("mail", ["Mail", "E-mail", "E-Mail", "Correo", "邮件"], "internet-mail", "Email"),
    ];
    apps.iter()
        .map(|(id, names, icon, cat)| DesktopApp {
            id: format!("demo-{id}.desktop"),
            name: pick(lang, *names).to_string(),
            generic_name: None,
            comment: None,
            exec: format!("demo-{id}"),
            icon: Some((*icon).to_string()),
            categories: vec![(*cat).to_string()],
            keywords: vec![],
            terminal: false,
            file_path: PathBuf::from(format!("/nonexistent/demo-{id}.desktop")),
        })
        .collect()
}

pub fn demo_state(lang: Language) -> SharedState {
    let snippets = vec![
        Snippet::new(pick(lang, ["Today's date", "Data de hoje", "Heutiges Datum", "Fecha de hoy", "今天的日期"]), "{date}"),
        Snippet::new(
            pick(lang, ["Meeting reply", "Resposta de reunião", "Antwort auf Termin", "Respuesta de reunión", "会议回复"]),
            pick(
                lang,
                [
                    "Hi {input:Name}, thanks for the invite. I can join on {tomorrow}.",
                    "Oi {input:Name}, obrigado pelo convite. Posso participar {tomorrow}.",
                    "Hallo {input:Name}, danke für die Einladung. Ich kann {tomorrow} teilnehmen.",
                    "Hola {input:Name}, gracias por la invitación. Puedo asistir {tomorrow}.",
                    "你好 {input:Name}，谢谢邀请。我可以在 {tomorrow} 参加。",
                ],
            ),
        ),
        Snippet::new(
            pick(lang, ["Email signature", "Assinatura de e-mail", "E-Mail-Signatur", "Firma de correo", "邮件签名"]),
            "Ana Demo\nhello@example.com",
        ),
    ];
    Rc::new(RefCell::new(PaletteState {
        recents: Recents::default(),
        timers: TimerStore::default(),
        focus: FocusTimer::default(),
        links: quick_links::defaults(),
        clocks: vec![],
        snippets: SnippetStore::in_memory(snippets),
        queue: PasteQueue::default(),
    }))
}

/// A picture for the image entries, drawn here (no file of the user is used).
fn draw_image(path: &Path, w: i32, h: i32, hue: [(f64, f64, f64); 2]) -> Result<(), String> {
    let e = |x: cairo::Error| x.to_string();
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h).map_err(e)?;
    {
        let cr = cairo::Context::new(&surface).map_err(e)?;
        let g = cairo::LinearGradient::new(0.0, 0.0, w as f64, h as f64);
        g.add_color_stop_rgb(0.0, hue[0].0, hue[0].1, hue[0].2);
        g.add_color_stop_rgb(1.0, hue[1].0, hue[1].1, hue[1].2);
        cr.set_source(&g).map_err(e)?;
        cr.paint().map_err(e)?;
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.25);
        cr.arc(w as f64 * 0.72, h as f64 * 0.38, h as f64 * 0.22, 0.0, std::f64::consts::TAU);
        cr.fill().map_err(e)?;
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.18);
        cr.rectangle(w as f64 * 0.1, h as f64 * 0.62, w as f64 * 0.55, h as f64 * 0.14);
        cr.fill().map_err(e)?;
        cr.rectangle(w as f64 * 0.1, h as f64 * 0.8, w as f64 * 0.35, h as f64 * 0.08);
        cr.fill().map_err(e)?;
    }
    let mut surface = surface;
    surface.flush();
    let stride = surface.stride() as usize;
    let bytes = {
        let data = surface.data().map_err(|x| x.to_string())?;
        glib::Bytes::from(&data[..])
    };
    // cairo ARGB32 is premultiplied BGRA on little-endian machines
    let texture = gtk4::gdk::MemoryTexture::new(w, h, gtk4::gdk::MemoryFormat::B8g8r8a8Premultiplied, &bytes, stride);
    gtk4::gdk::prelude::TextureExt::save_to_png(&texture, path).map_err(|x| x.to_string())
}

fn s<E: Display>(r: Result<Option<i64>, E>) -> Result<Option<i64>, String> {
    r.map_err(|e| e.to_string())
}

/// The fake clipboard history, in memory, with its images in `images_dir`.
pub fn demo_db(lang: Language, images_dir: &Path) -> Result<ClipboardDatabase, String> {
    std::fs::create_dir_all(images_dir).map_err(|e| e.to_string())?;
    let db = ClipboardDatabase::open_in_memory().map_err(|e| e.to_string())?.with_images_dir(images_dir.to_path_buf());
    draw_image(&images_dir.join("demo-a.png"), 640, 400, [(0.36, 0.30, 0.90), (0.95, 0.45, 0.60)])?;
    draw_image(&images_dir.join("demo-b.png"), 640, 400, [(0.10, 0.55, 0.60), (0.95, 0.80, 0.30)])?;
    let now = chrono::Utc::now().timestamp_millis();
    let min = 60_000_i64;
    let meeting = pick(
        lang,
        [
            "Meeting notes:\n- ship the beta on Friday\n- review the pricing page\n- book the venue",
            "Notas da reunião:\n- lançar o beta na sexta\n- revisar a página de preços\n- reservar o local",
            "Besprechungsnotizen:\n- Beta am Freitag veröffentlichen\n- Preisseite prüfen\n- Veranstaltungsort buchen",
            "Notas de la reunión:\n- lanzar la beta el viernes\n- revisar la página de precios\n- reservar el local",
            "会议记录：\n- 周五发布测试版\n- 检查价格页面\n- 预订场地",
        ],
    );
    // (age in minutes, id), oldest first
    let mut plan: Vec<(i64, i64)> = Vec::new();
    let mut note = |age: i64, id: Option<i64>| {
        if let Some(i) = id {
            plan.push((age, i));
        }
        id
    };
    let pinned = note(60 * 30, s(db.save_text_from(meeting, Some("Notes")))?);
    note(60 * 5, s(db.save_text_from("fn main() {\n    println!(\"hello\");\n}", Some("Text Editor")))?);
    note(60 * 2, s(db.save_files("/home/demo/Documents/report-q3.pdf\n/home/demo/Documents/budget.ods", Some("Files")))?);
    let img_b = note(40, s(db.save_image("demo-b.png", 640, 400, Some("Screenshot")))?);
    note(12, s(db.save_text_from("ssh deploy@staging.example.com", Some("Terminal")))?);
    note(8, s(db.save_text_from("#6C5CE7", Some("Text Editor")))?);
    let img_a = note(5, s(db.save_image("demo-a.png", 640, 400, Some("Screenshot")))?);
    note(2, s(db.save_text_from("https://example.com/docs/getting-started", Some("Web Browser")))?);
    note(0, s(db.save_text_from(DEMO_COPIED_TEXT, Some("Text Editor")))?);
    for (age, id) in plan {
        db.set_created_at(id, now - age * min).map_err(|e| e.to_string())?;
    }
    if let Some(id) = pinned {
        db.toggle_pinned(id).map_err(|e| e.to_string())?;
    }
    if let Some(id) = img_a {
        db.set_ocr_text(id, "Quarterly results\nRevenue up 12%").map_err(|e| e.to_string())?;
    }
    if let Some(id) = img_b {
        db.set_ocr_text(id, "").map_err(|e| e.to_string())?;
    }
    Ok(db)
}

/// A made-up capture for the editor: an invented billing page with a card number to blur. Nothing of the
/// machine is in it.
fn demo_capture(lang: Language) -> Option<gtk4::gdk::Texture> {
    use gtk4::pango;
    use gtk4::prelude::Cast;
    let (w, h) = (880, 520);
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h).ok()?;
    {
        let cr = cairo::Context::new(&surface).ok()?;
        cr.set_source_rgb(0.97, 0.97, 0.98);
        cr.paint().ok()?;
        cr.set_source_rgb(0.21, 0.33, 0.78);
        cr.rectangle(0.0, 0.0, w as f64, 56.0);
        cr.fill().ok()?;
        let text = |cr: &cairo::Context, t: &str, x: f64, y: f64, size: f64, bold: bool, rgb: (f64, f64, f64)| {
            let layout = pangocairo::functions::create_layout(cr);
            let mut d = pango::FontDescription::new();
            d.set_family("Sans");
            if bold {
                d.set_weight(pango::Weight::Bold);
            }
            d.set_absolute_size(size * pango::SCALE as f64);
            layout.set_font_description(Some(&d));
            layout.set_text(t);
            cr.set_source_rgb(rgb.0, rgb.1, rgb.2);
            cr.move_to(x, y);
            pangocairo::functions::show_layout(cr, &layout);
        };
        let ink = (0.12, 0.13, 0.16);
        text(&cr, pick(lang, ["Billing", "Cobrança", "Abrechnung", "Facturación", "账单"]), 40.0, 12.0, 28.0, true, (1.0, 1.0, 1.0));
        text(&cr, pick(lang, ["Invoice #1042", "Fatura #1042", "Rechnung #1042", "Factura #1042", "发票 #1042"]), 40.0, 96.0, 28.0, true, ink);
        text(&cr, pick(lang, ["Total", "Total", "Summe", "Total", "合计"]), 40.0, 160.0, 28.0, false, ink);
        text(&cr, "USD 1,280.00", 240.0, 160.0, 28.0, true, ink);
        text(&cr, pick(lang, ["Card", "Cartão", "Karte", "Tarjeta", "银行卡"]), 40.0, 220.0, 28.0, false, ink);
        text(&cr, "4111 1111 1111 1111", 250.0, 220.0, 28.0, false, ink);
        text(&cr, pick(lang, ["Paid", "Pago", "Bezahlt", "Pagado", "已付款"]), 60.0, 280.0, 28.0, true, (0.1, 0.5, 0.25));
        text(&cr, "example.com", 40.0, 460.0, 20.0, false, (0.45, 0.46, 0.5));
    }
    let mut surface = surface;
    surface.flush();
    let stride = surface.stride() as usize;
    let bytes = {
        let data = surface.data().ok()?;
        glib::Bytes::from(&data[..])
    };
    Some(gtk4::gdk::MemoryTexture::new(w, h, gtk4::gdk::MemoryFormat::B8g8r8a8Premultiplied, &bytes, stride).upcast())
}

/// The marks of the demo editor: a blur over the card number, a box around "Paid", an arrow to the total and a text.
fn demo_marks(lang: Language) -> Vec<menew_core::capture::Mark> {
    use menew_core::capture::{Mark, Point, Tool, COLORS};
    let mut text = Mark::new(Tool::Text, Point::new(600.0, 340.0), Point::new(600.0, 340.0), COLORS[4]);
    text.text = pick(lang, ["Check this", "Conferir", "Prüfen", "Revisar", "请核对"]).to_string();
    vec![
        Mark::new(Tool::Blur, Point::new(240.0, 212.0), Point::new(560.0, 266.0), COLORS[0]),
        Mark::new(Tool::Rect, Point::new(44.0, 270.0), Point::new(190.0, 326.0), COLORS[0]),
        Mark::new(Tool::Arrow, Point::new(590.0, 350.0), Point::new(470.0, 190.0), COLORS[0]),
        text,
    ]
}

/// The content of a window drawn with a soft shadow on a transparent margin, like the palette card.
fn render_content(window: &gtk4::Window, child: &gtk4::Widget, path: &Path) -> Result<(), String> {
    use gtk4::prelude::*;
    use gtk4::{graphene, gsk};
    let (w, h) = (child.width() as f32, child.height() as f32);
    if w < 1.0 || h < 1.0 {
        return Err("the window has no size yet".into());
    }
    let margin = 32.0_f32;
    let snap = gtk4::Snapshot::new();
    let card = gsk::RoundedRect::from_rect(graphene::Rect::new(margin, margin, w, h), 14.0);
    snap.append_outset_shadow(&card, &gtk4::gdk::RGBA::new(0.0, 0.0, 0.0, 0.30), 0.0, 8.0, 0.0, 24.0);
    snap.push_rounded_clip(&card);
    snap.translate(&graphene::Point::new(margin, margin));
    gtk4::WidgetPaintable::new(Some(child)).snapshot(&snap, w as f64, h as f64);
    snap.pop();
    let node = snap.to_node().ok_or("nothing was drawn")?;
    let renderer = window.native().and_then(|n| n.renderer()).ok_or("no renderer")?;
    let texture = renderer.render_texture(&node, Some(&graphene::Rect::new(0.0, 0.0, w + 2.0 * margin, h + 2.0 * margin)));
    texture.save_to_png(path).map_err(|e| e.to_string())
}

/// What each screen shows.
fn apply(screen: &str, lang: Language, p: &Rc<PaletteWindow>) {
    match screen {
        "palette" => p.demo_apps("", None),
        "palette-apps" => p.demo_apps(pick(lang, ["te", "te", "te", "te", "终"]), None),
        "palette-conversion" => p.demo_apps("5 km to mi", None),
        "palette-emoji" => p.demo_apps("emoji rocket", None),
        "palette-transform" => p.demo_apps("snake_case", Some(DEMO_COPIED_TEXT)),
        "clipboard" => p.demo_clipboard(ClipFilter::All, ""),
        "clipboard-images" => p.demo_clipboard(ClipFilter::Image, ""),
        "clipboard-snippets" => p.demo_clipboard(ClipFilter::Snippets, ""),
        _ => {}
    }
}

/// Draw every screen in light and dark into `dir`, then quit `app`. Sets `failed` on any error.
pub fn run(app: &adw::Application, dir: PathBuf, lang: Language, failed: Rc<Cell<bool>>) {
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("snapshot: cannot create {}: {e}", dir.display());
        failed.set(true);
        app.quit();
        return;
    }
    let images = std::env::temp_dir().join(format!("menew-snapshot-{}", std::process::id()));
    let db = match demo_db(lang, &images) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("snapshot: demo data: {e}");
            failed.set(true);
            app.quit();
            return;
        }
    };
    let palette = PaletteWindow::create_demo(app, db, demo_state(lang), demo_apps(lang));
    let jobs: Vec<(bool, &'static str)> = [false, true].into_iter().flat_map(|dark| SCREENS.iter().map(move |s| (dark, *s))).collect();
    let run = Rc::new(Run { app: app.clone(), palette, dir, jobs, images, failed, lang });
    step(run, 0);
}

/// Everything the screens loop shares.
struct Run {
    app: adw::Application,
    palette: Rc<PaletteWindow>,
    dir: PathBuf,
    jobs: Vec<(bool, &'static str)>,
    images: PathBuf,
    failed: Rc<Cell<bool>>,
    lang: Language,
}

fn step(run: Rc<Run>, i: usize) {
    let Some(&(dark, screen)) = run.jobs.get(i) else {
        let _ = std::fs::remove_dir_all(&run.images);
        run.app.quit();
        return;
    };
    adw::StyleManager::default().set_color_scheme(if dark { adw::ColorScheme::ForceDark } else { adw::ColorScheme::ForceLight });
    if is_window_screen(screen) {
        window_step(run, i, dark, screen);
        return;
    }
    apply(screen, run.lang, &run.palette);
    glib::timeout_add_local_once(Duration::from_millis(SETTLE_MS), move || {
        let file = run.dir.join(format!("{screen}-{}.png", if dark { "dark" } else { "light" }));
        match run.palette.capture_png(&file) {
            Ok(()) => println!("snapshot: {}", file.display()),
            Err(e) => {
                eprintln!("snapshot: {}: {e}", file.display());
                run.failed.set(true);
            }
        }
        step(run, i + 1);
    });
}

/// A screen that is a window of its own: open it with its fake data, draw it and close it.
fn window_step(run: Rc<Run>, i: usize, dark: bool, screen: &'static str) {
    let sa = Rc::clone(run.palette.screen());
    let opened = match screen {
        "capture-editor" => demo_capture(run.lang).and_then(|t| sa.demo_editor(t, demo_marks(run.lang))),
        "qr-code" => sa.demo_qr("https://example.com/docs/getting-started", None),
        _ => sa.demo_qr("", Some(("Casa-Demo", "demo-wifi-123"))),
    };
    let Some((window, content)) = opened else {
        eprintln!("snapshot: {screen}: could not open the window");
        run.failed.set(true);
        step(run, i + 1);
        return;
    };
    glib::timeout_add_local_once(Duration::from_millis(SETTLE_MS), move || {
        let file = run.dir.join(format!("{screen}-{}.png", if dark { "dark" } else { "light" }));
        match render_content(&window, &content, &file) {
            Ok(()) => println!("snapshot: {}", file.display()),
            Err(e) => {
                eprintln!("snapshot: {}: {e}", file.display());
                run.failed.set(true);
            }
        }
        sa.demo_close();
        step(run, i + 1);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `--snapshot` never reads the machine: the app list is the fixed demo list, in every language.
    #[test]
    fn snapshot_uses_no_real_data() {
        for lang in [Language::En, Language::Pt, Language::De, Language::Es, Language::Zh] {
            let apps = demo_apps(lang);
            assert_eq!(apps.len(), 10);
            for a in &apps {
                assert!(a.id.starts_with("demo-"), "{} is not a demo entry", a.id);
                assert!(a.file_path.starts_with("/nonexistent"), "{} points to a real file", a.id);
                assert!(!a.name.is_empty());
            }
        }
    }
}
