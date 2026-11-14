//! Transformations of the copied text (port of `TextTransforms.swift`): "uppercase", "json",
//! "base64"... Pure functions: they take the text and return the result, or `None` when it cannot
//! be done (invalid JSON, broken Base64).

use crate::format::date_numeric;
use crate::i18n::Language;
use crate::search::strip;
use crate::strings::{lookup, tr};
use serde_json::Value;
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextTransform {
    Upper,
    Lower,
    Title,
    Sentence,
    NoAccents,
    Trim,
    SortLines,
    ReverseLines,
    UniqueLines,
    JsonPretty,
    JsonMinify,
    Base64Encode,
    Base64Decode,
    UrlEncode,
    UrlDecode,
    Count,
    Camel,
    Snake,
    Kebab,
    Jwt,
}

pub const ALL: [TextTransform; 20] = [
    TextTransform::Upper,
    TextTransform::Lower,
    TextTransform::Title,
    TextTransform::Sentence,
    TextTransform::NoAccents,
    TextTransform::Trim,
    TextTransform::SortLines,
    TextTransform::ReverseLines,
    TextTransform::UniqueLines,
    TextTransform::JsonPretty,
    TextTransform::JsonMinify,
    TextTransform::Base64Encode,
    TextTransform::Base64Decode,
    TextTransform::UrlEncode,
    TextTransform::UrlDecode,
    TextTransform::Count,
    TextTransform::Camel,
    TextTransform::Snake,
    TextTransform::Kebab,
    TextTransform::Jwt,
];

/// Preview only up to this size; above it Enter still transforms but nothing is computed while typing.
pub const PREVIEW_LIMIT: usize = 100_000;

