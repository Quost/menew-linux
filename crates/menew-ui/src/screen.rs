//! The screen: capture (the system picks the area, window or screen), the annotation editor, the text of a capture
//! and QR codes (read one from the screen, show one for a text, a link or a Wi-Fi network). The port of
//! `Capture.swift`, `TextRecognizer.captureScreenText` and `QRCode.swift`; the rules are in `menew_core::capture`
//! and `menew_core::qr`, the vectors in `docs/screenshots-spec.md`.
//!
//! Privacy: a capture lives in memory. The file the portal wrote is read and deleted at once; the only other file
//! is the one given to tesseract, in the private runtime folder of the user, deleted when it was read. Nothing is
//! sent anywhere. Reading a QR code only copies its text: nothing in this file opens an address or runs a program
//! (a test in `menew-core` reads this file to make sure).

use gtk4::prelude::*;
use gtk4::{cairo, gdk, gio, glib, pango};
use libadwaita as adw;
use libadwaita::prelude::*;
use menew_core::capture::{self, Mark, Point, Rgb, TextOutcome, Tool};
use menew_core::qr::{self, ScanOutcome};
use menew_core::strings::tr;
use menew_core::{current_language, Language};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use tracing::{info, warn};

/// What to do with a capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Open the annotation editor.
    Edit,
    /// Copy the text in it.
    Text,
    /// Copy the content of the QR code in it.
    Qr,
}

/// What the capture call gives back: the file the system wrote, `None` when the user cancelled, or why it failed.
pub type CaptureResult = Result<Option<PathBuf>, String>;
/// Asks the system for a capture: `interactive` lets the user pick the area, window or screen.
pub type Portal = Rc<dyn Fn(bool, Box<dyn FnOnce(CaptureResult)>)>;

/// The folder for the pictures that are only in transit: private to the user.
fn private_dir() -> PathBuf {
    use std::os::unix::fs::DirBuilderExt;
    let dir = glib::user_runtime_dir().join("menew");
    let _ = std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&dir);
    dir
}

fn unique_name(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{prefix}-{}-{nanos}.png", std::process::id())
}

fn cairo_rgb(cr: &cairo::Context, c: Rgb, alpha: f64) {
    cr.set_source_rgba(c.0 as f64 / 255.0, c.1 as f64 / 255.0, c.2 as f64 / 255.0, alpha);
}

fn load_texture(path: &Path) -> Result<gdk::Texture, String> {
    let pixbuf = gtk4::gdk_pixbuf::Pixbuf::from_file(path).map_err(|e| e.to_string())?;
    Ok(gdk::Texture::for_pixbuf(&pixbuf))
}

/// The pixels of a texture as the memory format of Cairo's `ARgb32` (blue, green, red, alpha; premultiplied).
fn texture_pixels(texture: &gdk::Texture) -> (usize, usize, Vec<u8>) {
    let (w, h) = (texture.width() as usize, texture.height() as usize);
    let mut data = vec![0u8; w * h * 4];
    texture.download(&mut data, w * 4);
    (w, h, data)
}

fn bgra_texture(w: usize, h: usize, data: &[u8]) -> gdk::Texture {
    let bytes = glib::Bytes::from(data);
    gdk::MemoryTexture::new(w as i32, h as i32, gdk::MemoryFormat::B8g8r8a8Premultiplied, &bytes, w * 4).upcast()
}

// ---------------------------------------------------------------- the capture and what is done with it

pub struct ScreenApp {
    app: adw::Application,
    portal: Portal,
    busy: Cell<bool>,
    /// Headless test sessions only let a client with a mapped window set the clipboard (see `docs/parity.md`).
    map_hook: RefCell<Option<Rc<dyn Fn()>>>,
    editors: RefCell<Vec<Rc<Editor>>>,
    qr: RefCell<Option<Rc<QrWindow>>>,
}

impl ScreenApp {
    pub fn new(app: &adw::Application, portal: Portal) -> Rc<Self> {
        Rc::new(Self { app: app.clone(), portal, busy: Cell::new(false), map_hook: RefCell::new(None), editors: RefCell::new(vec![]), qr: RefCell::new(None) })
    }

    /// For `--snapshot` and tests: there is no screen to capture.
    pub fn inert(app: &adw::Application) -> Rc<Self> {
        Self::new(app, Rc::new(|_, done| done(Err("no screen in this mode".into()))))
    }

    pub fn set_map_hook(&self, hook: Rc<dyn Fn()>) {
        *self.map_hook.borrow_mut() = Some(hook);
    }

    fn lang() -> Language {
        current_language()
    }

    fn notify(&self, title: &str) {
        let n = gio::Notification::new(title);
        self.app.send_notification(Some("menew-screen"), &n);
        info!("Notification sent: {}", title);
    }

    fn clipboard(&self) -> Option<gdk::Clipboard> {
        if let Some(hook) = self.map_hook.borrow().as_ref() {
            hook();
        }
        gdk::Display::default().map(|d| d.clipboard())
    }

    fn copy_text(&self, text: &str) {
        if let Some(c) = self.clipboard() {
            c.set_text(text);
        }
    }

    /// `ScreenCapture.area` / `fullScreen`, `TextRecognizer.captureScreenText`, `QRCode.scanScreen`: ask the system for
    /// a capture and do what `purpose` says with it. `interactive` lets the user pick; without it the whole screen is
    /// taken (after a moment, so that the palette is gone, as the Mac waits 0.35 s).
    pub fn capture(self: &Rc<Self>, purpose: Purpose, interactive: bool) {
        if self.busy.get() {
            info!("Capture: one is already waiting for the system");
            return;
        }
        self.busy.set(true);
        let this = Rc::clone(self);
        let start = move || {
            let again = Rc::clone(&this);
            (this.portal)(
                interactive,
                Box::new(move |res| {
                    again.busy.set(false);
                    match res {
                        Ok(None) => info!("Capture: cancelled"),
                        Ok(Some(path)) => again.open_file(purpose, &path, true),
                        Err(e) => {
                            warn!("Capture failed: {}", e);
                            again.notify(&tr("cap_portal_failed", Self::lang(), &[&e]));
                        }
                    }
                }),
            );
        };
        if interactive {
            start();
        } else {
            glib::timeout_add_local_once(std::time::Duration::from_millis(350), start);
        }
    }

    /// The capture is a file the system wrote: read it, delete it (`delete`) and use it for `purpose`. The test hook
    /// `__capture-from` feeds a known image here and keeps the file.
    pub fn open_file(self: &Rc<Self>, purpose: Purpose, path: &Path, delete: bool) {
        let texture = load_texture(path);
        if delete {
            let _ = std::fs::remove_file(path);
        }
        match texture {
            Ok(t) => self.use_texture(purpose, t),
            Err(e) => {
                warn!("Capture: cannot open {}: {}", path.display(), e);
                self.notify(&tr("cap_unreadable", Self::lang(), &[]));
            }
        }
    }

