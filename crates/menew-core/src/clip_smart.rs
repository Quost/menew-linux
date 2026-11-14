//! Smart clipboard actions (port of `SmartClip.swift`): what the palette and the item menu offer for a
//! copied text that is a meeting link, a color, a tracking code, a phone number, an address, an
//! e-mail or a link. Pure: it returns what to do, the window runs it.
//!
//! Where the Mac asks Apple's `NSDataDetector` (phone numbers, addresses, links) this file uses
//! hand-written rules instead; `docs/clipboard-spec.md` says which ones are exact and which are not.

use crate::i18n::Language;
use crate::strings::tr;

/// What running a smart action does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmartRun {
    /// Open this address with the default handler (browser, phone app, mail app).
    Open(String),
    /// Put this text on the clipboard.
    Copy(String),
}

impl SmartRun {
    /// Text used by the test vectors in `docs/clipboard-spec.md`.
    pub fn describe(&self) -> String {
        match self {
            SmartRun::Open(u) => format!("open:{u}"),
            SmartRun::Copy(t) => format!("copy:{t}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartAction {
    /// `meeting`, `color.HEX`, `tracking`, `phone`, `whatsapp`, `address`, `mail`, `link`.
    pub id: String,
    pub title: String,
    pub subtitle: String,
    /// A freedesktop icon name.
    pub icon: &'static str,
    pub keywords: String,
    pub run: SmartRun,
}

/// The Mac only looks at short texts.
pub const MAX_CHARS: usize = 400;

/// Smart actions for a copied text, in the order of the Mac: meeting, colors, tracking, then what the
/// detector finds (phone, address, e-mail or link). `region` is the country of the user's locale
/// (`BR`, `PT`...), used to complete a phone number for WhatsApp.
pub fn actions(raw: &str, lang: Language, region: Option<&str>) -> Vec<SmartAction> {
    let text = raw.trim();
    if text.is_empty() || text.chars().count() > MAX_CHARS {
        return vec![];
    }
    detect(text, lang, region)
}

fn short_of(text: &str) -> String {
    if text.chars().count() > 60 {
        let cut: String = text.chars().take(57).collect();
        format!("{cut}\u{2026}")
    } else {
        text.to_string()
    }
}

fn detect(text: &str, lang: Language, region: Option<&str>) -> Vec<SmartAction> {
    let mut out = vec![];
    let short = short_of(text);

    if let Some(link) = meeting_link(text) {
        out.push(SmartAction {
            id: "meeting".into(),
            title: tr("smart_meeting", lang, &[]),
            subtitle: host_of(&link).unwrap_or_else(|| short.clone()),
            icon: "camera-video-symbolic",
            keywords: "entrar reuni\u{e3}o reuniao join meeting zoom meet teams beitreten unirse \u{52a0}\u{5165}".into(),
            run: SmartRun::Open(link),
        });
    }

    if let Some(color) = parse_color(text) {
        for (label, value) in [("HEX", color.hex()), ("RGB", color.rgb()), ("HSL", color.hsl())] {
            if value.to_lowercase() != text.to_lowercase() {
                out.push(SmartAction {
                    id: format!("color.{label}"),
                    title: tr("smart_copy_as", lang, &[&value]),
                    subtitle: tr("smart_color_copied", lang, &[&short]),
                    icon: "color-select-symbolic",
                    keywords: format!("cor color hex rgb hsl farbe \u{989c}\u{8272} {}", label.to_lowercase()),
                    run: SmartRun::Copy(value),
                });
            }
        }
    }

    if let Some(code) = tracking_code(text) {
        let url = if code.ends_with("BR") {
            format!("https://rastreamento.correios.com.br/app/index.php?objetos={code}")
        } else {
            format!("https://parcelsapp.com/en/tracking/{code}")
        };
        out.push(SmartAction {
            id: "tracking".into(),
            title: tr("smart_track", lang, &[&code]),
            subtitle: host_of(&url).unwrap_or_default(),
            icon: "package-x-generic-symbolic",
            keywords: "rastrear rastreio encomenda pacote correios track tracking package sendung verfolgen rastrear \u{5feb}\u{9012} \u{8ffd}\u{8e2a}".into(),
            run: SmartRun::Open(url),
        });
    }

    let total = text.chars().count();
    let near_all = |len: usize| len * 10 >= total * 7;

    // phone: only when the text is practically just that
    if let Some(phone) = phone_number(text) {
        if near_all(phone.chars().count()) {
            let digits: String = phone.chars().filter(|c| c.is_ascii_digit() || *c == '+').collect();
            out.push(SmartAction {
                id: "phone".into(),
                title: tr("smart_call", lang, &[&phone]),
                subtitle: tr("smart_call_sub", lang, &[]),
                icon: "call-start-symbolic",
                keywords: format!("ligar telefone call phone anrufen llamar \u{7535}\u{8bdd} {digits}"),
                run: SmartRun::Open(format!("tel:{digits}")),
            });
            // WhatsApp needs the country code: without a "+" it comes from the locale
            let mut wa: String = digits.chars().filter(|c| c.is_ascii_digit()).collect();
            let cc = region.and_then(country_code);
            if !digits.starts_with('+') {
                if let Some(cc) = cc {
                    wa = format!("{cc}{wa}");
                }
            }
            if (digits.starts_with('+') || cc.is_some()) && wa.chars().count() >= 10 {
                out.push(SmartAction {
                    id: "whatsapp".into(),
                    title: tr("smart_whatsapp", lang, &[]),
                    subtitle: phone.clone(),
                    icon: "chat-message-new-symbolic",
                    keywords: "whatsapp zap mensagem message chat".into(),
                    run: SmartRun::Open(format!("https://wa.me/{wa}")),
                });
            }
        }
    } else if let Some(address) = street_address(text) {
        if near_all(address.chars().count()) {
            out.push(SmartAction {
                id: "address".into(),
                title: tr("smart_maps", lang, &[]),
                subtitle: short.clone(),
                icon: "mark-location-symbolic",
                keywords: "mapa mapas maps endere\u{e7}o endereco address karte mapa \u{5730}\u{56fe}".into(),
                run: SmartRun::Open(format!("https://www.openstreetmap.org/search?query={}", percent_encode_query(&address))),
            });
        }
    }

    // e-mail and links: the Mac skips them when a meeting link was already offered
    if !out.iter().any(|a| a.id == "meeting") {
        for tok in text.split_whitespace() {
            let tok = trim_trailing_punctuation(tok);
            if tok.is_empty() || !near_all(tok.chars().count()) {
                continue;
            }
            if let Some(addr) = email_address(tok) {
                out.push(SmartAction {
                    id: "mail".into(),
                    title: tr("smart_mail", lang, &[&addr]),
                    subtitle: tr("smart_mail_sub", lang, &[]),
                    icon: "mail-send-symbolic",
                    keywords: "email e-mail escrever mail write schreiben correo \u{90ae}\u{4ef6}".into(),
                    run: SmartRun::Open(format!("mailto:{addr}")),
                });
            } else if let Some(url) = web_link(tok) {
                out.push(SmartAction {
                    id: "link".into(),
                    title: tr("smart_open", lang, &[&host_of(&url).unwrap_or_else(|| short.clone())]),
                    subtitle: url.clone(),
                    icon: "web-browser-symbolic",
                    keywords: "abrir link open url \u{f6}ffnen abrir \u{6253}\u{5f00}".into(),
                    run: SmartRun::Open(url),
                });
            }
        }
    }
    out
}

// ---------------------------------------------------------------- meeting link

/// Zoom, Meet, Teams, Webex, Whereby, Jitsi, Chime and FaceTime links (`MeetingStore.linkPattern`):
/// `https://[\w.-]*(zoom\.us/(j|my|w)/|meet\.google\.com/|teams\.microsoft\.com/l/meetup-join/|
/// teams\.live\.com/meet/|[\w-]+\.webex\.com/|whereby\.com/|meet\.jit\.si/|chime\.aws/|
/// facetime\.apple\.com/join)[^\s<>"')\]]*`, case insensitive. The first match in the text.
pub fn meeting_link(text: &str) -> Option<String> {
    const ALTS: [&str; 10] = [
        "zoom.us/j/",
        "zoom.us/my/",
        "zoom.us/w/",
        "meet.google.com/",
        "teams.microsoft.com/l/meetup-join/",
        "teams.live.com/meet/",
        "whereby.com/",
        "meet.jit.si/",
        "chime.aws/",
        "facetime.apple.com/join",
    ];
    let lower = text.to_ascii_lowercase();
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let mut from = 0;
    while let Some(i) = lower[from..].find("https://") {
        let start = from + i;
        let after = start + 8;
        // the `[\w.-]*` prefix can cover at most this run
        let run_end = after + text[after..].find(|c: char| !(word(c) || c == '.' || c == '-')).unwrap_or(text.len() - after);
        let mut matched = false;
        for p in after..=run_end {
            if !text.is_char_boundary(p) {
                continue;
            }
            if ALTS.iter().any(|a| lower[p..].starts_with(a)) {
                matched = true;
                break;
            }
            // `[\w-]+\.webex\.com/`
            if lower[p..].starts_with(".webex.com/") && p > after && text[..p].chars().last().map(|c| word(c) || c == '-').unwrap_or(false) {
                matched = true;
                break;
            }
        }
        if matched {
            let end = text[after..]
                .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'' | ')' | ']'))
                .map(|e| after + e)
                .unwrap_or(text.len());
            return Some(text[start..end].to_string());
        }
        from = start + 1;
    }
    None
}

/// `URL.host()`: the host of an address, without user, port or path.
pub fn host_of(url: &str) -> Option<String> {
    let rest = url.split_once("://").map(|(_, r)| r)?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?;
    let host = host.split(':').next()?;
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

// ---------------------------------------------------------------- color

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl ParsedColor {
    pub fn hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
    pub fn rgb(&self) -> String {
        format!("rgb({}, {}, {})", self.r, self.g, self.b)
    }
    pub fn hsl(&self) -> String {
        let (rf, gf, bf) = (self.r as f64 / 255.0, self.g as f64 / 255.0, self.b as f64 / 255.0);
        let mx = rf.max(gf).max(bf);
        let mn = rf.min(gf).min(bf);
        let l = (mx + mn) / 2.0;
        let (mut h, mut s) = (0.0, 0.0);
        if mx != mn {
            let d = mx - mn;
            s = if l > 0.5 { d / (2.0 - mx - mn) } else { d / (mx + mn) };
            h = if mx == rf {
                (gf - bf) / d + if gf < bf { 6.0 } else { 0.0 }
            } else if mx == gf {
                (bf - rf) / d + 2.0
            } else {
                (rf - gf) / d + 4.0
            };
            h /= 6.0;
        }
        format!("hsl({}, {}%, {}%)", (h * 360.0).round() as i64, (s * 100.0).round() as i64, (l * 100.0).round() as i64)
    }
}

/// `#3B82F6`, `#38f` and `rgb(59, 130, 246)` (`SmartClip.parseColor`).
pub fn parse_color(text: &str) -> Option<ParsedColor> {
    let t = text.trim_matches(|c| c == ' ' || c == '\t').to_lowercase();
    if let Some(hex) = t.strip_prefix('#') {
        if (hex.len() == 3 || hex.len() == 6) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            let full: String = if hex.len() == 3 { hex.chars().flat_map(|c| [c, c]).collect() } else { hex.to_string() };
            let v = u32::from_str_radix(&full, 16).ok()?;
            return Some(ParsedColor { r: (v >> 16 & 0xFF) as u8, g: (v >> 8 & 0xFF) as u8, b: (v & 0xFF) as u8 });
        }
    }
    if t.starts_with("rgb") {
        let open = t.find('(')?;
        let close = t.rfind(')')?;
        if close > open {
            let nums: Vec<i64> = t[open + 1..close].split(',').filter_map(|p| p.trim_matches(|c| c == ' ' || c == '\t').parse::<i64>().ok()).collect();
            if nums.len() >= 3 && nums[..3].iter().all(|n| (0..=255).contains(n)) {
                return Some(ParsedColor { r: nums[0] as u8, g: nums[1] as u8, b: nums[2] as u8 });
            }
        }
    }
    None
}

// ---------------------------------------------------------------- tracking code

/// Correios and the international pattern (`AA123456789BR`) and UPS (`1Z...`): the code in capitals,
/// without spaces, or `None`.
pub fn tracking_code(text: &str) -> Option<String> {
    let t: String = text.to_uppercase().replace(' ', "");
    let b = t.as_bytes();
    let intl = b.len() == 13
        && b[..2].iter().all(|c| c.is_ascii_uppercase())
        && b[2..11].iter().all(|c| c.is_ascii_digit())
        && b[11..].iter().all(|c| c.is_ascii_uppercase());
    let ups = b.len() == 18 && t.starts_with("1Z") && b[2..].iter().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    if intl || ups {
        Some(t)
    } else {
        None
    }
}

// ---------------------------------------------------------------- locale

/// The calling code of the countries of `SmartClip.countryCodes`.
pub fn country_code(region: &str) -> Option<&'static str> {
    Some(match region.to_ascii_uppercase().as_str() {
        "BR" => "55",
        "PT" => "351",
        "US" | "CA" => "1",
        "ES" => "34",
        "DE" => "49",
        "MX" => "52",
        "AR" => "54",
        "GB" => "44",
        "FR" => "33",
        "IT" => "39",
        "CN" => "86",
        "IN" => "91",
        _ => return None,
    })
}