impl TextTransform {
    /// Stable id ("text.upper").
    pub fn id(self) -> &'static str {
        match self {
            Self::Upper => "upper",
            Self::Lower => "lower",
            Self::Title => "title",
            Self::Sentence => "sentence",
            Self::NoAccents => "noAccents",
            Self::Trim => "trim",
            Self::SortLines => "sortLines",
            Self::ReverseLines => "reverseLines",
            Self::UniqueLines => "uniqueLines",
            Self::JsonPretty => "jsonPretty",
            Self::JsonMinify => "jsonMinify",
            Self::Base64Encode => "base64Encode",
            Self::Base64Decode => "base64Decode",
            Self::UrlEncode => "urlEncode",
            Self::UrlDecode => "urlDecode",
            Self::Count => "count",
            Self::Camel => "camel",
            Self::Snake => "snake",
            Self::Kebab => "kebab",
            Self::Jwt => "jwt",
        }
    }

    fn title_key(self) -> &'static str {
        match self {
            Self::Upper => "tt_upper",
            Self::Lower => "tt_lower",
            Self::Title => "tt_title",
            Self::Sentence => "tt_sentence",
            Self::NoAccents => "tt_no_accents",
            Self::Trim => "tt_trim",
            Self::SortLines => "tt_sort_lines",
            Self::ReverseLines => "tt_reverse_lines",
            Self::UniqueLines => "tt_unique_lines",
            Self::JsonPretty => "tt_json_pretty",
            Self::JsonMinify => "tt_json_minify",
            Self::Base64Encode => "tt_base64_encode",
            Self::Base64Decode => "tt_base64_decode",
            Self::UrlEncode => "tt_url_encode",
            Self::UrlDecode => "tt_url_decode",
            Self::Count => "tt_count",
            Self::Camel => "tt_camel",
            Self::Snake => "tt_snake",
            Self::Kebab => "tt_kebab",
            Self::Jwt => "tt_jwt",
        }
    }

    pub fn title(self, lang: Language) -> &'static str {
        lookup(self.title_key(), lang).unwrap_or("")
    }

    /// Words for the search, in the 5 languages (the translated title is searched too).
    pub fn keywords(self) -> String {
        let common = "texto text transformar transform converter convert umwandeln transformar 转换 文本";
        let own = match self {
            Self::Upper => "maiúsculas maiusculas upper uppercase caps großbuchstaben grossbuchstaben mayúsculas mayusculas 大写",
            Self::Lower => "minúsculas minusculas lower lowercase kleinbuchstaben minúsculas 小写",
            Self::Title => "título titulo title case capitalizar capitalize primeiras maiúsculas wortanfänge großschreibung tipo título 标题 首字母",
            Self::Sentence => "frase sentence case primeira maiúscula satz satzanfang oración oracion 句子 句首",
            Self::NoAccents => "acentos accents diacríticos diacritics remover remove akzente entfernen quitar tildes 重音 变音",
            Self::Trim => "espaços espacos whitespace trim spaces limpar clean leerzeichen bereinigen espacios limpiar 空格 空白",
            Self::SortLines => "ordenar sort linhas lines alfabética alphabetical sortieren zeilen ordenar líneas lineas 排序 行",
            Self::ReverseLines => "inverter reverse linhas lines umkehren zeilen invertir líneas lineas 反转 行",
            Self::UniqueLines => "repetidas duplicadas duplicates unique dedupe linhas lines doppelte zeilen duplicadas líneas 重复 去重",
            Self::JsonPretty => "json formatar format pretty print beautify indentar formatieren formatear 格式化",
            Self::JsonMinify => "json compactar minify compact minificar komprimieren compactar 压缩",
            Self::Base64Encode => "base64 codificar encode kodieren codificar 编码",
            Self::Base64Decode => "base64 decodificar decode dekodieren decodificar 解码",
            Self::UrlEncode => "url codificar encode percent escape kodieren codificar 编码 链接",
            Self::UrlDecode => "url decodificar decode percent unescape dekodieren decodificar 解码 链接",
            Self::Count => "contar count caracteres characters palavras words linhas lines zählen zeichen wörter contar palabras 统计 字数 字符",
            Self::Camel => "camel camelcase código code variável variable 驼峰",
            Self::Snake => "snake snakecase underline underscore código code variável variable 下划线",
            Self::Kebab => "kebab kebabcase hífen hyphen dash slug código code 连字符",
            Self::Jwt => "jwt token bearer decodificar decode auth oauth payload claims dekodieren decodificar 令牌 解码",
        };
        format!("{own} {common}")
    }

    /// Shows the result instead of copying it.
    pub fn is_info(self) -> bool {
        self == Self::Count
    }

    /// Text for when `apply` returns `None`.
    pub fn failure(self, lang: Language) -> &'static str {
        let key = match self {
            Self::JsonPretty | Self::JsonMinify => "tt_fail_json",
            Self::Base64Decode => "tt_fail_base64",
            Self::Jwt => "tt_fail_jwt",
            _ => "tt_fail",
        };
        lookup(key, lang).unwrap_or("")
    }

    /// Apply to `s`. Dates inside a JWT are shown in the computer's time zone.
    pub fn apply(self, s: &str, lang: Language) -> Option<String> {
        self.apply_with(s, lang, &local_date_text(lang))
    }

    /// Like [`apply`](Self::apply) with the date formatter chosen by the caller (tests use UTC).
    pub fn apply_with(self, s: &str, lang: Language, date: &dyn Fn(i64) -> String) -> Option<String> {
        match self {
            Self::Upper => Some(s.to_uppercase()),
            Self::Lower => Some(s.to_lowercase()),
            Self::Title => Some(title_case(s)),
            Self::Sentence => Some(sentence_case(s)),
            Self::NoAccents => Some(strip_diacritics(s)),
            Self::Trim => Some(collapse_whitespace(s)),
            Self::SortLines => Some(map_lines(s, |mut l| {
                l.sort_by(|a, b| natural_cmp(a, b));
                l
            })),
            Self::ReverseLines => Some(map_lines(s, |mut l| {
                l.reverse();
                l
            })),
            Self::UniqueLines => Some(map_lines(s, |l| {
                let mut seen = std::collections::HashSet::new();
                l.into_iter().filter(|x| seen.insert(x.clone())).collect()
            })),
            Self::JsonPretty => json(s, true),
            Self::JsonMinify => json(s, false),
            Self::Base64Encode => Some(base64_encode(s.as_bytes())),
            Self::Base64Decode => base64_decode(s),
            Self::UrlEncode => Some(url_encode(s)),
            Self::UrlDecode => url_decode(s),
            Self::Count => Some(stats(s, lang)),
            Self::Camel => Some(identifiers(s, |w| {
                w.iter()
                    .enumerate()
                    .map(|(i, x)| {
                        if i == 0 {
                            x.to_lowercase()
                        } else {
                            let mut cs = x.chars();
                            match cs.next() {
                                Some(f) => f.to_uppercase().collect::<String>() + &cs.as_str().to_lowercase(),
                                None => String::new(),
                            }
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("")
            })),
            Self::Snake => Some(identifiers(s, |w| w.iter().map(|x| x.to_lowercase()).collect::<Vec<_>>().join("_"))),
            Self::Kebab => Some(identifiers(s, |w| w.iter().map(|x| x.to_lowercase()).collect::<Vec<_>>().join("-"))),
            Self::Jwt => jwt(s, date),
        }
    }

    /// The preview line shown under the title: the result (first 200 characters, lines joined with
    /// "⏎") or why it failed.
    pub fn preview(self, text: &str, lang: Language) -> String {
        if text.len() > PREVIEW_LIMIT {
            return tr("tt_too_large", lang, &[]);
        }
        match self.apply(text, lang) {
            Some(r) => {
                let head: String = r.chars().take(200).collect();
                split_lines(&head).into_iter().filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" \u{23CE} ")
            }
            None => self.failure(lang).to_string(),
        }
    }
}

/// Date text of a Unix time in the computer's time zone.
fn local_date_text(lang: Language) -> impl Fn(i64) -> String {
    move |ts| {
        use chrono::TimeZone;
        match chrono::Local.timestamp_opt(ts, 0).single() {
            Some(d) => date_numeric(&d.naive_local(), lang),
            None => ts.to_string(),
        }
    }
}

// ---- helpers ----

fn is_newline(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}' | '\u{b}' | '\u{c}')
}