    fn use_texture(self: &Rc<Self>, purpose: Purpose, texture: gdk::Texture) {
        match purpose {
            Purpose::Edit => self.edit(texture),
            Purpose::Text => self.read_text(texture, true),
            Purpose::Qr => self.scan_qr(texture),
        }
    }

    // ---- the editor ----

    pub fn edit(self: &Rc<Self>, texture: gdk::Texture) {
        let Some(doc) = Doc::new(texture) else {
            self.notify(&tr("cap_unreadable", Self::lang(), &[]));
            return;
        };
        let editor = Editor::new(self, doc);
        self.editors.borrow_mut().push(Rc::clone(&editor));
        editor.show();
    }

    fn editor_closed(&self, editor: &Editor) {
        self.editors.borrow_mut().retain(|e| !std::ptr::eq(Rc::as_ptr(e), editor));
    }

    // ---- text from the screen ----

    /// `TextRecognizer`: read the text of the image with tesseract (in the background) and copy it. `count_lines`: the
    /// message says how many lines (the shortcut does, the button of the editor does not).
    pub fn read_text(self: &Rc<Self>, texture: gdk::Texture, count_lines: bool) {
        let lang = Self::lang();
        if !menew_core::ocr::available() {
            self.notify(&tr("ocr_missing", lang, &[]));
            return;
        }
        // "Reading text...": without it it looks as if the copy was instant and the paste gives the old content
        self.notify(&tr("ocr_reading", lang, &[]));
        let path = private_dir().join(unique_name("ocr"));
        if gdk::prelude::TextureExt::save_to_png(&texture, &path).is_err() {
            self.notify(&tr("cap_unreadable", lang, &[]));
            return;
        }
        let this = Rc::clone(self);
        glib::spawn_future_local(async move {
            let res = gio::spawn_blocking(move || {
                let r = menew_core::ocr::recognize(&path);
                let _ = std::fs::remove_file(&path);
                r
            })
            .await;
            match res {
                Ok(Ok(text)) => this.finish_text(&text, count_lines),
                Ok(Err(menew_core::ocr::OcrError::Missing)) => this.notify(&tr("ocr_missing", Self::lang(), &[])),
                Ok(Err(e)) => {
                    warn!("OCR failed: {}", e);
                    this.notify(&tr("ocr_none", Self::lang(), &[]));
                }
                Err(_) => warn!("OCR: the reader stopped"),
            }
        });
    }

    /// `TextRecognizer.finish`.
    fn finish_text(&self, recognized: &str, count_lines: bool) {
        let lang = Self::lang();
        match capture::text_outcome(recognized) {
            TextOutcome::Nothing => self.notify(&tr("ocr_none", lang, &[])),
            TextOutcome::Copy { text, lines } => {
                self.copy_text(&text);
                if lines == 1 || !count_lines {
                    self.notify(&tr("ocr_copied", lang, &[]));
                } else {
                    self.notify(&tr("ocr_copied_lines", lang, &[&lines.to_string()]));
                }
                info!("Text from the screen copied ({} lines, {} characters)", lines, text.chars().count());
            }
        }
    }

    // ---- QR codes ----

    /// `QRCode.scanScreen`: the content of the first code goes to the clipboard. A link is **not** opened.
    pub fn scan_qr(self: &Rc<Self>, texture: gdk::Texture) {
        let (w, h, data) = texture_pixels(&texture);
        let this = Rc::clone(self);
        glib::spawn_future_local(async move {
            let found = gio::spawn_blocking(move || qr::decode_bgra(w, h, &data)).await.unwrap_or_default();
            let lang = Self::lang();
            let outcome = qr::scan_outcome(&found);
            match &outcome {
                ScanOutcome::Nothing => this.notify(&tr("qr_none", lang, &[])),
                ScanOutcome::Link(t) => {
                    this.copy_text(t);
                    this.notify(&tr("qr_link_copied", lang, &[]));
                }
                ScanOutcome::Content(t) => {
                    this.copy_text(t);
                    this.notify(&tr("qr_content_copied", lang, &[]));
                }
            }
            info!("QR scan: {} code(s) found, outcome {}", found.len(), match outcome {
                ScanOutcome::Nothing => "none",
                ScanOutcome::Link(_) => "link copied",
                ScanOutcome::Content(_) => "content copied",
            });
        });
    }

    /// `QRCode.show`: the window with the code of `text`, of the copied text when it is `None`, or in the Wi-Fi mode.
    pub fn show_qr(self: &Rc<Self>, text: Option<String>, wifi: bool) {
        let win = {
            let mut slot = self.qr.borrow_mut();
            Rc::clone(slot.get_or_insert_with(|| QrWindow::new(self)))
        };
        win.set_wifi(wifi);
        match text {
            Some(t) => win.text.set_text(&t),
            None if !wifi => {
                // the copied text (not when it carries a password hint: the daemon never keeps those, and a QR is public)
                if let Some(display) = gdk::Display::default() {
                    let clipboard = display.clipboard();
                    if !clipboard.formats().mime_types().iter().any(|m| menew_core::clip_policy::is_secret_type(m)) {
                        let w = Rc::clone(&win);
                        clipboard.read_text_async(gio::Cancellable::NONE, move |res| {
                            if let Ok(Some(t)) = res {
                                w.text.set_text(&t);
                            }
                        });
                    }
                }
            }
            None => {}
        }
        win.window.present();
    }

    /// The image of the code of `payload` (1024 pixels or a little more, with a white border).
    pub fn qr_texture(payload: &str) -> Option<gdk::Texture> {
        let m = qr::encode(payload)?;
        let (w, h, gray) = qr::render_gray(&m, qr::scale_for(&m, 1024));
        let bgra: Vec<u8> = gray.iter().flat_map(|v| [*v, *v, *v, 255]).collect();
        Some(bgra_texture(w, h, &bgra))
    }

    // ---- test hooks (`MENEW_TEST=1`) ----

    /// `__annotate <in.png> <out.png> <marks>`: the export of the editor, without a window.
    pub fn test_annotate(self: &Rc<Self>, input: &Path, output: &Path, spec: &str) -> Result<(), String> {
        let doc = Doc::new(load_texture(input)?).ok_or("cannot read the image")?;
        *doc.marks.borrow_mut() = parse_marks(spec)?;
        doc.export_png(output)?;
        info!("Test hook: annotated {} -> {} ({} marks)", input.display(), output.display(), doc.marks.borrow().len());
        Ok(())
    }

    /// `__capture-save <in.png> <dir>`: what Save writes, with the name the Mac gives it, into `dir`.
    pub fn test_save(self: &Rc<Self>, input: &Path, dir: &Path) -> Result<PathBuf, String> {
        let doc = Doc::new(load_texture(input)?).ok_or("cannot read the image")?;
        let path = dir.join(capture::screenshot_file_name_now(Self::lang()));
        doc.export_png(&path)?;
        info!("Test hook: capture saved to {}", path.display());
        Ok(path)
    }

