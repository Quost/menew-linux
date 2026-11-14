//! Quick links in the palette (port of `QuickLinks.swift`): "jira ABC-123", "gh menew", "maps avenida
//! paulista". Each one is a keyword and an address with `{q}` where the typed text goes. They open
//! in the browser; Menew itself never goes to the internet for this.

use crate::i18n::Language;
use crate::palette::{Icon, ItemAction, Kind, PaletteItem};
use crate::strings::tr;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickLink {
    pub keyword: String,
    pub name: String,
    pub url: String,
}

impl QuickLink {
    pub fn new(keyword: &str, name: &str, url: &str) -> Self {
        Self { keyword: keyword.into(), name: name.into(), url: url.into() }
    }

    /// The address with the text in place of `{q}`; without `{q}` the address is opened as it is.
    pub fn url_for(&self, text: &str) -> Option<String> {
        let u = self.url.replace("{q}", &encode_query(text));
        valid_url(&u).then_some(u)
    }

    pub fn needs_text(&self) -> bool {
        self.url.contains("{q}")
    }
}

/// Same list as the Mac app.
pub fn defaults() -> Vec<QuickLink> {
    vec![
        QuickLink::new("g", "Google", "https://www.google.com/search?q={q}"),
        QuickLink::new("ddg", "DuckDuckGo", "https://duckduckgo.com/?q={q}"),
        QuickLink::new("yt", "YouTube", "https://www.youtube.com/results?search_query={q}"),
        QuickLink::new("wiki", "Wikipedia", "https://wikipedia.org/w/index.php?search={q}"),
        QuickLink::new("gh", "GitHub", "https://github.com/search?q={q}"),
        QuickLink::new("maps", "OpenStreetMap", "https://www.openstreetmap.org/search?query={q}"),
    ]
}