/// Split into lines, keeping empty ones; "\r\n" counts as one break.
fn split_lines(s: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if is_newline(c) {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    out.push(cur);
    out
}

/// Apply `f` to the lines, keeping the final line break when there is one.
fn map_lines(s: &str, f: impl FnOnce(Vec<String>) -> Vec<String>) -> String {
    let mut lines = split_lines(s);
    let trailing = lines.len() > 1 && lines.last().map(|l| l.is_empty()).unwrap_or(false);
    if trailing {
        lines.pop();
    }
    f(lines).join("\n") + if trailing { "\n" } else { "" }
}

/// Compare like Finder: ignoring case and accents, with numbers by value ("file2" < "file10").
fn natural_cmp(a: &str, b: &str) -> Ordering {
    fn chunks(s: &str) -> Vec<(bool, String)> {
        let mut out: Vec<(bool, String)> = vec![];
        for c in crate::search::fold(s).chars() {
            let d = c.is_ascii_digit();
            match out.last_mut() {
                Some((is_d, t)) if *is_d == d => t.push(c),
                _ => out.push((d, c.to_string())),
            }
        }
        out
    }
    let (ca, cb) = (chunks(a), chunks(b));
    for (x, y) in ca.iter().zip(cb.iter()) {
        let o = if x.0 && y.0 {
            let (tx, ty) = (x.1.trim_start_matches('0'), y.1.trim_start_matches('0'));
            tx.len().cmp(&ty.len()).then_with(|| tx.cmp(ty))
        } else {
            x.1.cmp(&y.1)
        };
        if o != Ordering::Equal {
            return o;
        }
    }
    ca.len().cmp(&cb.len()).then_with(|| a.cmp(b))
}

fn title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut start = true;
    let mut prev: Option<char> = None;
    for c in s.chars() {
        let in_word = c.is_alphanumeric() || ((c == '\'' || c == '\u{2019}') && prev.map(|p| p.is_alphanumeric()).unwrap_or(false));
        if c.is_alphabetic() {
            if start {
                out.extend(c.to_uppercase());
            } else {
                out.extend(c.to_lowercase());
            }
            start = false;
        } else {
            out.push(c);
            if c.is_numeric() {
                start = false;
            } else if !in_word {
                start = true;
            }
        }
        prev = Some(c);
    }
    out
}