    /// `__capture-copy <in.png> <marks>`: what the Copy button does (the capture with its marks on the clipboard).
    pub fn test_copy(self: &Rc<Self>, input: &Path, spec: &str) -> Result<(), String> {
        let doc = Doc::new(load_texture(input)?).ok_or("cannot read the image")?;
        *doc.marks.borrow_mut() = parse_marks(spec)?;
        let texture = doc.render()?;
        if let Some(c) = self.clipboard() {
            c.set_texture(&texture);
        }
        self.notify(&tr("cap_copied", Self::lang(), &[]));
        info!("Test hook: capture copied to the clipboard ({} x {})", doc.w, doc.h);
        Ok(())
    }

    /// `__qr-image <payload> <out.png>`: the image of "Copy image".
    pub fn test_qr_image(payload: &str, output: &Path) -> Result<(), String> {
        let t = Self::qr_texture(payload).ok_or("the text does not fit in a QR code")?;
        gdk::prelude::TextureExt::save_to_png(&t, output).map_err(|e| e.to_string())?;
        info!("Test hook: QR image of {} characters -> {}", payload.chars().count(), output.display());
        Ok(())
    }

    // ---- `menew --snapshot` ----

    /// The editor with marks already on it, over a picture drawn here. Returns the content of its window.
    pub fn demo_editor(self: &Rc<Self>, texture: gdk::Texture, marks: Vec<Mark>) -> Option<(gtk4::Window, gtk4::Widget)> {
        let doc = Doc::new(texture)?;
        *doc.marks.borrow_mut() = marks;
        let editor = Editor::new(self, doc);
        editor.after_change();
        self.editors.borrow_mut().push(Rc::clone(&editor));
        editor.show();
        let content = editor.window.content()?;
        Some((editor.window.clone().upcast(), content))
    }

    /// The QR window showing `text` (or the Wi-Fi mode with a name and a password). Returns its content.
    pub fn demo_qr(self: &Rc<Self>, text: &str, wifi: Option<(&str, &str)>) -> Option<(gtk4::Window, gtk4::Widget)> {
        let win = {
            let mut slot = self.qr.borrow_mut();
            Rc::clone(slot.get_or_insert_with(|| QrWindow::new(self)))
        };
        win.text.set_text(text);
        if let Some((name, password)) = wifi {
            win.name.set_text(name);
            win.password.set_text(password);
        }
        win.set_wifi(wifi.is_some());
        win.window.present();
        let content = win.window.content()?;
        Some((win.window.clone().upcast(), content))
    }

    /// Hide the windows of the demo.
    pub fn demo_close(&self) {
        for e in self.editors.borrow().iter() {
            e.window.set_visible(false);
        }
        self.editors.borrow_mut().clear();
        if let Some(q) = self.qr.borrow().as_ref() {
            q.window.set_visible(false);
        }
    }

    /// The payload the QR window shows now (for the test hook `__qr-show`).
    pub fn qr_payload(&self) -> Option<String> {
        self.qr.borrow().as_ref().map(|w| w.payload())
    }
}

// ---------------------------------------------------------------- the document

/// `CaptureDocument`: the image, the same image pixelated (shown inside the blur marks) and the marks.
struct Doc {
    w: i32,
    h: i32,
    base: cairo::ImageSurface,
    pix: cairo::ImageSurface,
    texture: gdk::Texture,
    marks: RefCell<Vec<Mark>>,
}

impl Doc {
    fn new(texture: gdk::Texture) -> Option<Doc> {
        let (w, h, data) = texture_pixels(&texture);
        if w == 0 || h == 0 {
            return None;
        }
        let pixelated = capture::pixelate(&data, w, h);
        let stride = (w * 4) as i32;
        let base = cairo::ImageSurface::create_for_data(data, cairo::Format::ARgb32, w as i32, h as i32, stride).ok()?;
        let pix = cairo::ImageSurface::create_for_data(pixelated, cairo::Format::ARgb32, w as i32, h as i32, stride).ok()?;
        Some(Doc { w: w as i32, h: h as i32, base, pix, texture, marks: RefCell::new(vec![]) })
    }

    /// `CaptureCanvas`: the image and the marks, drawn `k` times their size (1 to export).
    fn paint(&self, cr: &cairo::Context, marks: &[Mark], k: f64) {
        let (w, h) = (self.w as f64, self.h as f64);
        let lw = capture::line_width(w, h);
        let _ = cr.save();
        cr.scale(k, k);
        let _ = cr.set_source_surface(&self.base, 0.0, 0.0);
        let _ = cr.paint();
        for m in marks {
            cr.set_line_width(lw);
            cr.set_line_cap(cairo::LineCap::Round);
            cr.set_line_join(cairo::LineJoin::Round);
            match m.kind {
                Tool::Blur => {
                    // the pixelated image, only inside the box (a real blur of the pixels, not a gray rectangle)
                    if let Some((x0, y0, x1, y1)) = capture::blur_pixels(m, self.w as usize, self.h as usize) {
                        let _ = cr.save();
                        cr.rectangle(x0 as f64, y0 as f64, (x1 - x0) as f64, (y1 - y0) as f64);
                        cr.clip();
                        let _ = cr.set_source_surface(&self.pix, 0.0, 0.0);
                        let _ = cr.paint();
                        let _ = cr.restore();
                    }
                }
                Tool::Pen => {
                    if let Some(first) = m.points.first() {
                        cairo_rgb(cr, m.color, 1.0);
                        cr.move_to(first.x, first.y);
                        for p in &m.points[1..] {
                            cr.line_to(p.x, p.y);
                        }
                        let _ = cr.stroke();
                    }
                }
                Tool::Rect => {
                    let (x, y, rw, rh) = m.rect();
                    let r = capture::rect_corner_radius(lw).min(rw / 2.0).min(rh / 2.0);
                    cairo_rgb(cr, m.color, 1.0);
                    use std::f64::consts::{FRAC_PI_2, PI};
                    cr.new_sub_path();
                    cr.arc(x + rw - r, y + r, r, -FRAC_PI_2, 0.0);
                    cr.arc(x + rw - r, y + rh - r, r, 0.0, FRAC_PI_2);
                    cr.arc(x + r, y + rh - r, r, FRAC_PI_2, PI);
                    cr.arc(x + r, y + r, r, PI, 3.0 * FRAC_PI_2);
                    cr.close_path();
                    let _ = cr.stroke();
                }
                Tool::Arrow => {
                    cairo_rgb(cr, m.color, 1.0);
                    cr.move_to(m.a.x, m.a.y);
                    cr.line_to(m.b.x, m.b.y);
                    for p in capture::arrow_head_points(m.a, m.b, lw) {
                        cr.move_to(m.b.x, m.b.y);
                        cr.line_to(p.x, p.y);
                    }
                    let _ = cr.stroke();
                }
                Tool::Text => {
                    let layout = pangocairo::functions::create_layout(cr);
                    let mut desc = pango::FontDescription::new();
                    desc.set_family("Sans");
                    desc.set_weight(pango::Weight::Bold);
                    desc.set_absolute_size(capture::font_size(w, h) * pango::SCALE as f64);
                    layout.set_font_description(Some(&desc));
                    layout.set_text(&m.text);
                    // an outline so that it reads on any background
                    let (outline, alpha) = capture::text_outline(m.color);
                    let o = capture::text_outline_offset(lw);
                    cairo_rgb(cr, outline, alpha);
                    for (dx, dy) in [(-o, 0.0), (o, 0.0), (0.0, -o), (0.0, o)] {
                        cr.move_to(m.a.x + dx, m.a.y + dy);
                        pangocairo::functions::show_layout(cr, &layout);
                    }
                    cairo_rgb(cr, m.color, 1.0);
                    cr.move_to(m.a.x, m.a.y);
                    pangocairo::functions::show_layout(cr, &layout);
                }
            }
        }
        let _ = cr.restore();
    }

