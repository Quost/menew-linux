//! Screenshots and screen text (port of `Capture.swift`, `TextRecognizer.swift`, `QRCode.swift`).
//! The capture itself goes through the Screenshot portal (`menew-desktop`); this module has the
//! palette items, reading QR codes (`zbarimg`, zbar-tools), making them (`qrencode`) and the Wi-Fi
//! code of the current network (NetworkManager's `nmcli`). All of it runs locally.

use crate::i18n::Language;
use crate::palette::{Icon, ItemAction, Kind, PaletteItem};
use crate::strings::tr;
use std::path::Path;
use std::process::{Command, Stdio};

pub fn palette_items(raw: &str, clipboard_text: Option<&str>, lang: Language) -> Vec<PaletteItem> {
    let mut out = vec![
        PaletteItem::new("action.screenshot", Kind::Action, &tr("cap_area", lang, &[]))
            .subtitle(&tr("cap_area_sub", lang, &[]))
            .kw("print screenshot captura capturar tela imagem anotar annotate bildschirmfoto captura 截图 screen")
            .icon(Icon::Symbol("applets-screenshooter-symbolic"))
            .shortcut("Ctrl+Alt+S")
            .act(ItemAction::Command("cap.area".into())),
        PaletteItem::new("action.screenshotfull", Kind::Action, &tr("cap_full", lang, &[]))
            .kw("print screenshot tela inteira full screen captura ganzer bildschirm pantalla completa 全屏")
            .icon(Icon::Symbol("video-display-symbolic"))
            .act(ItemAction::Command("cap.full".into())),
        PaletteItem::new("action.ocr", Kind::Action, &tr("cap_ocr", lang, &[]))
            .kw("ocr texto text capturar capture scan ler read textsniper erkennen reconocer 文字 识别")
            .icon(Icon::Symbol("edit-select-all-symbolic"))
            .shortcut("Ctrl+Alt+O")
            .act(ItemAction::Command("cap.ocr".into())),
        PaletteItem::new("action.qrscan", Kind::Action, &tr("cap_qr_read", lang, &[]))
            .kw("qr code ler read scan escanear lesen leer 二维码 扫描")
            .icon(Icon::Symbol("camera-photo-symbolic"))
            .act(ItemAction::Command("cap.qr".into())),
        PaletteItem::new("action.qrwifi", Kind::Action, &tr("cap_qr_wifi", lang, &[]))
            .kw("qr wifi wi-fi rede network senha password teilen compartir 二维码 无线")
            .icon(Icon::Symbol("network-wireless-symbolic"))
            .act(ItemAction::Command("cap.qrwifi".into())),
    ];
    if let Some(t) = clipboard_text.map(str::trim).filter(|t| !t.is_empty() && t.len() <= 2000) {
        out.push(
            PaletteItem::new("action.qrclip", Kind::Action, &tr("cap_qr_show", lang, &[]))
                .subtitle(&crate::clip_item::collapsed_preview(t))
                .kw("qr code mostrar show gerar generate copiado copied zeigen mostrar 二维码 生成")
                .icon(Icon::Symbol("view-grid-symbolic"))
                .forget()
                .act(ItemAction::Command(format!("cap.qrshow:{t}"))),
        );
    }
    // "qr some text"
    let r = raw.trim();
    if let Some(text) = r.strip_prefix("qr ").or_else(|| r.strip_prefix("QR ")) {
        if !text.trim().is_empty() {
            out.push(
                PaletteItem::new("qr.typed", Kind::Action, &tr("cap_qr_for", lang, &[text.trim()]))
                    .kw(r)
                    .icon(Icon::Symbol("view-grid-symbolic"))
                    .forget()
                    .act(ItemAction::Command(format!("cap.qrshow:{}", text.trim()))),
            );
        }
    }
    out
}

/// Read every QR code (or barcode) in an image. `None` when `zbarimg` is missing.
pub fn read_codes(image: &Path) -> Option<Vec<String>> {
    if !crate::sys::has("zbarimg") {
        return None;
    }
    let out = Command::new("zbarimg").args(["--quiet", "--raw"]).arg(image).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    // exit 4 = nothing found
    Some(String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).filter(|l| !l.is_empty()).collect())
}

/// PNG bytes of the QR code for `text`. `None` when `qrencode` is missing.
pub fn make_qr_png(text: &str) -> Option<Vec<u8>> {
    if !crate::sys::has("qrencode") {
        return None;
    }
    let out = Command::new("qrencode").args(["-o", "-", "-t", "PNG", "-s", "10", "-m", "2", "--", text]).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    (out.status.success() && !out.stdout.is_empty()).then_some(out.stdout)
}

/// The text of a Wi-Fi QR code (`WIFI:T:WPA;S:name;P:pass;;`), escaping like the spec asks.
pub fn wifi_qr_text(ssid: &str, password: Option<&str>) -> String {
    let esc = |s: &str| s.chars().fold(String::new(), |mut o, c| {
        if matches!(c, '\\' | ';' | ',' | ':' | '"') {
            o.push('\\');
        }
        o.push(c);
        o
    });
    match password.filter(|p| !p.is_empty()) {
        Some(p) => format!("WIFI:T:WPA;S:{};P:{};;", esc(ssid), esc(p)),
        None => format!("WIFI:T:nopass;S:{};;", esc(ssid)),
    }
}

/// `nmcli -t -f active,ssid dev wifi` output: the network in use (`yes:Home`).
pub fn parse_active_ssid(out: &str) -> Option<String> {
    out.lines().find_map(|l| l.strip_prefix("yes:").map(|s| s.replace("\\:", ":"))).filter(|s| !s.is_empty())
}

/// The network in use and its password (NetworkManager may ask for the password itself).
pub fn current_wifi() -> Option<(String, Option<String>)> {
    let ssid = parse_active_ssid(&crate::sys::output("nmcli", &["-t", "-f", "active,ssid", "dev", "wifi"])?)?;
    let psk = crate::sys::output("nmcli", &["-s", "-g", "802-11-wireless-security.psk", "connection", "show", &ssid]).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    Some((ssid, psk))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wifi_codes() {
        assert_eq!(wifi_qr_text("Casa 5G", Some("a;b")), "WIFI:T:WPA;S:Casa 5G;P:a\\;b;;");
        assert_eq!(wifi_qr_text("Open", None), "WIFI:T:nopass;S:Open;;");
        assert_eq!(parse_active_ssid("no:Neighbor\nyes:My\\:Net\n").as_deref(), Some("My:Net"));
        assert_eq!(parse_active_ssid("no:x\n"), None);
    }

    #[test]
    fn palette_items_for_the_clipboard_and_typed_qr() {
        let it = palette_items("qr hello", Some("https://x.y"), Language::En);
        assert!(it.iter().any(|i| i.action == ItemAction::Command("cap.qrshow:https://x.y".into())));
        assert!(it.iter().any(|i| i.action == ItemAction::Command("cap.qrshow:hello".into())));
        assert!(it.iter().any(|i| i.action == ItemAction::Command("cap.ocr".into())));
    }
}
