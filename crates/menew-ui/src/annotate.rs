//! The screenshot editor (port of the annotation window of `Capture.swift`): pen, arrows,
//! rectangles, text and blur over the capture, then copy, save or drag it out. Also the window that
//! shows a QR code.

use crate::dialogs;
use gtk4::gdk;
use gtk4::gdk_pixbuf::{InterpType, Pixbuf};
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use menew_core::current_language;
use menew_core::strings::tr;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tool {
    Pen,
    Arrow,
    Rect,
    Text,
    Blur,
}

#[derive(Debug, Clone)]
enum Shape {
    Pen(Vec<(f64, f64)>, (f64, f64, f64)),
    Arrow((f64, f64), (f64, f64), (f64, f64, f64)),
    Rect((f64, f64), (f64, f64), (f64, f64, f64)),
    Text((f64, f64), String, (f64, f64, f64)),
    Blur((f64, f64), (f64, f64)),
}

const COLORS: [(f64, f64, f64); 5] = [(0.92, 0.2, 0.2), (1.0, 0.75, 0.0), (0.2, 0.7, 0.3), (0.2, 0.5, 0.95), (0.1, 0.1, 0.1)];

struct Editor {
    image: Pixbuf,
    shapes: RefCell<Vec<Shape>>,
    current: RefCell<Option<Shape>>,
    tool: Cell<Tool>,
    color: Cell<(f64, f64, f64)>,
    area: gtk4::DrawingArea,
    window: adw::ApplicationWindow,
    /// Scale from image pixels to widget pixels and the offset of the image in the widget.
    view: Cell<(f64, f64, f64)>,
}

/// Pixelate one rectangle of the image (in image coordinates) and paint it.
fn paint_blur(cr: &gtk4::cairo::Context, image: &Pixbuf, a: (f64, f64), b: (f64, f64)) {
    let x = a.0.min(b.0).max(0.0) as i32;
    let y = a.1.min(b.1).max(0.0) as i32;
    let w = ((a.0 - b.0).abs() as i32).min(image.width() - x);
    let h = ((a.1 - b.1).abs() as i32).min(image.height() - y);
    if w < 2 || h < 2 {
        return;
    }
    let sub = image.new_subpixbuf(x, y, w, h);
    let block = 12;
    let Some(small) = sub.scale_simple((w / block).max(1), (h / block).max(1), InterpType::Bilinear) else { return };
    let Some(big) = small.scale_simple(w, h, InterpType::Nearest) else { return };
    cr.save().ok();
    cr.set_source_pixbuf(&big, x as f64, y as f64);
    cr.rectangle(x as f64, y as f64, w as f64, h as f64);
    let _ = cr.fill();
    cr.restore().ok();
}

/// Draw the shapes in image coordinates (the caller sets the scale).
fn paint(cr: &gtk4::cairo::Context, image: &Pixbuf, shapes: &[Shape]) {
    cr.set_source_pixbuf(image, 0.0, 0.0);
    let _ = cr.paint();
    let line = (image.width().max(image.height()) as f64 / 300.0).clamp(3.0, 10.0);
    cr.set_line_cap(gtk4::cairo::LineCap::Round);
    cr.set_line_join(gtk4::cairo::LineJoin::Round);
    for s in shapes {
        match s {
            Shape::Blur(a, b) => paint_blur(cr, image, *a, *b),
            Shape::Pen(pts, c) => {
                cr.set_source_rgb(c.0, c.1, c.2);
                cr.set_line_width(line);
                if let Some(first) = pts.first() {
                    cr.move_to(first.0, first.1);
                    for p in pts.iter().skip(1) {
                        cr.line_to(p.0, p.1);
                    }
                    let _ = cr.stroke();
                }
            }
            Shape::Rect(a, b, c) => {
                cr.set_source_rgb(c.0, c.1, c.2);
                cr.set_line_width(line);
                cr.rectangle(a.0.min(b.0), a.1.min(b.1), (a.0 - b.0).abs(), (a.1 - b.1).abs());
                let _ = cr.stroke();
            }
            Shape::Arrow(a, b, c) => {
                cr.set_source_rgb(c.0, c.1, c.2);
                cr.set_line_width(line);
                cr.move_to(a.0, a.1);
                cr.line_to(b.0, b.1);
                let _ = cr.stroke();
                let ang = (b.1 - a.1).atan2(b.0 - a.0);
                let head = line * 4.5;
                cr.move_to(b.0, b.1);
                cr.line_to(b.0 - head * (ang - 0.45).cos(), b.1 - head * (ang - 0.45).sin());
                cr.line_to(b.0 - head * (ang + 0.45).cos(), b.1 - head * (ang + 0.45).sin());
                cr.close_path();
                let _ = cr.fill();
            }
            Shape::Text(p, t, c) => {
                let size = line * 7.0;
                cr.select_font_face("Sans", gtk4::cairo::FontSlant::Normal, gtk4::cairo::FontWeight::Bold);
                cr.set_font_size(size);
                // a light outline keeps the text readable on any background
                cr.move_to(p.0, p.1);
                cr.text_path(t);
                cr.set_source_rgb(1.0, 1.0, 1.0);
                cr.set_line_width(line);
                let _ = cr.stroke_preserve();
                cr.set_source_rgb(c.0, c.1, c.2);
                let _ = cr.fill();
            }
        }
    }
}

