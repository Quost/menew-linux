//! Screen capture rules shared by the editor, the text reader and the QR reader: the marks of the annotation
//! editor, the blur (pixelation), file names and the outcome of "copy text from the screen". The port of
//! `Sources/Menew/Capture.swift` and `TextRecognizer.swift`; the vectors are in `docs/screenshots-spec.md`.
//! Nothing here touches the screen, the clipboard or the network.

use crate::i18n::Language;
use std::path::PathBuf;

/// The tools of the editor (`CaptureMark.Kind`), in the order of the Mac toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Pen,
    Arrow,
    Rect,
    Text,
    Blur,
}

pub const TOOLS: [Tool; 5] = [Tool::Pen, Tool::Arrow, Tool::Rect, Tool::Text, Tool::Blur];

/// The tool selected when the editor opens.
pub const DEFAULT_TOOL: Tool = Tool::Arrow;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// The eight colors of the editor, in order (SwiftUI `.red`, `.orange`, `.yellow`, `.green`, `.blue`, `.purple`,
/// `.black`, `.white`; the system values of macOS in light appearance).
pub const COLORS: [Rgb; 8] = [
    Rgb(255, 59, 48),
    Rgb(255, 149, 0),
    Rgb(255, 204, 0),
    Rgb(52, 199, 89),
    Rgb(0, 122, 255),
    Rgb(175, 82, 222),
    Rgb(0, 0, 0),
    Rgb(255, 255, 255),
];

/// The color selected when the editor opens (red).
pub const DEFAULT_COLOR: Rgb = COLORS[0];
pub const BLACK: Rgb = COLORS[6];

/// What `CaptureMark` holds. Points are in pixels of the image.
#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    pub kind: Tool,
    pub a: Point,
    pub b: Point,
    pub color: Rgb,
    pub text: String,
    /// The freehand stroke of the pen.
    pub points: Vec<Point>,
}

impl Mark {
    pub fn new(kind: Tool, a: Point, b: Point, color: Rgb) -> Self {
        Self { kind, a, b, color, text: String::new(), points: vec![] }
    }

    /// `CaptureMark.rect`: the box between `a` and `b`, as `(x, y, width, height)`.
    pub fn rect(&self) -> (f64, f64, f64, f64) {
        (self.a.x.min(self.b.x), self.a.y.min(self.b.y), (self.b.x - self.a.x).abs(), (self.b.y - self.a.y).abs())
    }

    /// `CaptureEditor.drawing(...).onEnded`: a stroke is kept when the pen made more than 2 points, any other
    /// shape when it is longer than 4 pixels. (The text is kept by `commit_text`, not by this.)
    pub fn is_kept(&self) -> bool {
        if self.kind == Tool::Pen {
            self.points.len() > 2
        } else {
            (self.b.x - self.a.x).hypot(self.b.y - self.a.y) > 4.0
        }
    }
}

/// `CaptureEditor.commitText`: the typed text without the spaces around it, `None` when nothing is left.
pub fn clean_typed_text(typed: &str) -> Option<String> {
    let clean = typed.trim();
    (!clean.is_empty()).then(|| clean.to_string())
}

/// `CaptureDocument.lineWidth`: proportional to the image, so arrows and rectangles do not vanish in big captures.
pub fn line_width(width: f64, height: f64) -> f64 {
    3f64.max(width.min(height) / 160.0)
}

/// `CaptureDocument.fontSize`.
pub fn font_size(width: f64, height: f64) -> f64 {
    16f64.max(width.min(height) / 28.0)
}

/// Length of the two strokes of an arrow head (`lw * 4.5`).
pub fn arrow_head_length(lw: f64) -> f64 {
    lw * 4.5
}

/// The ends of the two strokes of the arrow head: from the tip `b`, at `angle +- 0.83 pi` of the arrow direction.
pub fn arrow_head_points(a: Point, b: Point, lw: f64) -> [Point; 2] {
    let angle = (b.y - a.y).atan2(b.x - a.x);
    let head = arrow_head_length(lw);
    [0.83 * std::f64::consts::PI, -0.83 * std::f64::consts::PI].map(|side| Point::new(b.x + (angle + side).cos() * head, b.y + (angle + side).sin() * head))
}