/// The country of a locale name: `pt_BR.UTF-8` -> `BR`.
pub fn region_of_locale(locale: &str) -> Option<String> {
    let base = locale.split(['.', '@']).next()?;
    let region = base.split(['_', '-']).nth(1)?;
    if region.len() == 2 && region.chars().all(|c| c.is_ascii_alphabetic()) {
        Some(region.to_ascii_uppercase())
    } else {
        None
    }
}

/// The region from `LC_ALL`, `LC_MESSAGES` or `LANG`.
pub fn region_from_env() -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.is_empty())
        .and_then(|v| region_of_locale(&v))
}

// ---------------------------------------------------------------- detector replacements

fn trim_trailing_punctuation(tok: &str) -> &str {
    tok.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '}', '>', '"', '\''])
}

/// `name@host.tld`.
pub fn email_address(tok: &str) -> Option<String> {
    let (local, host) = tok.split_once('@')?;
    if local.is_empty() || host.contains('@') {
        return None;
    }
    let local_ok = local.chars().all(|c| c.is_ascii_alphanumeric() || "._%+-".contains(c));
    let labels: Vec<&str> = host.split('.').collect();
    let host_ok = labels.len() >= 2
        && labels.iter().all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        && labels.last().map(|l| l.len() >= 2 && l.chars().all(|c| c.is_ascii_alphabetic())).unwrap_or(false);
    if local_ok && host_ok {
        Some(tok.to_string())
    } else {
        None
    }
}