    /// `CaptureDocument.render`: the capture with the marks, at its own size.
    fn render(&self) -> Result<gdk::Texture, String> {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, self.w, self.h).map_err(|e| e.to_string())?;
        {
            let cr = cairo::Context::new(&surface).map_err(|e| e.to_string())?;
            self.paint(&cr, &self.marks.borrow(), 1.0);
        }
        let mut surface = surface;
        surface.flush();
        let stride = surface.stride() as usize;
        let bytes = {
            let data = surface.data().map_err(|e| e.to_string())?;
            glib::Bytes::from(&data[..])
        };
        Ok(gdk::MemoryTexture::new(self.w, self.h, gdk::MemoryFormat::B8g8r8a8Premultiplied, &bytes, stride).upcast())
    }

    fn export_png(&self, path: &Path) -> Result<(), String> {
        let t = self.render()?;
        gdk::prelude::TextureExt::save_to_png(&t, path).map_err(|e| e.to_string())
    }
}

/// `x,y,x,y[:color]` marks of the test hook, separated by `;`: `blur:20,20,60,50`, `rect:5,5,50,40:red`,
/// `arrow:0,0,100,50:blue`, `pen:1,1 5,5 9,9:green`, `text:10,10:black:Hello`.
fn parse_marks(spec: &str) -> Result<Vec<Mark>, String> {
    let color = |name: &str| -> Result<Rgb, String> {
        let names = ["red", "orange", "yellow", "green", "blue", "purple", "black", "white"];
        names.iter().position(|n| *n == name).map(|i| capture::COLORS[i]).ok_or(format!("unknown color {name}"))
    };
    let pt = |s: &str| -> Result<Point, String> {
        let (x, y) = s.split_once(',').ok_or(format!("bad point {s}"))?;
        Ok(Point::new(x.parse::<f64>().map_err(|e| e.to_string())?, y.parse::<f64>().map_err(|e| e.to_string())?))
    };
    let mut out = vec![];
    for item in spec.split(';').filter(|s| !s.trim().is_empty()) {
        let mut parts = item.splitn(4, ':');
        let kind = parts.next().unwrap_or_default();
        let coords = parts.next().ok_or(format!("bad mark {item}"))?;
        let col = match parts.next() {
            Some(c) if !c.is_empty() => color(c)?,
            _ => capture::DEFAULT_COLOR,
        };
        let mark = match kind {
            "pen" => {
                let points: Vec<Point> = coords.split(' ').map(pt).collect::<Result<_, _>>()?;
                let mut m = Mark::new(Tool::Pen, points[0], *points.last().unwrap(), col);
                m.points = points;
                m
            }
            "text" => {
                let a = pt(coords)?;
                let mut m = Mark::new(Tool::Text, a, a, col);
                m.text = parts.next().unwrap_or_default().to_string();
                m
            }
            "arrow" | "rect" | "blur" => {
                let v: Vec<&str> = coords.split(',').collect();
                if v.len() != 4 {
                    return Err(format!("bad box {coords}"));
                }
                let tool = match kind {
                    "arrow" => Tool::Arrow,
                    "rect" => Tool::Rect,
                    _ => Tool::Blur,
                };
                Mark::new(tool, pt(&format!("{},{}", v[0], v[1]))?, pt(&format!("{},{}", v[2], v[3]))?, col)
            }
            other => return Err(format!("unknown mark {other}")),
        };
        out.push(mark);
    }
    Ok(out)
}

// ---------------------------------------------------------------- the editor window

const EDITOR_CSS: &str = "
    .menew-cap-canvas { background: alpha(currentColor, 0.06); border-radius: 6px; }
    .menew-cap-dot { min-width: 24px; min-height: 24px; padding: 2px; }
";

/// What a keyboard shortcut of the editor does.
type KeyAction = Box<dyn Fn(&Rc<Editor>)>;

struct Editor {
    sa: Rc<ScreenApp>,
    doc: Doc,
    window: adw::ApplicationWindow,
    area: gtk4::DrawingArea,
    entry: gtk4::Entry,
    entry_css: gtk4::CssProvider,
    undo_btn: gtk4::Button,
    colors_box: gtk4::Box,
    tool: Cell<Tool>,
    color: Cell<Rgb>,
    draft: RefCell<Option<Mark>>,
    start: Cell<Point>,
    /// The text being typed: where it goes and in which color.
    typing: RefCell<Option<Mark>>,
    dialog: RefCell<Option<gtk4::FileChooserNative>>,
    drag_dir: RefCell<Option<PathBuf>>,
}

impl Editor {
    fn new(sa: &Rc<ScreenApp>, doc: Doc) -> Rc<Editor> {
        let lang = ScreenApp::lang();
        if let Some(display) = gdk::Display::default() {
            let provider = gtk4::CssProvider::new();
            provider.load_from_data(EDITOR_CSS);
            gtk4::style_context_add_provider_for_display(&display, &provider, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }
        let entry_css = gtk4::CssProvider::new();
        if let Some(display) = gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(&display, &entry_css, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
        }

        // the window is as big as the capture, up to 80% of a common screen (`CaptureEditorController.show`)
        let fit = (1.0f64).min(1100.0 / doc.w as f64).min(620.0 / doc.h as f64);
        let window = adw::ApplicationWindow::builder()
            .application(&sa.app)
            .title(tr("cap_title", lang, &[]))
            .default_width(((doc.w as f64 * fit) as i32 + 28).max(640))
            .default_height((doc.h as f64 * fit) as i32 + 170)
            .build();
        window.add_css_class("menew-capture");

        let area = gtk4::DrawingArea::builder().hexpand(true).vexpand(true).build();
        area.add_css_class("menew-cap-canvas");
        let entry = gtk4::Entry::builder()
            .placeholder_text(tr("cap_tool_text", lang, &[]))
            .halign(gtk4::Align::Start)
            .valign(gtk4::Align::Start)
            .width_chars(14)
            .has_frame(false)
            .visible(false)
            .build();
        entry.add_css_class("menew-cap-entry");
        let undo_btn = gtk4::Button::builder().icon_name("edit-undo-symbolic").tooltip_text(tr("cap_undo", lang, &[])).sensitive(false).build();
        let colors_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);

        let ed = Rc::new(Editor {
            sa: Rc::clone(sa),
            doc,
            window,
            area,
            entry,
            entry_css,
            undo_btn,
            colors_box,
            tool: Cell::new(capture::DEFAULT_TOOL),
            color: Cell::new(capture::DEFAULT_COLOR),
            draft: RefCell::new(None),
            start: Cell::new(Point::new(0.0, 0.0)),
            typing: RefCell::new(None),
            dialog: RefCell::new(None),
            drag_dir: RefCell::new(None),
        });
        ed.build();
        ed
    }

