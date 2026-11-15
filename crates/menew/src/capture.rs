//! Screenshot commands (`cap.*`): capture through the portal, then the editor, the screen text or
//! the QR code reader; and the QR code window.

use crate::commands::Commands;
use gtk4::gio;
use gtk4::glib;
use libadwaita as adw;
use menew_core::strings::tr;
use menew_core::{capture, current_language, ocr};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use tracing::{info, warn};

fn notify(palette: &menew_ui::PaletteWindow, key: &str, args: &[&str]) {
    palette.notify("menew-capture", &tr(key, current_language(), args), None);
}

/// Read the text of an image and copy it.
fn copy_text_of(palette: Rc<menew_ui::PaletteWindow>, path: PathBuf, delete_after: bool) {
    glib::spawn_future_local(async move {
        if !ocr::available() {
            notify(&palette, "cap_ocr_missing", &[]);
            return;
        }
        let p = path.clone();
        let text = gio::spawn_blocking(move || ocr::recognize(&p)).await;
        if delete_after {
            let _ = std::fs::remove_file(&path);
        }
        match text {
            Ok(Ok(t)) if !t.trim().is_empty() => {
                palette.copy_to_clipboard(t.trim());
                notify(&palette, "cap_ocr_done", &[]);
            }
            Ok(Err(e)) => {
                warn!("OCR: {}", e);
                notify(&palette, "cap_ocr_none", &[]);
            }
            _ => notify(&palette, "cap_ocr_none", &[]),
        }
    });
}

fn show_qr(app: &adw::Application, palette: &menew_ui::PaletteWindow, text: &str, caption: &str) {
    match capture::make_qr_png(text) {
        Some(png) => menew_ui::annotate::show_qr(app, &png, caption),
        None => notify(palette, "cap_qr_make_missing", &[]),
    }
}

pub fn setup(app: &adw::Application, cmds: &Rc<Commands>, palette: &Rc<menew_ui::PaletteWindow>, rt: Arc<tokio::runtime::Runtime>) {
    let p = Rc::clone(palette);
    cmds.add_provider("actions", Rc::new(move |raw, lang| capture::palette_items(raw, p.clipboard_text_now().as_deref(), lang)));
    let app = app.clone();
    let p = Rc::clone(palette);
    cmds.add_runner(Rc::new(move |cmd| {
        let (name, arg) = cmd.split_once(':').map(|(a, b)| (a, Some(b))).unwrap_or((cmd, None));
        match name {
            "cap.area" | "cap.full" | "cap.ocr" | "cap.qr" => {
                p.hide();
                let interactive = name != "cap.full";
                let what = name.to_string();
                let (app, p, rt) = (app.clone(), Rc::clone(&p), Arc::clone(&rt));
                // let the palette disappear before the desktop takes the picture
                glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                    let job = rt.spawn(menew_desktop::screenshot(interactive));
                    glib::spawn_future_local(async move {
                        let path = match job.await {
                            Ok(Ok(path)) => path,
                            Ok(Err(e)) => {
                                warn!("Screenshot: {}", e);
                                notify(&p, "cap_failed", &[]);
                                return;
                            }
                            Err(_) => return,
                        };
                        info!("Screenshot at {}", path.display());
                        match what.as_str() {
                            "cap.ocr" => copy_text_of(Rc::clone(&p), path, true),
                            "cap.qr" => {
                                let p2 = path.clone();
                                let codes = gio::spawn_blocking(move || capture::read_codes(&p2)).await.ok().flatten();
                                let _ = std::fs::remove_file(&path);
                                match codes {
                                    None => notify(&p, "cap_qr_missing", &[]),
                                    Some(c) if c.is_empty() => notify(&p, "cap_qr_none", &[]),
                                    Some(c) => {
                                        let first = c[0].clone();
                                        if first.starts_with("http://") || first.starts_with("https://") {
                                            p.open_uri(&first);
                                        }
                                        p.copy_to_clipboard(&first);
                                        notify(&p, "cap_qr_found", &[&first]);
                                    }
                                }
                            }
                            _ => {
                                let p2 = Rc::clone(&p);
                                menew_ui::annotate::open_editor(&app, &path, Rc::new(move |img| copy_text_of(Rc::clone(&p2), img, false)));
                            }
                        }
                    });
                });
                true
            }
            "cap.edit" => {
                // open the editor on an image file (`menew run cap.edit:/path/shot.png`)
                let path = PathBuf::from(arg.unwrap_or_default());
                let p2 = Rc::clone(&p);
                menew_ui::annotate::open_editor(&app, &path, Rc::new(move |img| copy_text_of(Rc::clone(&p2), img, false)));
                true
            }
            "cap.qrshow" => {
                let text = arg.unwrap_or_default();
                p.hide();
                show_qr(&app, &p, text, text);
                true
            }
            "cap.qrwifi" => {
                p.hide();
                let (app, p) = (app.clone(), Rc::clone(&p));
                glib::spawn_future_local(async move {
                    match gio::spawn_blocking(capture::current_wifi).await.ok().flatten() {
                        Some((ssid, psk)) => show_qr(&app, &p, &capture::wifi_qr_text(&ssid, psk.as_deref()), &ssid),
                        None => notify(&p, "cap_qr_wifi_none", &[]),
                    }
                });
                true
            }
            _ => false,
        }
    }));
}