/// Percent-encoding like `.urlQueryAllowed` minus `& + = ? #` (what Swift does for `{q}`).
fn encode_query(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        let keep = b.is_ascii_alphanumeric() || b"-._~!$'()*,/:;@".contains(&b);
        if keep {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

fn valid_url(u: &str) -> bool {
    match u.split_once("://") {
        Some((scheme, rest)) => {
            !scheme.is_empty()
                && scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
                && !rest.is_empty()
                && !u.chars().any(|c| c.is_whitespace() || c.is_control())
        }
        None => false,
    }
}

/// Host of an http(s) URL string.
fn host_of(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let auth = rest.split(['/', '?', '#']).next()?;
    let auth = auth.rsplit('@').next()?;
    let host = auth.split(':').next()?;
    (!host.is_empty()).then(|| host.to_string())
}

/// "github.com/quost", "https://example.com": a typed address.
pub fn web_address(text: &str) -> Option<String> {
    let full = if text.starts_with("http://") || text.starts_with("https://") { text.to_string() } else { format!("https://{text}") };
    if !valid_url(&full) {
        return None;
    }
    let host = host_of(&full)?;
    if !host.contains('.') || host.split('.').any(|l| l.is_empty()) {
        return None;
    }
    let tld = host.rsplit('.').next()?;
    (tld.chars().count() >= 2 && tld.chars().all(|c| c.is_alphabetic())).then_some(full)
}

/// Palette items for the text as typed (with its capitals: "jira ABC-123").
pub fn items(links: &[QuickLink], raw: &str, lang: Language) -> Vec<PaletteItem> {
    let text = raw.trim();
    if text.is_empty() {
        return vec![];
    }
    let mut out = vec![];
    let word = text.split(' ').next().unwrap_or("");
    let rest = text[word.len()..].trim();

    for link in links.iter().filter(|l| l.keyword.to_lowercase() == word.to_lowercase()) {
        let id = format!("link.{}", link.keyword);
        if link.needs_text() && rest.is_empty() {
            out.push(
                PaletteItem::new(&id, Kind::Calc, &tr("link_type", lang, &[&link.name]))
                    .subtitle(&link.url)
                    .kw(text)
                    .icon(Icon::Symbol("emblem-symbolic-link"))
                    .forget()
                    .keep_open()
                    .act(ItemAction::None),
            );
        } else if let Some(url) = link.url_for(rest) {
            let title = if rest.is_empty() { link.name.clone() } else { tr("link_pair", lang, &[&link.name, rest]) };
            out.push(
                PaletteItem::new(&id, Kind::Calc, &title)
                    .subtitle(&url)
                    .kw(text)
                    .icon(Icon::Symbol("emblem-symbolic-link"))
                    .forget()
                    .act(ItemAction::OpenUri(url)),
            );
        }
    }

    // a typed address ("github.com/quost", "https://...") opens directly
    if out.is_empty() && !text.contains(' ') && text.contains('.') {
        if let Some(url) = web_address(text) {
            let host = host_of(&url).unwrap_or_else(|| text.to_string());
            out.push(
                PaletteItem::new("link.open", Kind::Calc, &tr("link_open", lang, &[&host]))
                    .subtitle(&url)
                    .kw(text)
                    .icon(Icon::Symbol("web-browser-symbolic"))
                    .forget()
                    .act(ItemAction::OpenUri(url)),
            );
        }
    }
    out
}

/// At the end of the list: search what was typed on the web, with the first search link.
pub fn web_search_item(links: &[QuickLink], raw: &str, lang: Language) -> Option<PaletteItem> {
    let text = raw.trim();
    let word = text.split(' ').find(|s| !s.is_empty()).unwrap_or("").to_lowercase();
    if text.chars().count() < 2 || links.iter().any(|l| l.keyword.to_lowercase() == word) {
        return None;
    }
    let engine = links.iter().find(|l| l.needs_text())?;
    let url = engine.url_for(text)?;
    Some(
        PaletteItem::new("link.search", Kind::Action, &tr("link_search", lang, &[text, &engine.name]))
            .subtitle(&url)
            .icon(Icon::Symbol("system-search-symbolic"))
            .forget()
            .act(ItemAction::OpenUri(url)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(k: &str) -> QuickLink {
        defaults().into_iter().find(|l| l.keyword == k).unwrap()
    }

    // The vectors V066 to V076 of docs/palette-spec.md
    #[test]
    fn urls_for_the_default_links() {
        assert_eq!(link("g").url_for("coffee").as_deref(), Some("https://www.google.com/search?q=coffee"));
        assert_eq!(link("gh").url_for("menew").as_deref(), Some("https://github.com/search?q=menew"));
        assert_eq!(link("maps").url_for("paulista").as_deref(), Some("https://www.openstreetmap.org/search?query=paulista"));
        assert_eq!(link("maps").url_for("avenida paulista").as_deref(), Some("https://www.openstreetmap.org/search?query=avenida%20paulista"));
        assert_eq!(link("yt").url_for("lo-fi beats").as_deref(), Some("https://www.youtube.com/results?search_query=lo-fi%20beats"));
        assert_eq!(link("wiki").url_for("ação").as_deref(), Some("https://wikipedia.org/w/index.php?search=a%C3%A7%C3%A3o"));
        assert_eq!(link("g").url_for("c++ & rust").as_deref(), Some("https://www.google.com/search?q=c%2B%2B%20%26%20rust"));
        // the characters urlQueryAllowed keeps stay as they are
        assert_eq!(link("g").url_for("a/b:c;d@e").as_deref(), Some("https://www.google.com/search?q=a/b:c;d@e"));
        assert_eq!(link("g").url_for("50%").as_deref(), Some("https://www.google.com/search?q=50%25"));
    }

    #[test]
    fn a_link_without_q_opens_as_it_is() {
        let l = QuickLink::new("home", "Home", "https://example.com/");
        assert!(!l.needs_text());
        assert_eq!(l.url_for("ignored").as_deref(), Some("https://example.com/"));
    }

    #[test]
    fn typed_addresses() {
        assert_eq!(web_address("github.com/quost").as_deref(), Some("https://github.com/quost"));
        assert_eq!(web_address("https://example.com").as_deref(), Some("https://example.com"));
        assert_eq!(web_address("http://a.b.co/x?y=1#z").as_deref(), Some("http://a.b.co/x?y=1#z"));
        assert_eq!(web_address("localhost"), None);
        assert_eq!(web_address("file.x1"), None);
        assert_eq!(web_address("1.2"), None);
        assert_eq!(web_address("a..com"), None);
    }

    #[test]
    fn items_and_web_search() {
        let mut links = defaults();
        links.push(QuickLink::new("jira", "Jira", "https://company.atlassian.net/browse/{q}"));
        let en = Language::En;
        let it = items(&links, "jira ABC-123", en);
        assert_eq!(it.len(), 1);
        assert_eq!(it[0].title, "Jira: ABC-123");
        assert_eq!(it[0].action, ItemAction::OpenUri("https://company.atlassian.net/browse/ABC-123".into()));
        // a keyword with nothing after it asks for the text and does not close the palette
        let hint = items(&links, "g", en);
        assert_eq!(hint[0].title, "Google: type what to search");
        assert!(hint[0].keep_open);
        assert!(web_search_item(&links, "g coffee", en).is_none());
        assert_eq!(web_search_item(&links, "coffee shop", en).unwrap().title, "Search “coffee shop” on Google");
        assert!(web_search_item(&links, "x", en).is_none());
        assert!(items(&links, "", en).is_empty());
    }
}