/// Corner radius of a rectangle mark (`lw * 1.5`).
pub fn rect_corner_radius(lw: f64) -> f64 {
    lw * 1.5
}

/// The outline of a text mark so that it reads on any background: white around black text, otherwise black at
/// 55% opacity. Returns the color and the alpha.
pub fn text_outline(color: Rgb) -> (Rgb, f64) {
    if color == BLACK {
        (COLORS[7], 1.0)
    } else {
        (BLACK, 0.55)
    }
}

/// Offset of the four copies that make the outline (`max(1, lw / 3)`).
pub fn text_outline_offset(lw: f64) -> f64 {
    1f64.max(lw / 3.0)
}

// ---------------------------------------------------------------- blur

/// Size of the blocks of the blur: `Float(max(10, width / 90))` in `CaptureDocument.init`. The Mac gives the
/// number to `CIPixellate` as it is; here it is rounded to whole pixels.
pub fn pixelate_scale(width: usize) -> usize {
    10usize.max((width as f64 / 90.0).round() as usize)
}

/// The whole image pixelated: every block of `pixelate_scale(width)` pixels, counted from the top left corner, gets
/// the average color of its pixels. `data` is 4 bytes per pixel, tight rows (`width * 4`), any channel order
/// (Cairo's `ARgb32` is BGRA on little endian machines). The blur mark shows this image inside its rectangle.
pub fn pixelate(data: &[u8], width: usize, height: usize) -> Vec<u8> {
    assert_eq!(data.len(), width * height * 4, "pixelate: the buffer does not match the size");
    let block = pixelate_scale(width);
    let mut out = data.to_vec();
    let mut by = 0;
    while by < height {
        let bh = block.min(height - by);
        let mut bx = 0;
        while bx < width {
            let bw = block.min(width - bx);
            let mut sum = [0u64; 4];
            for y in by..by + bh {
                for x in bx..bx + bw {
                    let i = (y * width + x) * 4;
                    for c in 0..4 {
                        sum[c] += data[i + c] as u64;
                    }
                }
            }
            let n = (bw * bh) as u64;
            let avg = sum.map(|s| ((s + n / 2) / n) as u8);
            for y in by..by + bh {
                for x in bx..bx + bw {
                    let i = (y * width + x) * 4;
                    out[i..i + 4].copy_from_slice(&avg);
                }
            }
            bx += block;
        }
        by += block;
    }
    out
}

/// The pixels a blur mark covers: its box moved out to whole pixels and cut to the image, as
/// `(x0, y0, x1, y1)` with `x1` and `y1` excluded. `None` when it covers nothing.
pub fn blur_pixels(mark: &Mark, width: usize, height: usize) -> Option<(usize, usize, usize, usize)> {
    let (x, y, w, h) = mark.rect();
    let x0 = x.floor().max(0.0) as usize;
    let y0 = y.floor().max(0.0) as usize;
    let x1 = ((x + w).ceil().max(0.0) as usize).min(width);
    let y1 = ((y + h).ceil().max(0.0) as usize).min(height);
    (x0 < x1 && y0 < y1).then_some((x0, y0, x1, y1))
}

/// What the blur marks do to the pixels: inside each box the pixelated image, elsewhere the original. This is the
/// reference of the tests; the editor gets the same pixels by clipping the pixelated image to the box.
pub fn apply_blur(data: &[u8], pixelated: &[u8], width: usize, height: usize, marks: &[Mark]) -> Vec<u8> {
    let mut out = data.to_vec();
    for m in marks.iter().filter(|m| m.kind == Tool::Blur) {
        if let Some((x0, y0, x1, y1)) = blur_pixels(m, width, height) {
            for y in y0..y1 {
                let (s, e) = ((y * width + x0) * 4, (y * width + x1) * 4);
                out[s..e].copy_from_slice(&pixelated[s..e]);
            }
        }
    }
    out
}