/// Everything lowercase, with a capital at the start and after . ! ? or a line break.
fn sentence_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut capitalize = true;
    for ch in s.to_lowercase().chars() {
        if capitalize && ch.is_alphabetic() {
            out.extend(ch.to_uppercase());
            capitalize = false;
        } else {
            out.push(ch);
        }
        if ".!?".contains(ch) || is_newline(ch) {
            capitalize = true;
        }
    }
    out
}

/// Remove diacritics, keeping the case ("Ação" -> "Acao"). Letters without a decomposition
/// (ø, ł, đ...) stay, like the Mac's `stripDiacritics` transform.
pub fn strip_diacritics(s: &str) -> String {
    s.chars()
        .map(|c| {
            if "øłđħŧıØŁĐĦŦ".contains(c) {
                return c;
            }
            let lower = c.to_lowercase().next().unwrap_or(c);
            let base = strip(lower);
            if base == lower {
                c
            } else if c.is_uppercase() {
                base.to_uppercase().next().unwrap_or(base)
            } else {
                base
            }
        })
        .collect()
}

/// Strip the ends of every line, join repeated spaces, keep at most one blank line in a row.
fn collapse_whitespace(s: &str) -> String {
    let mut out: Vec<String> = vec![];
    for line in split_lines(s) {
        let clean = line.split([' ', '\t', '\u{a0}']).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ");
        if clean.is_empty() && out.last().map(|l| l.is_empty()).unwrap_or(true) {
            continue;
        }
        out.push(clean);
    }
    while out.last().map(|l| l.is_empty()).unwrap_or(false) {
        out.pop();
    }
    out.join("\n")
}

// ---- JSON ----

fn write_json(v: &Value, pretty: bool, depth: usize, out: &mut String) {
    let pad = |n: usize| "  ".repeat(n);
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => match (n.as_i64(), n.as_u64(), n.as_f64()) {
            (Some(i), _, _) => out.push_str(&i.to_string()),
            (_, Some(u), _) => out.push_str(&u.to_string()),
            (_, _, Some(f)) if f.fract() == 0.0 && f.abs() < 1e15 => out.push_str(&format!("{}", f as i64)),
            (_, _, Some(f)) => out.push_str(&format!("{f}")),
            _ => out.push_str(&n.to_string()),
        },
        // serde_json does not escape "/" (like withoutEscapingSlashes)
        Value::String(s) => out.push_str(&serde_json::to_string(s).unwrap_or_default()),
        Value::Array(a) if a.is_empty() => out.push_str("[]"),
        Value::Object(o) if o.is_empty() => out.push_str("{}"),
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                if pretty {
                    out.push('\n');
                    out.push_str(&pad(depth + 1));
                }
                write_json(x, pretty, depth + 1, out);
            }
            if pretty {
                out.push('\n');
                out.push_str(&pad(depth));
            }
            out.push(']');
        }
        Value::Object(o) => {
            out.push('{');
            // serde_json keeps the keys sorted (sortedKeys)
            for (i, (k, x)) in o.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                if pretty {
                    out.push('\n');
                    out.push_str(&pad(depth + 1));
                }
                out.push_str(&serde_json::to_string(k).unwrap_or_default());
                // Apple's pretty printer writes `"key" : value`
                out.push_str(if pretty { " : " } else { ":" });
                write_json(x, pretty, depth + 1, out);
            }
            if pretty {
                out.push('\n');
                out.push_str(&pad(depth));
            }
            out.push('}');
        }
    }
}

