//! Emoji and symbols in the palette (port of `Emoji.swift`): "emoji rocket", ":fire" for emoji;
//! "arrow", "degree", "euro", "check" for symbols. The names are the Unicode names (generated into
//! `emoji_data.rs` with the Mac's filter); the words in other languages come from the Mac's table.

use crate::emoji_data::EMOJI;
use crate::i18n::Language;
use crate::palette::{Icon, ItemAction, Kind, PaletteItem};
use crate::search::{fold, score};
use crate::strings::tr;

/// Words in Portuguese, German, Spanish and Chinese for the English names.
const WORDS: &[(&str, &str)] = &[
    ("coracao", "heart"), ("coração", "heart"), ("amor", "heart"), ("herz", "heart"), ("corazon", "heart"), ("corazón", "heart"), ("心", "heart"),
    ("foguete", "rocket"), ("rakete", "rocket"), ("cohete", "rocket"), ("火箭", "rocket"),
    ("risada", "joy"), ("rindo", "laughing"), ("rir", "laughing"), ("lachen", "laughing"), ("risa", "laughing"), ("笑", "laughing"),
    ("sorriso", "smiling"), ("feliz", "smiling"), ("lächeln", "smiling"), ("sonrisa", "smiling"), ("微笑", "smiling"),
    ("triste", "crying"), ("chorando", "crying"), ("weinen", "crying"), ("llorando", "crying"), ("哭", "crying"),
    ("fogo", "fire"), ("feuer", "fire"), ("fuego", "fire"), ("火", "fire"),
    ("joinha", "thumbs up"), ("positivo", "thumbs up"), ("daumen", "thumbs up"), ("pulgar", "thumbs up"), ("赞", "thumbs up"),
    ("palmas", "clapping"), ("aplausos", "clapping"), ("klatschen", "clapping"), ("鼓掌", "clapping"),
    ("festa", "party"), ("comemorar", "party"), ("party", "party"), ("fiesta", "party"), ("派对", "party"),
    ("estrela", "star"), ("stern", "star"), ("estrella", "star"), ("星", "star"),
    ("check", "check mark"), ("certo", "check mark"), ("ok", "ok"), ("feito", "check mark"), ("erledigt", "check mark"), ("hecho", "check mark"),
    ("errado", "cross mark"), ("xis", "cross mark"), ("falsch", "cross mark"), ("mal", "cross mark"),
    ("cafe", "hot beverage"), ("café", "hot beverage"), ("kaffee", "hot beverage"), ("咖啡", "hot beverage"),
    ("cerveja", "beer"), ("bier", "beer"), ("cerveza", "beer"), ("啤酒", "beer"),
    ("olhos", "eyes"), ("augen", "eyes"), ("ojos", "eyes"), ("眼睛", "eyes"),
    ("pensando", "thinking"), ("denken", "thinking"), ("pensar", "thinking"), ("思考", "thinking"),
    ("oracao", "folded hands"), ("oração", "folded hands"), ("obrigado", "folded hands"), ("beten", "folded hands"), ("gracias", "folded hands"), ("谢谢", "folded hands"),
    ("musculo", "biceps"), ("músculo", "biceps"), ("forca", "biceps"), ("força", "biceps"), ("muskel", "biceps"), ("fuerza", "biceps"),
    ("cachorro", "dog"), ("hund", "dog"), ("perro", "dog"), ("狗", "dog"), ("gato", "cat"), ("katze", "cat"), ("猫", "cat"),
    ("sol", "sun"), ("sonne", "sun"), ("太阳", "sun"), ("chuva", "rain"), ("regen", "rain"), ("lluvia", "rain"), ("雨", "rain"),
    ("dinheiro", "money"), ("geld", "money"), ("dinero", "money"), ("钱", "money"),
    ("presente", "gift"), ("geschenk", "gift"), ("regalo", "gift"), ("礼物", "gift"),
    ("alerta", "warning"), ("aviso", "warning"), ("warnung", "warning"), ("警告", "warning"),
    ("ideia", "light bulb"), ("idee", "light bulb"), ("idea", "light bulb"), ("想法", "light bulb"),
    ("relogio", "clock"), ("relógio", "clock"), ("uhr", "clock"), ("reloj", "clock"), ("时钟", "clock"),
    ("computador", "computer"), ("computer", "computer"), ("电脑", "computer"), ("telefone", "telephone"), ("celular", "mobile phone"),
    ("pizza", "pizza"), ("bolo", "cake"), ("kuchen", "cake"), ("pastel", "cake"), ("蛋糕", "cake"),
    ("brasil", "flag"), ("100", "hundred points"), ("caveira", "skull"), ("fantasma", "ghost"), ("cocô", "pile of poo"), ("coco", "pile of poo"),
];