const TLDS: &[&str] = &[
    "com", "org", "net", "io", "dev", "app", "edu", "gov", "co", "me", "info", "biz", "br", "pt", "de", "es", "fr", "it", "uk", "us", "cn",
    "jp", "in", "ru", "nl", "ca", "au", "ch", "eu", "ai", "tv", "xyz", "cloud", "tech", "online", "site", "store", "blog", "page", "link",
];

/// An address the default browser can open: `scheme://...`, `www.x.y` or `x.com/path` (the last one only
/// with a well known ending, so that `file.txt` is not a link). Returns the address with its scheme.
pub fn web_link(tok: &str) -> Option<String> {
    if let Some((scheme, rest)) = tok.split_once("://") {
        let scheme_ok = !scheme.is_empty() && scheme.chars().all(|c| c.is_ascii_alphabetic()) && scheme.len() <= 10;
        if scheme_ok && !rest.is_empty() && !rest.starts_with('/') {
            return Some(tok.to_string());
        }
        return None;
    }
    let host = tok.split(['/', '?', '#']).next().unwrap_or("");
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 || !labels.iter().all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')) {
        return None;
    }
    let tld = labels.last()?.to_ascii_lowercase();
    if labels[0].eq_ignore_ascii_case("www") || TLDS.contains(&tld.as_str()) {
        Some(format!("http://{tok}"))
    } else {
        None
    }
}