// ---------------------------------------------------------------- file names and places

/// The word before the date in the name of a saved capture: `L("Captura %@", ...)`.
fn capture_word(lang: Language) -> &'static str {
    match lang {
        Language::En => "Screenshot",
        Language::Pt => "Captura",
        Language::De => "Bildschirmfoto",
        Language::Es => "Captura",
        Language::Zh => "截图",
    }
}

/// `ScreenCapture.save`: "Screenshot 2026-11-17 10.30.15.png". The Mac writes the date in the format of the
/// region and replaces `/` with `-` and `:` with `.`; here the date is always year-month-day (it sorts, and
/// needs no replacing).
pub fn screenshot_file_name(lang: Language, year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> String {
    format!("{} {year:04}-{month:02}-{day:02} {hour:02}.{minute:02}.{second:02}.png", capture_word(lang))
}

/// `screenshot_file_name` for the local time now.
pub fn screenshot_file_name_now(lang: Language) -> String {
    use chrono::{Datelike, Timelike};
    let n = chrono::Local::now();
    screenshot_file_name(lang, n.year(), n.month(), n.day(), n.hour(), n.minute(), n.second())
}

/// `CaptureEditor.dragProvider`: the file that is dragged to another app is called "Screenshot.png".
pub fn drag_file_name(lang: Language) -> String {
    format!("{}.png", capture_word(lang))
}

/// `ScreenCapture.save` opens the panel in the Desktop folder. Here: the Desktop folder of the user (XDG),
/// the home folder when there is none.
pub fn default_save_dir() -> PathBuf {
    dirs::desktop_dir().filter(|d| d.is_dir()).or_else(dirs::home_dir).unwrap_or_else(|| PathBuf::from("."))
}

/// The file of a `file://` address returned by the screenshot portal (percent escapes decoded).
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    // an optional host ("file://localhost/x") comes before the first slash of the path
    let path = &rest[rest.find('/')?..];
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
            if let Some(v) = hex {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).ok().map(PathBuf::from)
}

// ---------------------------------------------------------------- text from the screen

/// What `TextRecognizer.finish` does with the recognized text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextOutcome {
    /// Nothing was read: "No text found", the clipboard is left alone.
    Nothing,
    /// The text for the clipboard, and the number of lines (`clean.split("\n")`, empty ones omitted).
    Copy { text: String, lines: usize },
}

pub fn text_outcome(recognized: &str) -> TextOutcome {
    let clean = recognized.trim();
    if clean.is_empty() {
        return TextOutcome::Nothing;
    }
    TextOutcome::Copy { text: clean.to_string(), lines: clean.split('\n').filter(|l| !l.is_empty()).count() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_is_decoded() {
        assert_eq!(uri_to_path("file:///home/ana/Pictures/Screenshot%20from%202026.png"), Some(PathBuf::from("/home/ana/Pictures/Screenshot from 2026.png")));
        assert_eq!(uri_to_path("file://localhost/tmp/a.png"), Some(PathBuf::from("/tmp/a.png")));
        assert_eq!(uri_to_path("https://x/y.png"), None);
        assert_eq!(uri_to_path("file:///tmp/100%.png"), Some(PathBuf::from("/tmp/100%.png")));
        assert_eq!(uri_to_path("file:///tmp/a%2"), Some(PathBuf::from("/tmp/a%2")));
    }

    #[test]
    fn pixelate_averages_each_block() {
        // 20 x 10, blocks of 10: the left block is all 100, the right one half 0 and half 200
        let (w, h) = (20usize, 10usize);
        let mut data = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let v = if x < 10 { 100 } else if y < 5 { 0 } else { 200 };
                data[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        let p = pixelate(&data, w, h);
        assert_eq!(&p[0..4], &[100, 100, 100, 255]);
        assert_eq!(&p[(5 * w + 15) * 4..(5 * w + 15) * 4 + 4], &[100, 100, 100, 255]);
    }
}