    fn show(&self) {
        self.window.present();
        info!("Capture editor shown: {} x {} pixels, {} marks", self.doc.w, self.doc.h, self.doc.marks.borrow().len());
    }

    /// Scale and offset of the image inside the canvas: `(k, x, y)` (never enlarged, centered).
    fn layout(&self) -> (f64, f64, f64) {
        let (aw, ah) = (self.area.width() as f64, self.area.height() as f64);
        let k = (aw / self.doc.w as f64).min(ah / self.doc.h as f64).min(1.0);
        let k = if k > 0.0 { k } else { 1.0 };
        (k, (aw - self.doc.w as f64 * k) / 2.0, (ah - self.doc.h as f64 * k) / 2.0)
    }

    fn to_image(&self, x: f64, y: f64) -> Point {
        let (k, ox, oy) = self.layout();
        Point::new((x - ox) / k, (y - oy) / k)
    }

    fn build(self: &Rc<Self>) {
        let lang = ScreenApp::lang();
        let weak: Weak<Editor> = Rc::downgrade(self);

        // the canvas
        self.area.set_draw_func({
            let weak = weak.clone();
            move |_, cr, _, _| {
                let Some(ed) = weak.upgrade() else { return };
                let (k, ox, oy) = ed.layout();
                cr.translate(ox, oy);
                let mut marks = ed.doc.marks.borrow().clone();
                if let Some(d) = ed.draft.borrow().as_ref() {
                    marks.push(d.clone());
                }
                ed.doc.paint(cr, &marks, k);
            }
        });
        let drag = gtk4::GestureDrag::new();
        drag.connect_drag_begin({
            let weak = weak.clone();
            move |_, x, y| {
                if let Some(ed) = weak.upgrade() {
                    ed.start.set(ed.to_image(x, y));
                }
            }
        });
        drag.connect_drag_update({
            let weak = weak.clone();
            move |_, dx, dy| {
                if let Some(ed) = weak.upgrade() {
                    ed.dragging(dx, dy);
                }
            }
        });
        drag.connect_drag_end({
            let weak = weak.clone();
            move |_, dx, dy| {
                if let Some(ed) = weak.upgrade() {
                    ed.dragged(dx, dy);
                }
            }
        });
        self.area.add_controller(drag);

        // the text being typed sits over the canvas
        self.entry.connect_activate({
            let weak = weak.clone();
            move |_| {
                if let Some(ed) = weak.upgrade() {
                    ed.commit_text();
                }
            }
        });
        let focus = gtk4::EventControllerFocus::new();
        focus.connect_leave({
            let weak = weak.clone();
            move |_| {
                if let Some(ed) = weak.upgrade() {
                    ed.commit_text();
                }
            }
        });
        self.entry.add_controller(focus);
        let overlay = gtk4::Overlay::new();
        overlay.set_child(Some(&self.area));
        overlay.add_overlay(&self.entry);

        // the toolbar: tools, colors, undo
        let tools = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        tools.add_css_class("linked");
        let mut first: Option<gtk4::ToggleButton> = None;
        for (tool, key) in [(Tool::Pen, "cap_tool_pen"), (Tool::Arrow, "cap_tool_arrow"), (Tool::Rect, "cap_tool_rect"), (Tool::Text, "cap_tool_text"), (Tool::Blur, "cap_tool_blur")] {
            let b = gtk4::ToggleButton::builder().tooltip_text(tr(key, lang, &[])).active(tool == capture::DEFAULT_TOOL).build();
            b.set_child(Some(&tool_icon(tool)));
            if let Some(f) = &first {
                b.set_group(Some(f));
            } else {
                first = Some(b.clone());
            }
            let weak = weak.clone();
            b.connect_toggled(move |b| {
                if b.is_active() {
                    if let Some(ed) = weak.upgrade() {
                        ed.select_tool(tool);
                    }
                }
            });
            tools.append(&b);
        }
        let mut first_color: Option<gtk4::ToggleButton> = None;
        for c in capture::COLORS {
            let b = gtk4::ToggleButton::builder().active(c == capture::DEFAULT_COLOR).build();
            b.add_css_class("flat");
            b.add_css_class("circular");
            b.add_css_class("menew-cap-dot");
            let dot = gtk4::DrawingArea::builder().content_width(16).content_height(16).build();
            dot.set_draw_func(move |_, cr, w, h| {
                let r = (w.min(h) as f64) / 2.0 - 0.5;
                cr.arc(w as f64 / 2.0, h as f64 / 2.0, r, 0.0, std::f64::consts::TAU);
                cairo_rgb(cr, c, 1.0);
                let _ = cr.fill_preserve();
                cr.set_source_rgba(0.5, 0.5, 0.5, 0.5);
                cr.set_line_width(1.0);
                let _ = cr.stroke();
            });
            b.set_child(Some(&dot));
            if let Some(f) = &first_color {
                b.set_group(Some(f));
            } else {
                first_color = Some(b.clone());
            }
            let weak = weak.clone();
            b.connect_toggled(move |b| {
                if b.is_active() {
                    if let Some(ed) = weak.upgrade() {
                        ed.color.set(c);
                        if let Some(t) = ed.typing.borrow_mut().as_mut() {
                            t.color = c;
                        }
                    }
                }
            });
            self.colors_box.append(&b);
        }
        self.undo_btn.connect_clicked({
            let weak = weak.clone();
            move |_| {
                if let Some(ed) = weak.upgrade() {
                    ed.undo();
                }
            }
        });
        let toolbar = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
        toolbar.set_margin_start(14);
        toolbar.set_margin_end(14);
        toolbar.set_margin_top(10);
        toolbar.append(&tools);
        toolbar.append(&self.colors_box);
        let spacer = gtk4::Box::builder().hexpand(true).build();
        toolbar.append(&spacer);
        toolbar.append(&self.undo_btn);

        // the actions: drag to another app, copy the text, save, copy
        let actions = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        actions.set_margin_start(14);
        actions.set_margin_end(14);
        actions.set_margin_bottom(14);
        let handle = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        handle.set_tooltip_text(Some(&tr("cap_drag", lang, &[])));
        handle.append(&gtk4::Image::from_icon_name("list-drag-handle-symbolic"));
        let hint = gtk4::Label::new(Some(&tr("cap_drag", lang, &[])));
        hint.add_css_class("dim-label");
        hint.add_css_class("caption");
        handle.append(&hint);
        let source = gtk4::DragSource::new();
        source.set_actions(gdk::DragAction::COPY);
        source.connect_prepare({
            let weak = weak.clone();
            move |_, _, _| weak.upgrade().and_then(|ed| ed.drag_content())
        });
        handle.add_controller(source);
        actions.append(&handle);
        actions.append(&gtk4::Box::builder().hexpand(true).build());
        let copy_text = button(&tr("cap_copy_text", lang, &[]), "insert-text-symbolic");
        copy_text.connect_clicked({
            let weak = weak.clone();
            move |_| {
                if let Some(ed) = weak.upgrade() {
                    // the capture as it came, without the marks (`doc.image`)
                    ed.sa.read_text(ed.doc.texture.clone(), false);
                }
            }
        });
        let save = button(&tr("cap_save", lang, &[]), "document-save-symbolic");
        save.connect_clicked({
            let weak = weak.clone();
            move |_| {
                if let Some(ed) = weak.upgrade() {
                    ed.save();
                }
            }
        });
        let copy = button(&tr("cap_copy", lang, &[]), "edit-copy-symbolic");
        copy.add_css_class("suggested-action");
        copy.connect_clicked({
            let weak = weak.clone();
            move |_| {
                if let Some(ed) = weak.upgrade() {
                    ed.copy();
                }
            }
        });
        actions.append(&copy_text);
        actions.append(&save);
        actions.append(&copy);

        let frame = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
        frame.append(&toolbar);
        frame.set_vexpand(true);
        overlay.set_margin_start(14);
        overlay.set_margin_end(14);
        overlay.set_vexpand(true);
        frame.append(&overlay);
        frame.append(&actions);
        let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        root.add_css_class("background");
        root.append(&adw::HeaderBar::new());
        root.append(&frame);
        self.window.set_content(Some(&root));

        // Ctrl+Z, Ctrl+S, Ctrl+C and Esc
        let keys = gtk4::ShortcutController::new();
        let add = |keys: &gtk4::ShortcutController, trigger: &str, f: KeyAction| {
            let weak = weak.clone();
            let action = gtk4::CallbackAction::new(move |_, _| {
                if let Some(ed) = weak.upgrade() {
                    f(&ed);
                }
                glib::Propagation::Stop
            });
            keys.add_shortcut(gtk4::Shortcut::new(gtk4::ShortcutTrigger::parse_string(trigger), Some(action)));
        };
        add(&keys, "<Control>z", Box::new(|ed| ed.undo()));
        add(&keys, "<Control>s", Box::new(|ed| ed.save()));
        add(&keys, "<Control>c", Box::new(|ed| ed.copy()));
        add(&keys, "Escape", Box::new(|ed| ed.escape()));
        self.window.add_controller(keys);

        self.window.connect_close_request({
            let weak = weak.clone();
            move |_| {
                if let Some(ed) = weak.upgrade() {
                    ed.closed();
                }
                glib::Propagation::Proceed
            }
        });
    }