/// A phone number: digits with spaces, dashes, dots and parentheses, an optional `+`. 10 to 15 digits,
/// or 8 or 9 after a `+`. Dates and IP addresses are not phone numbers. The text is the number
/// as typed (trimmed).
pub fn phone_number(text: &str) -> Option<String> {
    let t = text.trim();
    let allowed = |c: char| c.is_ascii_digit() || " -.()+".contains(c);
    if t.is_empty() || !t.chars().all(allowed) {
        return None;
    }
    if t.matches('+').count() > 1 || (t.contains('+') && !t.starts_with('+')) {
        return None;
    }
    let digits = t.chars().filter(char::is_ascii_digit).count();
    let ok = (10..=15).contains(&digits) || (t.starts_with('+') && (8..=9).contains(&digits));
    if !ok {
        return None;
    }
    // 192.168.0.1 and 2026-11-14: separators of one kind only between short groups
    let groups: Vec<&str> = t.split(|c: char| !c.is_ascii_digit()).filter(|g| !g.is_empty()).collect();
    if t.contains('.') && !t.contains(' ') && !t.contains('-') && !t.contains('(') && groups.len() == 4 && groups.iter().all(|g| g.len() <= 3) {
        return None;
    }
    if groups.len() == 3 && groups[0].len() == 4 && groups[1].len() <= 2 && groups[2].len() <= 2 && !t.contains('+') {
        return None;
    }
    Some(t.to_string())
}

