//! Palette search (port of `PaletteSearch` in `Palette.swift`): case and diacritic insensitive,
//! with the same scores so the same things come out on top.

/// Lowercase and strip diacritics ("Ação" -> "acao").
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        for l in c.to_lowercase() {
            out.push(strip(l));
        }
    }
    out
}

pub fn strip(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'ď' | 'đ' => 'd',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
        'ĥ' | 'ħ' => 'h',
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => 'i',
        'ĵ' => 'j',
        'ķ' => 'k',
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => 'l',
        'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => 'o',
        'ŕ' | 'ŗ' | 'ř' => 'r',
        'ś' | 'ŝ' | 'ş' | 'š' => 's',
        'ţ' | 'ť' | 'ŧ' => 't',
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'ŵ' => 'w',
        'ý' | 'ÿ' | 'ŷ' => 'y',
        'ź' | 'ż' | 'ž' => 'z',
        other => other,
    }
}

/// Words of a text: runs of letters and digits.
pub fn words(s: &str) -> Vec<&str> {
    s.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect()
}

fn is_subsequence(q: &str, t: &str) -> bool {
    let mut it = t.chars();
    q.chars().all(|c| it.any(|x| x == c))
}

/// `None` = no match. The higher, the closer to the top. `query` must already be folded.
pub fn score(query: &str, title: &str, keywords: &str) -> Option<i32> {
    let t = fold(title);
    let tokens: Vec<&str> = query.split(' ').filter(|s| !s.is_empty()).collect();
    if tokens.len() > 1 {
        // "janela esquerda": every word must appear
        let hay_src = format!("{} {}", t, fold(keywords));
        let hay = words(&hay_src);
        return if tokens.iter().all(|tk| hay.iter().any(|w| w.starts_with(tk))) {
            Some(500)
        } else {
            None
        };
    }
    let q = query;
    let tlen = t.chars().count() as i32;
    if t == q {
        return Some(1000);
    }
    if t.starts_with(q) {
        return Some(800 - tlen);
    }
    let ws = words(&t);
    if ws.iter().any(|w| w.starts_with(q)) {
        return Some(650 - tlen);
    }
    if q.chars().count() >= 2 {
        let initials: String = ws.iter().filter_map(|w| w.chars().next()).collect();
        if initials.starts_with(q) {
            return Some(600); // "mt" -> Microsoft Teams
        }
    }
    if t.contains(q) {
        return Some(450 - tlen);
    }
    let kw = fold(keywords);
    if words(&kw).iter().any(|w| w.starts_with(q)) {
        return Some(350);
    }
    if q.chars().count() >= 3 && is_subsequence(q, &t) {
        return Some(150 - tlen);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fold_strips_case_and_accents() {
        assert_eq!(fold("Ação Ümlaut"), "acao umlaut");
        assert_eq!(fold("CAFÉ"), "cafe");
    }

    #[test]
    fn scores_follow_the_mac_rules() {
        assert_eq!(score("firefox", "Firefox", ""), Some(1000));
        assert_eq!(score("fire", "Firefox", ""), Some(800 - 7));
        assert_eq!(score("teams", "Microsoft Teams", ""), Some(650 - 15));
        assert_eq!(score("mt", "Microsoft Teams", ""), Some(600));
        assert_eq!(score("soft", "Microsoft Teams", ""), Some(450 - 15));
        assert_eq!(score("zzz", "Microsoft Teams", "zzzword"), Some(350));
        assert_eq!(score("mcs", "Microsoft Teams", ""), Some(150 - 15));
        assert_eq!(score("qq", "Microsoft Teams", ""), None);
        // several words: all must appear
        assert_eq!(score("janela esq", "Janela", "esquerda"), Some(500));
        assert_eq!(score("janela dir", "Janela", "esquerda"), None);
    }
}