fn json(s: &str, pretty: bool) -> Option<String> {
    let v: Value = serde_json::from_str(s.trim()).ok()?;
    let mut out = String::new();
    write_json(&v, pretty, 0, &mut out);
    Some(out)
}

// ---- Base64 and URL ----

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

fn base64_bytes(s: &str) -> Option<Vec<u8>> {
    let b: Vec<u8> = s.bytes().collect();
    if !b.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(b.len() / 4 * 3);
    for (ci, chunk) in b.chunks(4).enumerate() {
        let last = (ci + 1) * 4 == b.len();
        let mut n = 0u32;
        let mut pad = 0;
        for (i, &c) in chunk.iter().enumerate() {
            let v = if c == b'=' {
                if !last || i < 2 {
                    return None;
                }
                pad += 1;
                0
            } else {
                if pad > 0 {
                    return None;
                }
                B64.iter().position(|&x| x == c)? as u32
            };
            n = n << 6 | v;
        }
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Some(out)
}

/// Accepts URL-safe Base64 (- and _) and a missing "=" at the end.
fn base64_decode(s: &str) -> Option<String> {
    let mut b: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    b = b.replace('-', "+").replace('_', "/");
    if b.is_empty() {
        return None;
    }
    let n = b.chars().count();
    b.push_str(&"=".repeat((4 - n % 4) % 4));
    String::from_utf8(base64_bytes(&b)?).ok()
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

fn url_decode(s: &str) -> Option<String> {
    let s = s.replace('+', " ");
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

// ---- counts, identifiers, JWT ----

fn count_words(s: &str) -> usize {
    let chars: Vec<char> = s.chars().collect();
    let mut words = 0;
    let mut in_word = false;
    for (i, &c) in chars.iter().enumerate() {
        let apostrophe_inside = (c == '\'' || c == '\u{2019}')
            && in_word
            && chars.get(i + 1).map(|n| n.is_alphanumeric()).unwrap_or(false);
        if c.is_alphanumeric() || apostrophe_inside {
            if !in_word {
                words += 1;
                in_word = true;
            }
        } else {
            in_word = false;
        }
    }
    words
}

fn stats(s: &str, lang: Language) -> String {
    let words = count_words(s);
    let lines = if s.is_empty() { 0 } else { split_lines(s).len() };
    let key = if lines == 1 { "tt_stats_one" } else { "tt_stats_many" };
    tr(key, lang, &[&s.chars().count().to_string(), &words.to_string(), &lines.to_string()])
}

/// Split every line into words ("Olá mundo", "helloWorld", "HTTPServer", "foo-bar") and join again.
fn identifiers(s: &str, join: impl Fn(&[String]) -> String) -> String {
    let plain = strip_diacritics(s);
    map_lines(&plain, |lines| {
        lines
            .into_iter()
            .map(|line| {
                let chars: Vec<char> = line.chars().collect();
                let mut words: Vec<String> = vec![];
                let mut current = String::new();
                for (i, &ch) in chars.iter().enumerate() {
                    if !(ch.is_alphabetic() || ch.is_numeric()) {
                        if !current.is_empty() {
                            words.push(std::mem::take(&mut current));
                        }
                        continue;
                    }
                    if let Some(prev) = current.chars().last() {
                        if ch.is_uppercase() {
                            let next_is_lower = chars.get(i + 1).map(|n| n.is_lowercase()).unwrap_or(false);
                            // "helloWorld" -> hello World; "HTTPServer" -> HTTP Server
                            if prev.is_lowercase() || prev.is_numeric() || (prev.is_uppercase() && next_is_lower) {
                                words.push(std::mem::take(&mut current));
                            }
                        }
                    }
                    current.push(ch);
                }
                if !current.is_empty() {
                    words.push(current);
                }
                join(&words)
            })
            .collect()
    })
}

/// Header and payload of a JWT, formatted, with exp/iat/nbf as dates at the end. The signature is not checked.
fn jwt(s: &str, date: &dyn Fn(i64) -> String) -> Option<String> {
    let mut token = s.trim().to_string();
    if token.to_lowercase().starts_with("bearer ") {
        token = token[7..].trim().to_string();
    }
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let header = base64_decode(parts[0])?;
    let payload = base64_decode(parts[1])?;
    let h = json(&header, true)?;
    let p = json(&payload, true)?;
    let mut out = format!("{h}\n{p}");
    if let Ok(Value::Object(claims)) = serde_json::from_str::<Value>(&payload) {
        let dates: Vec<String> = ["exp", "iat", "nbf"]
            .iter()
            .filter_map(|k| claims.get(*k).and_then(|v| v.as_f64()).map(|t| format!("{k}: {}", date(t as i64))))
            .collect();
        if !dates.is_empty() {
            out.push_str("\n\n");
            out.push_str(&dates.join("\n"));
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use TextTransform::*;

    fn t(tr: TextTransform, s: &str) -> Option<String> {
        tr.apply_with(s, Language::En, &|ts| format!("@{ts}"))
    }

    // The vectors V035 to V062 of docs/palette-spec.md, one by one
    #[test]
    fn case_and_identifier_transforms() {
        assert_eq!(t(Snake, "hello World").as_deref(), Some("hello_world"));
        assert_eq!(t(Kebab, "helloWorld").as_deref(), Some("hello-world"));
        assert_eq!(t(Camel, "hello world").as_deref(), Some("helloWorld"));
        assert_eq!(t(Snake, "HTTPServer error").as_deref(), Some("http_server_error"));
        assert_eq!(t(Snake, "Olá mundo\nfoo-bar").as_deref(), Some("ola_mundo\nfoo_bar"));
        assert_eq!(t(Upper, "hello world").as_deref(), Some("HELLO WORLD"));
        assert_eq!(t(Lower, "Hello WORLD").as_deref(), Some("hello world"));
        assert_eq!(t(Title, "hello wORLD").as_deref(), Some("Hello World"));
        assert_eq!(t(Title, "it's a dog-friendly place").as_deref(), Some("It's A Dog-Friendly Place"));
        assert_eq!(t(Sentence, "hello. WORLD is here").as_deref(), Some("Hello. World is here"));
        assert_eq!(t(NoAccents, "Ação é útil").as_deref(), Some("Acao e util"));
        assert_eq!(t(NoAccents, "Łódź").as_deref(), Some("Łodz"));
    }

    #[test]
    fn whitespace_and_lines() {
        assert_eq!(t(Trim, "a   b \n\n\n c  ").as_deref(), Some("a b\n\nc"));
        assert_eq!(t(SortLines, "b\na\nc").as_deref(), Some("a\nb\nc"));
        assert_eq!(t(SortLines, "file10\nfile2\nFile1").as_deref(), Some("File1\nfile2\nfile10"));
        // the final line break is kept
        assert_eq!(t(SortLines, "b\na\n").as_deref(), Some("a\nb\n"));
        assert_eq!(t(ReverseLines, "a\nb\nc").as_deref(), Some("c\nb\na"));
        assert_eq!(t(UniqueLines, "a\nb\na").as_deref(), Some("a\nb"));
    }

    #[test]
    fn json() {
        assert_eq!(
            t(JsonPretty, r#"{"b":1,"a":[1,2]}"#).as_deref(),
            Some("{\n  \"a\" : [\n    1,\n    2\n  ],\n  \"b\" : 1\n}")
        );
        assert_eq!(t(JsonMinify, r#"{ "b": 1, "a": [1, 2] }"#).as_deref(), Some(r#"{"a":[1,2],"b":1}"#));
        assert_eq!(t(JsonPretty, r#"{"a":}"#), None);
        assert_eq!(t(JsonMinify, "[1, 2.5, \"a/b\", null, true]").as_deref(), Some("[1,2.5,\"a/b\",null,true]"));
        assert_eq!(t(JsonPretty, "[]").as_deref(), Some("[]"));
    }

    #[test]
    fn base64_and_urls() {
        assert_eq!(t(Base64Encode, "hello").as_deref(), Some("aGVsbG8="));
        assert_eq!(t(Base64Encode, "hi").as_deref(), Some("aGk="));
        assert_eq!(t(Base64Encode, "").as_deref(), Some(""));
        assert_eq!(t(Base64Decode, "aGVsbG8=").as_deref(), Some("hello"));
        assert_eq!(t(Base64Decode, "aGVsbG8").as_deref(), Some("hello"));
        assert_eq!(t(Base64Decode, "aGk_").as_deref(), Some("hi?"));
        assert_eq!(t(Base64Decode, "not base64!"), None);
        assert_eq!(t(Base64Decode, "   "), None);
        assert_eq!(t(UrlEncode, "a b&c").as_deref(), Some("a%20b%26c"));
        assert_eq!(t(UrlEncode, "ação").as_deref(), Some("a%C3%A7%C3%A3o"));
        assert_eq!(t(UrlDecode, "a%20b%26c+d").as_deref(), Some("a b&c d"));
        assert_eq!(t(UrlDecode, "100%"), None);
        assert_eq!(t(UrlDecode, "%ff"), None);
    }

    #[test]
    fn counts() {
        assert_eq!(t(Count, "Hello world\nline two").as_deref(), Some("20 characters · 4 words · 2 lines"));
        assert_eq!(t(Count, "one").as_deref(), Some("3 characters · 1 words · 1 line"));
        assert_eq!(t(Count, "").as_deref(), Some("0 characters · 0 words · 0 lines"));
        assert_eq!(t(Count, "don't stop").as_deref(), Some("10 characters · 2 words · 1 line"));
    }

    #[test]
    fn jwt_with_dates() {
        let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxIiwiZXhwIjoxNzkzNDAwMDAwfQ.sig";
        assert_eq!(
            t(Jwt, token).as_deref(),
            Some("{\n  \"alg\" : \"HS256\",\n  \"typ\" : \"JWT\"\n}\n{\n  \"exp\" : 1793400000,\n  \"sub\" : \"1\"\n}\n\nexp: @1793400000")
        );
        assert_eq!(t(Jwt, &format!("Bearer {token}")), t(Jwt, token));
        assert_eq!(t(Jwt, "not.a.jwt"), None);
        assert_eq!(t(Jwt, "only.two"), None);
    }

    #[test]
    fn preview_shortens_and_flattens() {
        let long = "x".repeat(300);
        assert_eq!(Upper.preview(&long, Language::En).chars().count(), 200);
        assert_eq!(ReverseLines.preview("a\nb\nc", Language::En), "c \u{23CE} b \u{23CE} a");
        assert_eq!(JsonPretty.preview("{", Language::En), "Invalid JSON");
        let huge = "a".repeat(PREVIEW_LIMIT + 1);
        assert_eq!(Upper.preview(&huge, Language::En), "Text too large to preview");
    }

    #[test]
    fn every_transform_has_a_title_in_every_language() {
        for tr in ALL {
            for l in [Language::En, Language::Pt, Language::De, Language::Es, Language::Zh] {
                assert!(!tr.title(l).is_empty(), "{:?} {:?}", tr, l);
                assert!(!tr.failure(l).is_empty());
            }
            assert!(!tr.keywords().is_empty());
        }
        assert_eq!(ALL.len(), 20);
    }
}