    fn select_tool(&self, tool: Tool) {
        self.commit_text();
        self.tool.set(tool);
        // the colors do not apply to the blur
        self.colors_box.set_sensitive(tool != Tool::Blur);
        self.colors_box.set_opacity(if tool == Tool::Blur { 0.35 } else { 1.0 });
    }

    fn dragging(&self, dx: f64, dy: f64) {
        let tool = self.tool.get();
        if tool == Tool::Text {
            return;
        }
        let (k, _, _) = self.layout();
        let start = self.start.get();
        let p = Point::new(start.x + dx / k, start.y + dy / k);
        let mut draft = self.draft.borrow_mut();
        if tool == Tool::Pen {
            let mut d = draft.take().unwrap_or_else(|| {
                let mut m = Mark::new(Tool::Pen, start, start, self.color.get());
                m.points = vec![start];
                m
            });
            d.points.push(p);
            d.b = p;
            *draft = Some(d);
        } else {
            *draft = Some(Mark::new(tool, start, p, self.color.get()));
        }
        drop(draft);
        self.area.queue_draw();
    }

    fn dragged(&self, _dx: f64, _dy: f64) {
        let start = self.start.get();
        if self.tool.get() == Tool::Text {
            self.commit_text();
            self.start_text(start);
            return;
        }
        let draft = self.draft.borrow_mut().take();
        if let Some(d) = draft {
            if d.is_kept() {
                self.doc.marks.borrow_mut().push(d);
            }
        }
        self.after_change();
    }

    /// The field to type the text of a mark: at `at`, bold, in the color of the mark.
    fn start_text(&self, at: Point) {
        let c = self.color.get();
        *self.typing.borrow_mut() = Some(Mark::new(Tool::Text, at, at, c));
        let (k, ox, oy) = self.layout();
        let size = capture::font_size(self.doc.w as f64, self.doc.h as f64) * k;
        self.entry_css.load_from_data(&format!(
            "entry.menew-cap-entry {{ font-weight: bold; font-size: {size:.1}px; color: rgb({},{},{}); background: transparent; border: none; box-shadow: none; padding: 0; min-height: 0; outline: none; }}",
            c.0, c.1, c.2
        ));
        self.entry.set_text("");
        self.entry.set_margin_start((ox + at.x * k) as i32);
        self.entry.set_margin_top((oy + at.y * k) as i32);
        self.entry.set_visible(true);
        self.entry.grab_focus();
        self.after_change();
    }

    /// `CaptureEditor.commitText`.
    fn commit_text(&self) {
        let Some(mut mark) = self.typing.borrow_mut().take() else { return };
        let typed = self.entry.text().to_string();
        self.entry.set_visible(false);
        self.entry.set_text("");
        if let Some(text) = capture::clean_typed_text(&typed) {
            mark.text = text;
            self.doc.marks.borrow_mut().push(mark);
        }
        self.after_change();
    }

    fn after_change(&self) {
        self.undo_btn.set_sensitive(!self.doc.marks.borrow().is_empty() || self.typing.borrow().is_some());
        self.area.queue_draw();
    }

    /// `CaptureEditor.undo`: the text being typed first, then the last mark.
    fn undo(&self) {
        if self.typing.borrow_mut().take().is_some() {
            self.entry.set_visible(false);
            self.entry.set_text("");
        } else {
            self.doc.marks.borrow_mut().pop();
        }
        self.after_change();
    }

    fn escape(&self) {
        if self.typing.borrow().is_some() {
            self.undo();
        } else {
            self.window.close();
        }
    }

    /// `ScreenCapture.copy`: the capture with its marks goes to the clipboard and the editor closes.
    fn copy(&self) {
        self.commit_text();
        match self.doc.render() {
            Ok(t) => {
                if let Some(c) = self.sa.clipboard() {
                    c.set_texture(&t);
                }
                self.sa.notify(&tr("cap_copied", ScreenApp::lang(), &[]));
                info!("Capture copied to the clipboard ({} x {})", self.doc.w, self.doc.h);
                self.window.close();
            }
            Err(e) => {
                warn!("Capture: cannot render: {}", e);
                self.sa.notify(&tr("cap_save_failed", ScreenApp::lang(), &[]));
            }
        }
    }

