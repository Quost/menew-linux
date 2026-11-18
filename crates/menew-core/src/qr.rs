//! QR codes: read them from a capture and make them for a text, a link or a Wi-Fi network. The port of
//! `Sources/Menew/QRCode.swift` (Vision to read, Core Image to make); here `rqrr` reads and `qrcodegen` makes,
//! both pure Rust. A link is never opened: reading a code only gives text back (the Mac says the same: "um QR
//! pode enganar"). The vectors are in `docs/screenshots-spec.md`.

/// `QRCode.wifiPayload`: the text that the cameras of iPhone and Android understand to join a network.
/// `\`, `;`, `,`, `:` and `"` are escaped with a backslash. No password means an open network (`T:nopass`).
pub fn wifi_payload(name: &str, password: &str) -> String {
    fn esc(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            if matches!(c, '\\' | ';' | ',' | ':' | '"') {
                out.push('\\');
            }
            out.push(c);
        }
        out
    }
    if password.is_empty() {
        format!("WIFI:T:nopass;S:{};;", esc(name))
    } else {
        format!("WIFI:T:WPA;S:{};P:{};;", esc(name), esc(password))
    }
}

/// The payload of the QR window (`QRModel.payload`): the text trimmed, or the Wi-Fi payload (empty without a name).
pub fn window_payload(wifi: bool, text: &str, name: &str, password: &str) -> String {
    if wifi {
        if name.is_empty() {
            String::new()
        } else {
            wifi_payload(name, password)
        }
    } else {
        text.trim().to_string()
    }
}

/// A QR code: `size x size` modules, row by row, `true` is dark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrMatrix {
    pub size: usize,
    pub modules: Vec<bool>,
}

impl QrMatrix {
    pub fn dark(&self, x: usize, y: usize) -> bool {
        self.modules[y * self.size + x]
    }
}

/// `CIQRCodeGenerator` with correction level `M` (the Mac's setting). `None` when the text does not fit in a code
/// (about 2300 bytes at this level).
pub fn encode(text: &str) -> Option<QrMatrix> {
    // the level stays M: `encode_text` would raise it when the code has room
    let segs = qrcodegen::QrSegment::make_segments(text);
    let code = qrcodegen::QrCode::encode_segments_advanced(&segs, qrcodegen::QrCodeEcc::Medium, qrcodegen::Version::MIN, qrcodegen::Version::MAX, None, false).ok()?;
    let size = code.size() as usize;
    let mut modules = Vec::with_capacity(size * size);
    for y in 0..size as i32 {
        for x in 0..size as i32 {
            modules.push(code.get_module(x, y));
        }
    }
    Some(QrMatrix { size, modules })
}

/// Width in modules of the white border around the code in the images made here. (The Mac copies the bare code
/// and draws the border in its window; a border in the image makes it readable when pasted on a dark page.)
pub const QUIET_ZONE: usize = 4;

/// A gray image (0 = black, 255 = white) of the code with `QUIET_ZONE` modules around it, every module `scale`
/// pixels wide. Returns `(width, height, pixels)`. For the 1024 pixels of the Mac use [`scale_for`].
pub fn render_gray(m: &QrMatrix, scale: usize) -> (usize, usize, Vec<u8>) {
    let modules = m.size + 2 * QUIET_ZONE;
    let side = modules * scale;
    let mut px = vec![255u8; side * side];
    for y in 0..m.size {
        for x in 0..m.size {
            if m.dark(x, y) {
                for dy in 0..scale {
                    let row = ((y + QUIET_ZONE) * scale + dy) * side + (x + QUIET_ZONE) * scale;
                    px[row..row + scale].fill(0);
                }
            }
        }
    }
    (side, side, px)
}

/// The module size that makes the image at least `target` pixels wide (`QRCode.image(for:size:)` uses 1024).
pub fn scale_for(m: &QrMatrix, target: usize) -> usize {
    let modules = m.size + 2 * QUIET_ZONE;
    target.div_ceil(modules).max(1)
}

/// The text of the QR codes in a gray image, in the order found. Dark codes on a light page and the reverse are
/// both tried.
pub fn decode_gray(width: usize, height: usize, pixels: &[u8]) -> Vec<String> {
    assert_eq!(pixels.len(), width * height, "decode_gray: the buffer does not match the size");
    let read = |invert: bool| -> Vec<String> {
        let mut img = rqrr::PreparedImage::prepare_from_greyscale(width, height, |x, y| {
            let v = pixels[y * width + x];
            if invert { 255 - v } else { v }
        });
        img.detect_grids().into_iter().filter_map(|g| g.decode().ok()).map(|(_, content)| content).collect()
    };
    let found = read(false);
    if found.is_empty() {
        read(true)
    } else {
        found
    }
}

/// `decode_gray` for a 4 bytes per pixel image in the order blue, green, red, alpha (Cairo and GDK on little
/// endian machines); the gray is the usual weighting of the three colors.
pub fn decode_bgra(width: usize, height: usize, data: &[u8]) -> Vec<String> {
    assert_eq!(data.len(), width * height * 4, "decode_bgra: the buffer does not match the size");
    let gray: Vec<u8> = (0..width * height)
        .map(|i| {
            let p = &data[i * 4..i * 4 + 3];
            ((p[0] as u32 * 29 + p[1] as u32 * 150 + p[2] as u32 * 77) / 256) as u8
        })
        .collect();
    decode_gray(width, height, &gray)
}

/// What `QRCode.scanScreen` does with what it found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanOutcome {
    /// "No QR code found".
    Nothing,
    /// The first code is a web link: "QR link copied". It is copied, never opened.
    Link(String),
    /// Anything else (text, Wi-Fi, a contact...): "QR content copied".
    Content(String),
}

impl ScanOutcome {
    /// The text that goes to the clipboard, if any. A scan never does anything else with the content.
    pub fn clipboard_text(&self) -> Option<&str> {
        match self {
            ScanOutcome::Nothing => None,
            ScanOutcome::Link(t) | ScanOutcome::Content(t) => Some(t),
        }
    }
}

/// `URL(string: first)?.scheme?.hasPrefix("http") == true`: an address (no spaces or control characters) whose
/// scheme starts with `http`.
pub fn is_link(s: &str) -> bool {
    if s.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    match s.split_once(':') {
        Some((scheme, _)) => {
            scheme.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false)
                && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
                && scheme.to_ascii_lowercase().starts_with("http")
        }
        None => false,
    }
}

pub fn scan_outcome(found: &[String]) -> ScanOutcome {
    match found.first() {
        None => ScanOutcome::Nothing,
        Some(first) if is_link(first) => ScanOutcome::Link(first.clone()),
        Some(first) => ScanOutcome::Content(first.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_made_here_is_read_back() {
        for text in ["https://example.com/a?b=c", "Hello", "ação 你好 ✓", &"x".repeat(300)] {
            let m = encode(text).unwrap();
            let (w, h, px) = render_gray(&m, 4);
            assert_eq!(decode_gray(w, h, &px), vec![text.to_string()], "{text}");
        }
    }

    #[test]
    fn a_dark_page_is_read_too() {
        let m = encode("night").unwrap();
        let (w, h, mut px) = render_gray(&m, 5);
        px.iter_mut().for_each(|p| *p = 255 - *p);
        assert_eq!(decode_gray(w, h, &px), vec!["night".to_string()]);
    }

    #[test]
    fn no_code_in_a_blank_image() {
        assert!(decode_gray(64, 64, &vec![255u8; 64 * 64]).is_empty());
    }
}