impl Editor {
    fn to_image(&self, x: f64, y: f64) -> (f64, f64) {
        let (s, ox, oy) = self.view.get();
        ((x - ox) / s, (y - oy) / s)
    }

    /// The image with the annotations, at full size.
    fn texture(&self) -> Option<gdk::Texture> {
        let (w, h) = (self.image.width(), self.image.height());
        let mut surface = gtk4::cairo::ImageSurface::create(gtk4::cairo::Format::ARgb32, w, h).ok()?;
        {
            let cr = gtk4::cairo::Context::new(&surface).ok()?;
            paint(&cr, &self.image, &self.shapes.borrow());
        }
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().ok()?.to_vec();
        // cairo's ARGB32 is native-endian premultiplied: BGRA in memory on little-endian machines
        let format = if cfg!(target_endian = "little") { gdk::MemoryFormat::B8g8r8a8Premultiplied } else { gdk::MemoryFormat::A8r8g8b8Premultiplied };
        Some(gdk::MemoryTexture::new(w, h, format, &glib::Bytes::from_owned(data), stride).upcast())
    }

    fn png_bytes(&self) -> Option<Vec<u8>> {
        Some(self.texture()?.save_to_png_bytes().to_vec())
    }
}

fn notify(text: &str) {
    if let Some(app) = gio::Application::default() {
        app.send_notification(Some("menew-capture"), &gio::Notification::new(text));
    }
}