    /// `ScreenCapture.save`: a dialog that starts in the Desktop folder with "Screenshot <date>.png".
    #[allow(deprecated)]
    fn save(self: &Rc<Self>) {
        self.commit_text();
        let lang = ScreenApp::lang();
        let dialog = gtk4::FileChooserNative::new(Some(&tr("cap_save", lang, &[]).replace('…', "")), Some(&self.window), gtk4::FileChooserAction::Save, None, None);
        dialog.set_current_name(&capture::screenshot_file_name_now(lang));
        let _ = dialog.set_current_folder(Some(&gio::File::for_path(capture::default_save_dir())));
        let filter = gtk4::FileFilter::new();
        filter.add_mime_type("image/png");
        filter.set_name(Some("PNG"));
        dialog.add_filter(&filter);
        let weak = Rc::downgrade(self);
        dialog.connect_response(move |d, resp| {
            let Some(ed) = weak.upgrade() else { return };
            if resp == gtk4::ResponseType::Accept {
                if let Some(path) = d.file().and_then(|f| f.path()) {
                    let path = if path.extension().is_some() { path } else { path.with_extension("png") };
                    match ed.doc.export_png(&path) {
                        Ok(()) => {
                            info!("Capture saved to {}", path.display());
                            ed.sa.notify(&tr("cap_saved", lang, &[]));
                        }
                        Err(e) => {
                            warn!("Capture: cannot save {}: {}", path.display(), e);
                            ed.sa.notify(&tr("cap_save_failed", lang, &[]));
                        }
                    }
                }
            }
            ed.dialog.borrow_mut().take();
        });
        dialog.show();
        *self.dialog.borrow_mut() = Some(dialog);
    }