const STREET_WORDS: &[&str] = &[
    // en
    "street", "st", "avenue", "ave", "road", "rd", "boulevard", "blvd", "lane", "drive", "highway", "hwy", "square",
    // pt
    "rua", "avenida", "av", "alameda", "travessa", "pra\u{e7}a", "praca", "rodovia", "estrada", "largo", "beco",
    // es
    "calle", "plaza", "paseo", "carrera", "camino", "carretera", "avda",
    // de
    "stra\u{df}e", "strasse", "str", "platz", "weg", "allee", "gasse", "damm",
    // zh
    "\u{8def}", "\u{8857}", "\u{5927}\u{9053}", "\u{5df7}",
];

/// A street address: a street word (`Rua`, `Av.`, `Street`, `Calle`, `Stra\u{df}e`, `\u{8def}`...) and a house number or postal
/// code. A heuristic: Apple's detector knows far more. Returns the address as typed (trimmed).
pub fn street_address(text: &str) -> Option<String> {
    let t = text.trim();
    if t.chars().count() < 8 || t.contains("://") || t.contains('@') {
        return None;
    }
    let lower = t.to_lowercase();
    let has_number = t.chars().any(|c| c.is_ascii_digit());
    if !has_number || t.split_whitespace().count() < 2 {
        return None;
    }
    let words: Vec<String> = lower
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .map(|w| w.trim_matches('.').to_string())
        .filter(|w| !w.is_empty())
        .collect();
    let cjk_hit = ["\u{8def}", "\u{8857}", "\u{5927}\u{9053}", "\u{5df7}"].iter().any(|s| lower.contains(s));
    let word_hit = words.iter().any(|w| {
        STREET_WORDS.iter().any(|s| w == s) || (w.ends_with("stra\u{df}e") || w.ends_with("strasse") || w.ends_with("platz") || w.ends_with("gasse"))
    });
    if word_hit || cjk_hit {
        Some(t.to_string())
    } else {
        None
    }
}