/// Open the editor on a capture. `ocr` reads the text of the image (for the "Copy text" button).
pub fn open_editor(app: &adw::Application, path: &Path, ocr: Rc<dyn Fn(PathBuf)>) {
    let lang = current_language();
    let Ok(image) = Pixbuf::from_file(path) else {
        notify(&tr("cap_failed", lang, &[]));
        return;
    };
    let window = adw::ApplicationWindow::builder().application(app).title(tr("ann_title", lang, &[])).default_width((image.width() + 40).clamp(480, 1400)).default_height((image.height() + 120).clamp(360, 900)).build();
    let area = gtk4::DrawingArea::builder().hexpand(true).vexpand(true).build();
    let ed = Rc::new(Editor {
        image,
        shapes: RefCell::new(vec![]),
        current: RefCell::new(None),
        tool: Cell::new(Tool::Arrow),
        color: Cell::new(COLORS[0]),
        area: area.clone(),
        window: window.clone(),
        view: Cell::new((1.0, 0.0, 0.0)),
    });

    let e = Rc::clone(&ed);
    area.set_draw_func(move |_, cr, w, h| {
        let (iw, ih) = (e.image.width() as f64, e.image.height() as f64);
        let s = (w as f64 / iw).min(h as f64 / ih).min(1.0);
        let (ox, oy) = ((w as f64 - iw * s) / 2.0, (h as f64 - ih * s) / 2.0);
        e.view.set((s, ox, oy));
        cr.translate(ox, oy);
        cr.scale(s, s);
        let mut all = e.shapes.borrow().clone();
        if let Some(c) = e.current.borrow().clone() {
            all.push(c);
        }
        paint(cr, &e.image, &all);
    });

    let drag = gtk4::GestureDrag::new();
    let e = Rc::clone(&ed);
    drag.connect_drag_begin(move |_, x, y| {
        let p = e.to_image(x, y);
        let c = e.color.get();
        let shape = match e.tool.get() {
            Tool::Pen => Some(Shape::Pen(vec![p], c)),
            Tool::Arrow => Some(Shape::Arrow(p, p, c)),
            Tool::Rect => Some(Shape::Rect(p, p, c)),
            Tool::Blur => Some(Shape::Blur(p, p)),
            Tool::Text => None,
        };
        *e.current.borrow_mut() = shape;
    });
    let e = Rc::clone(&ed);
    drag.connect_drag_update(move |g, dx, dy| {
        let Some((sx, sy)) = g.start_point() else { return };
        let p = e.to_image(sx + dx, sy + dy);
        if let Some(s) = e.current.borrow_mut().as_mut() {
            match s {
                Shape::Pen(pts, _) => pts.push(p),
                Shape::Arrow(_, b, _) | Shape::Rect(_, b, _) | Shape::Blur(_, b) => *b = p,
                Shape::Text(..) => {}
            }
        }
        e.area.queue_draw();
    });
    let e = Rc::clone(&ed);
    drag.connect_drag_end(move |g, dx, dy| {
        if let Some(s) = e.current.borrow_mut().take() {
            e.shapes.borrow_mut().push(s);
        } else if e.tool.get() == Tool::Text && dx.abs() < 4.0 && dy.abs() < 4.0 {
            let Some((sx, sy)) = g.start_point() else { return };
            let p = e.to_image(sx, sy);
            let c = e.color.get();
            let e2 = Rc::clone(&e);
            let lang = current_language();
            dialogs::ask_text(Some(e.window.upcast_ref()), &tr("ann_text_ask", lang, &[]), "", &tr("ann_add", lang, &[]), &tr("cancel", lang, &[]), move |t| {
                if !t.trim().is_empty() {
                    e2.shapes.borrow_mut().push(Shape::Text(p, t, c));
                    e2.area.queue_draw();
                }
            });
        }
        e.area.queue_draw();
    });
    area.add_controller(drag);

    // ---- toolbar ----
    let header = adw::HeaderBar::new();
    let tools = gtk4::Box::builder().css_classes(["linked"]).build();
    let mut first: Option<gtk4::ToggleButton> = None;
    for (tool, icon, key) in [
        (Tool::Arrow, "go-next-symbolic", "ann_arrow"),
        (Tool::Rect, "checkbox-symbolic", "ann_rect"),
        (Tool::Pen, "document-edit-symbolic", "ann_pen"),
        (Tool::Text, "insert-text-symbolic", "ann_text"),
        (Tool::Blur, "view-conceal-symbolic", "ann_blur"),
    ] {
        let b = gtk4::ToggleButton::builder().icon_name(icon).tooltip_text(tr(key, lang, &[])).build();
        if let Some(f) = &first {
            b.set_group(Some(f));
        } else {
            b.set_active(true);
            first = Some(b.clone());
        }
        let e = Rc::clone(&ed);
        b.connect_toggled(move |b| {
            if b.is_active() {
                e.tool.set(tool);
            }
        });
        tools.append(&b);
    }
    header.pack_start(&tools);
    let colors = gtk4::Box::builder().css_classes(["linked"]).margin_start(6).build();
    let mut first_c: Option<gtk4::ToggleButton> = None;
    for c in COLORS {
        let sw = gtk4::DrawingArea::builder().content_width(14).content_height(14).build();
        sw.set_draw_func(move |_, cr, w, h| {
            cr.set_source_rgb(c.0, c.1, c.2);
            cr.arc(w as f64 / 2.0, h as f64 / 2.0, w.min(h) as f64 / 2.0, 0.0, std::f64::consts::TAU);
            let _ = cr.fill();
        });
        let b = gtk4::ToggleButton::builder().child(&sw).build();
        if let Some(f) = &first_c {
            b.set_group(Some(f));
        } else {
            b.set_active(true);
            first_c = Some(b.clone());
        }
        let e = Rc::clone(&ed);
        b.connect_toggled(move |b| {
            if b.is_active() {
                e.color.set(c);
            }
        });
        colors.append(&b);
    }
    header.pack_start(&colors);
    let undo = gtk4::Button::builder().icon_name("edit-undo-symbolic").tooltip_text(tr("ann_undo", lang, &[])).build();
    let e = Rc::clone(&ed);
    undo.connect_clicked(move |_| {
        e.shapes.borrow_mut().pop();
        e.area.queue_draw();
    });
    header.pack_start(&undo);

    let save = gtk4::Button::builder().label(tr("ann_save", lang, &[])).build();
    let copy = gtk4::Button::builder().label(tr("ann_copy", lang, &[])).css_classes(["suggested-action"]).build();
    let text_btn = gtk4::Button::builder().icon_name("edit-select-all-symbolic").tooltip_text(tr("ann_copy_text", lang, &[])).build();
    header.pack_end(&copy);
    header.pack_end(&save);
    header.pack_end(&text_btn);

    let e = Rc::clone(&ed);
    copy.connect_clicked(move |_| {
        if let (Some(t), Some(d)) = (e.texture(), gdk::Display::default()) {
            d.clipboard().set_texture(&t);
            notify(&tr("ann_copied", current_language(), &[]));
            e.window.close();
        }
    });
    let e = Rc::clone(&ed);
    save.connect_clicked(move |_| {
        let lang = current_language();
        let stamp = chrono::Local::now().format("%Y-%m-%d %H.%M.%S");
        let dialog = gtk4::FileDialog::builder().title(tr("ann_save", lang, &[])).initial_name(format!("Menew {stamp}.png")).modal(true).build();
        if let Some(dir) = glib::user_special_dir(glib::UserDirectory::Pictures) {
            dialog.set_initial_folder(Some(&gio::File::for_path(dir)));
        }
        let e2 = Rc::clone(&e);
        dialog.save(Some(&e.window), gio::Cancellable::NONE, move |res| {
            let Ok(file) = res else { return };
            let Some(path) = file.path() else { return };
            if let Some(bytes) = e2.png_bytes() {
                if std::fs::write(&path, bytes).is_ok() {
                    notify(&tr("ann_saved", current_language(), &[&path.display().to_string()]));
                }
            }
        });
    });
    let p = path.to_path_buf();
    text_btn.connect_clicked(move |_| ocr(p.clone()));

    // drag the annotated image out as a PNG file
    let src = gtk4::DragSource::builder().actions(gdk::DragAction::COPY).build();
    let e = Rc::clone(&ed);
    src.connect_prepare(move |_, _, _| {
        let bytes = e.png_bytes()?;
        let dir = glib::user_cache_dir().join("menew").join("captures");
        std::fs::create_dir_all(&dir).ok()?;
        let file = dir.join(format!("Menew {}.png", chrono::Local::now().format("%Y-%m-%d %H.%M.%S")));
        std::fs::write(&file, bytes).ok()?;
        let list = gdk::FileList::from_array(&[gio::File::for_path(&file)]);
        Some(gdk::ContentProvider::for_value(&list.to_value()))
    });
    // dragging out only from the hint row, so drawing on the image still works
    let hint = gtk4::Label::builder().label(tr("ann_drag_hint", lang, &[])).css_classes(["dim-label", "caption"]).margin_top(6).margin_bottom(6).build();
    hint.add_controller(src);

    let body = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).build();
    body.append(&area);
    body.append(&hint);
    let tv = adw::ToolbarView::new();
    tv.add_top_bar(&header);
    tv.set_content(Some(&body));
    window.set_content(Some(&tv));
    let esc = gtk4::EventControllerKey::new();
    let w = window.clone();
    let e = Rc::clone(&ed);
    esc.connect_key_pressed(move |_, key, _, mods| {
        if key == gdk::Key::Escape {
            w.close();
            return glib::Propagation::Stop;
        }
        if mods.contains(gdk::ModifierType::CONTROL_MASK) && (key == gdk::Key::z || key == gdk::Key::Z) {
            e.shapes.borrow_mut().pop();
            e.area.queue_draw();
            return glib::Propagation::Stop;
        }
        if mods.contains(gdk::ModifierType::CONTROL_MASK) && (key == gdk::Key::c || key == gdk::Key::C) {
            copy.emit_clicked();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(esc);
    window.present();
}

/// A window with a QR code, big enough to scan from a phone.
pub fn show_qr(app: &adw::Application, png: &[u8], caption: &str) {
    let lang = current_language();
    let Ok(tex) = gdk::Texture::from_bytes(&glib::Bytes::from(png)) else { return };
    let pic = gtk4::Picture::builder().paintable(&tex).can_shrink(true).content_fit(gtk4::ContentFit::Contain).width_request(320).height_request(320).build();
    pic.add_css_class("menew-qr");
    let body = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(12).margin_top(12).margin_bottom(18).margin_start(18).margin_end(18).build();
    body.append(&pic);
    body.append(&gtk4::Label::builder().label(caption).wrap(true).max_width_chars(40).selectable(true).build());
    body.append(&gtk4::Label::builder().label(tr("cap_qr_scan_hint", lang, &[])).css_classes(["dim-label"]).build());
    let tv = adw::ToolbarView::new();
    tv.add_top_bar(&adw::HeaderBar::new());
    tv.set_content(Some(&body));
    let w = adw::ApplicationWindow::builder().application(app).title(tr("cap_qr_title", lang, &[])).default_width(400).default_height(500).content(&tv).build();
    w.present();
}