    /// `CaptureEditor.dragProvider`: a file called "Screenshot.png" (and the image itself) for the app it is dropped in.
    fn drag_content(&self) -> Option<gdk::ContentProvider> {
        self.commit_text();
        let dir = private_dir().join(format!("drag-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok()?;
        let path = dir.join(capture::drag_file_name(ScreenApp::lang()));
        self.doc.export_png(&path).ok()?;
        *self.drag_dir.borrow_mut() = Some(dir);
        // the address of the file (what a file manager or a browser takes on a drop) and the image itself
        let uri = gio::File::for_path(&path).uri();
        let list = glib::Bytes::from_owned(format!("{uri}\r\n").into_bytes());
        let texture = self.doc.render().ok()?;
        Some(gdk::ContentProvider::new_union(&[gdk::ContentProvider::for_bytes("text/uri-list", &list), gdk::ContentProvider::for_value(&texture.to_value())]))
    }

    fn closed(&self) {
        if let Some(dir) = self.drag_dir.borrow_mut().take() {
            let _ = std::fs::remove_dir_all(dir);
        }
        self.sa.editor_closed(self);
    }
}

fn button(label: &str, icon: &str) -> gtk4::Button {
    let b = gtk4::Button::new();
    b.set_child(Some(&adw::ButtonContent::builder().label(label).icon_name(icon).build()));
    b
}

/// The icon of a tool, drawn here (the symbols of the Mac have no equal in the icon themes): pen, arrow, rectangle,
/// text and a checkerboard for the blur.
fn tool_icon(tool: Tool) -> gtk4::DrawingArea {
    let area = gtk4::DrawingArea::builder().content_width(18).content_height(18).build();
    area.set_draw_func(move |a, cr, w, h| {
        #[allow(deprecated)]
        let c = a.style_context().color();
        cr.set_source_rgba(c.red() as f64, c.green() as f64, c.blue() as f64, c.alpha() as f64);
        cr.set_line_width(1.6);
        cr.set_line_cap(cairo::LineCap::Round);
        cr.set_line_join(cairo::LineJoin::Round);
        let (w, h) = (w as f64, h as f64);
        let (x0, y0, x1, y1) = (2.0, 2.0, w - 2.0, h - 2.0);
        match tool {
            Tool::Pen => {
                cr.move_to(x0, y1 - 3.0);
                cr.curve_to(x0 + 3.0, y0, x0 + 6.0, y1, x0 + 9.0, y0 + 5.0);
                cr.curve_to(x0 + 11.0, y0 + 2.0, x1 - 2.0, y1 - 4.0, x1, y0 + 2.0);
                let _ = cr.stroke();
            }
            Tool::Arrow => {
                cr.move_to(x0 + 1.0, y1 - 1.0);
                cr.line_to(x1 - 1.0, y0 + 1.0);
                cr.move_to(x1 - 7.0, y0 + 1.0);
                cr.line_to(x1 - 1.0, y0 + 1.0);
                cr.line_to(x1 - 1.0, y0 + 7.0);
                let _ = cr.stroke();
            }
            Tool::Rect => {
                cr.rectangle(x0 + 1.0, y0 + 2.0, x1 - x0 - 2.0, y1 - y0 - 4.0);
                let _ = cr.stroke();
            }
            Tool::Text => {
                cr.move_to(x0 + 2.0, y0 + 2.0);
                cr.line_to(x1 - 2.0, y0 + 2.0);
                cr.move_to(w / 2.0, y0 + 2.0);
                cr.line_to(w / 2.0, y1);
                let _ = cr.stroke();
            }
            Tool::Blur => {
                let s = (w - 4.0) / 4.0;
                for i in 0..4 {
                    for j in 0..4 {
                        if (i + j) % 2 == 0 {
                            cr.rectangle(2.0 + i as f64 * s, 2.0 + j as f64 * s, s, s);
                        }
                    }
                }
                let _ = cr.fill();
            }
        }
    });
    area
}

// ---------------------------------------------------------------- the QR window

struct QrWindow {
    sa: Weak<ScreenApp>,
    window: adw::Window,
    text: gtk4::Entry,
    name: gtk4::Entry,
    password: gtk4::PasswordEntry,
    area: gtk4::DrawingArea,
    hint: gtk4::Label,
    copy_btn: gtk4::Button,
    wifi: Cell<bool>,
    text_btn: gtk4::ToggleButton,
    wifi_btn: gtk4::ToggleButton,
    text_row: gtk4::Box,
    wifi_row: gtk4::Box,
}

impl QrWindow {
    fn new(sa: &Rc<ScreenApp>) -> Rc<QrWindow> {
        let lang = ScreenApp::lang();
        let window = adw::Window::builder().title(tr("act_qr_show", lang, &[])).default_width(360).resizable(false).build();
        let text = gtk4::Entry::builder().placeholder_text(tr("qr_text_or_link", lang, &[])).build();
        let name = gtk4::Entry::builder().placeholder_text(tr("qr_network", lang, &[])).build();
        let password = gtk4::PasswordEntry::builder().placeholder_text(tr("qr_password", lang, &[])).show_peek_icon(true).build();
        let area = gtk4::DrawingArea::builder().content_width(240).content_height(240).halign(gtk4::Align::Center).build();
        let hint = gtk4::Label::builder().wrap(true).justify(gtk4::Justification::Center).build();
        hint.add_css_class("dim-label");
        hint.add_css_class("caption");
        let copy_btn = button(&tr("qr_copy_image", lang, &[]), "edit-copy-symbolic");
        copy_btn.add_css_class("suggested-action");
        let text_btn = gtk4::ToggleButton::builder().label(tr("qr_text_or_link", lang, &[])).active(true).build();
        let wifi_btn = gtk4::ToggleButton::builder().label(tr("qr_wifi", lang, &[])).group(&text_btn).build();
        let text_row = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        text_row.append(&text);
        let wifi_row = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        wifi_row.append(&name);
        wifi_row.append(&password);
        let win = Rc::new(QrWindow { sa: Rc::downgrade(sa), window, text, name, password, area, hint, copy_btn, wifi: Cell::new(false), text_btn, wifi_btn, text_row, wifi_row });
        win.build();
        win
    }

    fn payload(&self) -> String {
        qr::window_payload(self.wifi.get(), self.text.text().as_str(), self.name.text().as_str(), self.password.text().as_str())
    }

    fn set_wifi(&self, wifi: bool) {
        self.wifi.set(wifi);
        self.text_btn.set_active(!wifi);
        self.wifi_btn.set_active(wifi);
        self.refresh();
    }

    fn refresh(&self) {
        let lang = ScreenApp::lang();
        let wifi = self.wifi.get();
        self.text_row.set_visible(!wifi);
        self.wifi_row.set_visible(wifi);
        self.hint.set_label(&tr(if wifi { "qr_hint_wifi" } else { "qr_hint_text" }, lang, &[]));
        // no more than a code can hold
        let payload = self.payload();
        self.copy_btn.set_sensitive(!payload.is_empty() && qr::encode(&payload).is_some());
        self.area.queue_draw();
    }

    fn build(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.area.set_draw_func({
            let weak = weak.clone();
            move |_, cr, w, h| {
                let Some(win) = weak.upgrade() else { return };
                let (w, h) = (w as f64, h as f64);
                let payload = win.payload();
                let rounded = |cr: &cairo::Context| {
                    use std::f64::consts::{FRAC_PI_2, PI};
                    let r = 14.0;
                    cr.new_sub_path();
                    cr.arc(w - r, r, r, -FRAC_PI_2, 0.0);
                    cr.arc(w - r, h - r, r, 0.0, FRAC_PI_2);
                    cr.arc(r, h - r, r, FRAC_PI_2, PI);
                    cr.arc(r, r, r, PI, 3.0 * FRAC_PI_2);
                    cr.close_path();
                };
                match qr::encode(&payload).filter(|_| !payload.is_empty()) {
                    Some(m) => {
                        rounded(cr);
                        cr.set_source_rgb(1.0, 1.0, 1.0);
                        let _ = cr.fill();
                        // whole pixels per module, so that the code stays sharp
                        let side = w.min(h) - 24.0;
                        let ms = (side / m.size as f64).floor().max(1.0);
                        let total = ms * m.size as f64;
                        let (ox, oy) = ((w - total) / 2.0, (h - total) / 2.0);
                        cr.set_source_rgb(0.0, 0.0, 0.0);
                        for y in 0..m.size {
                            for x in 0..m.size {
                                if m.dark(x, y) {
                                    cr.rectangle(ox + x as f64 * ms, oy + y as f64 * ms, ms, ms);
                                }
                            }
                        }
                        let _ = cr.fill();
                    }
                    None => {
                        rounded(cr);
                        cr.set_source_rgba(0.5, 0.5, 0.5, 0.15);
                        let _ = cr.fill();
                    }
                }
            }
        });
        for e in [&self.text, &self.name] {
            e.connect_changed({
                let weak = weak.clone();
                move |_| {
                    if let Some(w) = weak.upgrade() {
                        w.refresh();
                    }
                }
            });
        }
        self.password.connect_changed({
            let weak = weak.clone();
            move |_| {
                if let Some(w) = weak.upgrade() {
                    w.refresh();
                }
            }
        });
        self.text_btn.connect_toggled({
            let weak = weak.clone();
            move |b| {
                if b.is_active() {
                    if let Some(w) = weak.upgrade() {
                        w.wifi.set(false);
                        w.refresh();
                    }
                }
            }
        });
        self.wifi_btn.connect_toggled({
            let weak = weak.clone();
            move |b| {
                if b.is_active() {
                    if let Some(w) = weak.upgrade() {
                        w.wifi.set(true);
                        w.refresh();
                    }
                }
            }
        });
        self.copy_btn.connect_clicked({
            let weak = weak.clone();
            move |_| {
                let Some(win) = weak.upgrade() else { return };
                let Some(sa) = win.sa.upgrade() else { return };
                if let Some(t) = ScreenApp::qr_texture(&win.payload()) {
                    if let Some(c) = sa.clipboard() {
                        c.set_texture(&t);
                    }
                    sa.notify(&tr("qr_image_copied", ScreenApp::lang(), &[]));
                }
            }
        });
        let lang = ScreenApp::lang();
        let close = button(&tr("cap_close", lang, &[]), "window-close-symbolic");
        close.connect_clicked({
            let weak = weak.clone();
            move |_| {
                if let Some(w) = weak.upgrade() {
                    w.window.set_visible(false);
                }
            }
        });
        let modes = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        modes.add_css_class("linked");
        modes.set_halign(gtk4::Align::Center);
        modes.append(&self.text_btn);
        modes.append(&self.wifi_btn);
        let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        buttons.append(&close);
        buttons.append(&gtk4::Box::builder().hexpand(true).build());
        buttons.append(&self.copy_btn);
        let col = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        col.set_margin_start(18);
        col.set_margin_end(18);
        col.set_margin_top(12);
        col.set_margin_bottom(16);
        col.append(&modes);
        col.append(&self.area);
        col.append(&self.text_row);
        col.append(&self.wifi_row);
        col.append(&self.hint);
        col.append(&buttons);
        let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        root.add_css_class("background");
        root.append(&adw::HeaderBar::new());
        root.append(&col);
        self.window.set_content(Some(&root));
        // closing hides: the window and what was typed are used again next time (like the Mac panel)
        self.window.connect_close_request(|w| {
            w.set_visible(false);
            glib::Propagation::Stop
        });
        let keys = gtk4::ShortcutController::new();
        let w2 = weak.clone();
        keys.add_shortcut(gtk4::Shortcut::new(
            gtk4::ShortcutTrigger::parse_string("Escape"),
            Some(gtk4::CallbackAction::new(move |_, _| {
                if let Some(w) = w2.upgrade() {
                    w.window.set_visible(false);
                }
                glib::Propagation::Stop
            })),
        ));
        self.window.add_controller(keys);
        self.refresh();
    }
}