/// `String.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed)`.
pub fn percent_encode_query(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        let c = b as char;
        if c.is_ascii_alphanumeric() || "-._~!$&'()*+,;=:@/?".contains(c) {
            out.push(c);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(text: &str) -> Vec<String> {
        actions(text, Language::En, Some("BR")).into_iter().map(|a| a.id).collect()
    }

    #[test]
    fn colors_follow_the_mac() {
        assert_eq!(ids("#3B82F6"), vec!["color.RGB", "color.HSL"]);
        let a = actions("#3b82f6", Language::En, None);
        // lowercase text: the HEX in capitals is the same value, so it is not offered again
        assert_eq!(a.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), vec!["color.RGB", "color.HSL"]);
        assert_eq!(parse_color("#38f").unwrap().hex(), "#3388FF");
        assert_eq!(parse_color("rgb(59, 130, 246)").unwrap().hsl(), "hsl(217, 91%, 60%)");
        assert!(parse_color("#12345").is_none());
        assert!(parse_color("rgb(1, 2, 300)").is_none());
    }

    #[test]
    fn meeting_links() {
        assert_eq!(meeting_link("join https://us02web.zoom.us/j/123456?pwd=abc now").unwrap(), "https://us02web.zoom.us/j/123456?pwd=abc");
        assert_eq!(meeting_link("https://meet.google.com/abc-defg-hij").unwrap(), "https://meet.google.com/abc-defg-hij");
        assert_eq!(meeting_link("HTTPS://Acme.Webex.com/meet/x").unwrap(), "HTTPS://Acme.Webex.com/meet/x");
        assert!(meeting_link("https://example.com/j/1").is_none());
        assert!(meeting_link("http://meet.google.com/abc").is_none());
    }

    #[test]
    fn tracking_codes() {
        assert_eq!(tracking_code("ab 123456789 br").unwrap(), "AB123456789BR");
        assert_eq!(tracking_code("1Z999AA10123456784").unwrap(), "1Z999AA10123456784");
        assert!(tracking_code("AB12345678BR").is_none());
    }

    #[test]
    fn phone_and_whatsapp() {
        assert_eq!(ids("+55 11 98765-4321"), vec!["phone", "whatsapp"]);
        assert_eq!(ids("(11) 98765-4321"), vec!["phone", "whatsapp"]);
        assert_eq!(actions("(11) 98765-4321", Language::En, None).len(), 1);
        assert!(ids("2026-11-14").is_empty());
        assert!(ids("192.168.100.100").is_empty());
        assert!(ids("call me at 11 98765-4321 tomorrow please").is_empty());
    }

    #[test]
    fn mail_link_and_address() {
        assert_eq!(ids("hello@example.com"), vec!["mail"]);
        assert_eq!(ids("https://github.com/Quost/menew"), vec!["link"]);
        assert_eq!(ids("www.example.org"), vec!["link"]);
        assert!(ids("file.txt").is_empty());
        assert_eq!(ids("Av. Paulista, 1578 - Bela Vista, S\u{e3}o Paulo"), vec!["address"]);
        assert!(ids("I walked down the street yesterday").is_empty());
    }

    #[test]
    fn long_text_has_no_actions() {
        assert!(actions(&"a".repeat(401), Language::En, None).is_empty());
        assert!(actions("   ", Language::En, None).is_empty());
    }

    #[test]
    fn locale_region() {
        assert_eq!(region_of_locale("pt_BR.UTF-8").as_deref(), Some("BR"));
        assert_eq!(region_of_locale("C.UTF-8"), None);
        assert_eq!(region_of_locale("en"), None);
    }
}