const PREFERRED: &[(&str, &str)] = &[
    ("heart", "\u{2764}\u{FE0F}"), ("thumbs up", "👍"), ("joy", "😂"), ("smiling", "😊"), ("crying", "😢"), ("fire", "🔥"),
    ("party", "🎉"), ("star", "⭐"), ("check mark", "✅"), ("hot beverage", "☕"), ("thinking", "🤔"), ("folded hands", "🙏"),
];

pub const PREFIXES: [&str; 3] = ["emoji ", "emojis ", ":"];

/// Every emoji with its lowercase Unicode name, plus the red heart with its variation selector.
fn index() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = EMOJI.iter().map(|(c, n)| (c.to_string(), n.to_string())).collect();
    v.push(("\u{2764}\u{FE0F}".to_string(), "red heart".to_string()));
    v
}

/// Up to 8 emoji for the text after "emoji " or ":".
pub fn emoji_for(raw: &str) -> Vec<(String, String)> {
    let q = fold(raw.trim());
    if q.chars().count() < 2 {
        return vec![];
    }
    let english: String = WORDS
        .iter()
        .find(|(k, _)| *k == q)
        .map(|(_, v)| *v)
        .or_else(|| {
            if q.chars().count() >= 4 {
                WORDS.iter().find(|(k, _)| k.starts_with(&q)).map(|(_, v)| *v)
            } else {
                None
            }
        })
        .map(String::from)
        .unwrap_or_else(|| q.clone());
    let mut hits: Vec<(String, String)> = index()
        .into_iter()
        .filter(|(_, name)| name.split(' ').any(|w| w.starts_with(&english)) || name.contains(&english))
        .collect();
    let preferred = PREFERRED.iter().find(|(k, _)| *k == english).map(|(_, c)| *c);
    let rank = |c: &str, name: &str| -> u8 {
        if preferred == Some(c) {
            0
        } else if name.split(' ').any(|w| w == english) {
            1
        } else {
            2
        }
    };
    hits.sort_by_key(|(c, n)| (rank(c, n), n.chars().count()));
    hits.truncate(8);
    hits
}

/// Symbols that have no key, with words in the five languages. (The Apple logo of the Mac list is
/// left out: it only exists in Apple's fonts.)
pub const SYMBOLS: &[(&str, &str)] = &[
    ("→", "seta direita arrow right pfeil rechts flecha derecha 箭头 右"), ("←", "seta esquerda arrow left pfeil links flecha izquierda 箭头 左"),
    ("↑", "seta cima arrow up pfeil oben flecha arriba 箭头 上"), ("↓", "seta baixo arrow down pfeil unten flecha abajo 箭头 下"),
    ("⇒", "seta dupla implica implies doppelpfeil flecha doble"), ("↔", "seta dupla ambos arrow both"),
    ("✓", "check certo visto tick haken marca 勾"), ("✗", "xis errado cross falsch cruz 叉"),
    ("•", "ponto bolinha bullet punkt viñeta 圆点"), ("…", "reticencias reticências ellipsis auslassung puntos suspensivos 省略号"),
    ("\u{2014}", "travessao travessão em dash gedankenstrich raya 破折号"), ("–", "meia risca en dash halbgeviertstrich guion"),
    ("°", "grau graus degree grad grado 度"), ("±", "mais ou menos plus minus más menos 正负"),
    ("×", "vezes multiplicacao multiplicação times mal por 乘"), ("÷", "divisao divisão divide geteilt dividido 除"),
    ("≈", "aproximadamente approximately ungefähr aproximado 约等于"), ("≠", "diferente not equal ungleich distinto 不等于"),
    ("≤", "menor ou igual less equal kleiner gleich menor igual 小于等于"), ("≥", "maior ou igual greater equal größer gleich mayor igual 大于等于"),
    ("∞", "infinito infinity unendlich 无穷"), ("π", "pi"), ("√", "raiz quadrada square root wurzel raíz 根号"),
    ("½", "meio metade half halb medio 二分之一"), ("¼", "um quarto quarter viertel cuarto"), ("¾", "tres quartos três quartos three quarters"),
    ("²", "ao quadrado squared hoch zwei cuadrado 平方"), ("³", "ao cubo cubed hoch drei cubo 立方"),
    ("€", "euro"), ("£", "libra pound pfund"), ("¥", "iene yen yuan 元"), ("₿", "bitcoin"),
    ("©", "copyright direitos autorais derechos"), ("®", "registrado registered eingetragen registrado"), ("™", "marca registrada trademark marke"),
    ("§", "paragrafo parágrafo section paragraph párrafo"), ("¶", "pilcrow parágrafo"),
    ("⌘", "command comando cmd befehl"), ("⌥", "option opcao opção alt"), ("⌃", "control controle ctrl steuerung"),
    ("⇧", "shift maiusculas maiúsculas umschalt mayúsculas"), ("⏎", "return enter"), ("⌫", "delete apagar backspace löschen borrar"),
];

/// "rocket" -> "Rocket", "red heart" -> "Red Heart"
fn capitalized(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut cs = w.chars();
            match cs.next() {
                Some(f) => f.to_uppercase().collect::<String>() + &cs.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn item(id: String, ch: &str, title: &str, kind: Kind, keywords: &str, lang: Language) -> PaletteItem {
    PaletteItem::new(&id, kind, title)
        .subtitle(&tr("paste_hint", lang, &[]))
        .kw(keywords)
        .icon(Icon::Glyph(ch.to_string()))
        .forget()
        .act(ItemAction::PasteText(ch.to_string()))
}

/// `raw` is the text as typed, `q` the folded query.
pub fn items(raw: &str, q: &str, lang: Language) -> Vec<PaletteItem> {
    let text = raw.trim();
    let lower = text.to_lowercase();
    if let Some(prefix) = PREFIXES.iter().find(|p| lower.starts_with(**p)) {
        let rest: String = text.chars().skip(prefix.chars().count()).collect();
        return emoji_for(&rest)
            .into_iter()
            .map(|(c, n)| item(format!("emoji.{c}"), &c, &capitalized(&n), Kind::Calc, "", lang))
            .collect();
    }
    if q.chars().count() < 3 {
        return vec![];
    }
    SYMBOLS
        .iter()
        .filter(|(_, kw)| score(q, "", kw).is_some())
        .map(|(c, kw)| {
            let parts: Vec<&str> = kw.split(' ').collect();
            let n = if parts.first() == Some(&"seta") { 2 } else { 1 };
            let name = parts.iter().take(n).copied().collect::<Vec<_>>().join(" ");
            item(format!("symbol.{c}"), c, &capitalized(&name), Kind::Action, kw, lang)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first(q: &str) -> Option<(String, String)> {
        emoji_for(q).into_iter().next()
    }

    // The vectors V077 to V081 of docs/palette-spec.md
    #[test]
    fn names_in_five_languages() {
        assert_eq!(first("rocket").map(|e| e.0), Some("🚀".into()));
        assert_eq!(first("foguete").map(|e| e.0), Some("🚀".into()));
        assert_eq!(first("rakete").map(|e| e.0), Some("🚀".into()));
        assert_eq!(first("cohete").map(|e| e.0), Some("🚀".into()));
        assert_eq!(first("火箭").map(|e| e.0), Some("🚀".into()));
        assert_eq!(first("fire").map(|e| e.0), Some("🔥".into()));
        assert_eq!(first("fogo").map(|e| e.0), Some("🔥".into()));
        // the preferred one comes first, ahead of every "heart" in the list
        assert_eq!(first("coração"), Some(("\u{2764}\u{FE0F}".into(), "red heart".into())));
        assert_eq!(first("amor").map(|e| e.1), Some("red heart".into()));
    }

    #[test]
    fn at_most_8_and_at_least_2_characters() {
        assert!(emoji_for("face").len() <= 8);
        assert!(emoji_for("face").len() > 1);
        assert!(emoji_for("f").is_empty());
        assert!(emoji_for("  ").is_empty());
        assert!(emoji_for("qqzzxx").is_empty());
    }

    #[test]
    fn the_list_has_the_same_filter_as_the_mac() {
        // 1F300 to 1F5FF with emoji presentation, and the symbols of 2600 to 27BF that are emoji
        assert!(EMOJI.len() > 1000, "{}", EMOJI.len());
        assert!(EMOJI.iter().any(|(c, n)| *c == "\u{1F680}" && *n == "rocket"));
        assert!(EMOJI.iter().any(|(c, _)| *c == "\u{2705}"));
        // outside the ranges (the trade mark sign) is not in the list
        assert!(!EMOJI.iter().any(|(c, _)| *c == "\u{2122}"));
    }

    #[test]
    fn symbols_by_word() {
        let by = |w: &str| items(w, &fold(w), Language::En).into_iter().map(|i| (i.id, i.title)).collect::<Vec<_>>();
        assert!(by("euro").contains(&("symbol.€".into(), "Euro".into())));
        assert!(by("degree").contains(&("symbol.°".into(), "Grau".into())));
        assert!(by("arrow").contains(&("symbol.→".into(), "Seta Direita".into())));
        assert!(by("check").iter().any(|(id, _)| id == "symbol.✓"));
        // two letters are not enough
        assert!(by("eu").is_empty());
    }

    #[test]
    fn prefixes_pick_the_emoji_list() {
        let its = items(":fire", "fire", Language::En);
        assert_eq!(its[0].id, "emoji.🔥");
        assert_eq!(its[0].title, "Fire");
        assert_eq!(items("emoji rocket", "emoji rocket", Language::En)[0].id, "emoji.🚀");
        assert_eq!(items("emojis rocket", "emojis rocket", Language::En)[0].id, "emoji.🚀");
    }
}
