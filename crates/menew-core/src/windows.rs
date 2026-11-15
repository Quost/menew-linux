//! Window positions in the palette (port of the `Windows.swift` palette items). The snapping itself
//! is in `menew-desktop`; here only the items, with the ids `win.<position>`.

use crate::i18n::Language;
use crate::palette::{Icon, ItemAction, Kind, PaletteItem};
use crate::strings::tr;

/// (position id, title key, extra keywords)
const POSITIONS: [(&str, &str, &str); 17] = [
    ("left", "win_left", "esquerda left links izquierda 左"),
    ("right", "win_right", "direita right rechts derecha 右"),
    ("top", "win_top", "cima top oben arriba 上"),
    ("bottom", "win_bottom", "baixo bottom unten abajo 下"),
    ("max", "win_max", "maximizar maximize cheia full maximieren maximizar 最大化"),
    ("almost", "win_almost", "quase almost fast casi 几乎"),
    ("center", "win_center", "centro center zentrieren centrar 居中"),
    ("topleft", "win_topleft", "canto corner ecke esquina 角"),
    ("topright", "win_topright", "canto corner ecke esquina 角"),
    ("bottomleft", "win_bottomleft", "canto corner ecke esquina 角"),
    ("bottomright", "win_bottomright", "canto corner ecke esquina 角"),
    ("leftthird", "win_leftthird", "terço third drittel tercio 三分之一"),
    ("centerthird", "win_centerthird", "terço third drittel tercio 三分之一"),
    ("rightthird", "win_rightthird", "terço third drittel tercio 三分之一"),
    ("lefttwothirds", "win_lefttwothirds", "dois terços two thirds zwei drittel dos tercios 三分之二"),
    ("righttwothirds", "win_righttwothirds", "dois terços two thirds zwei drittel dos tercios 三分之二"),
    ("display", "win_display", "monitor tela display screen bildschirm pantalla 显示器"),
];

pub fn items(lang: Language) -> Vec<PaletteItem> {
    POSITIONS
        .iter()
        .map(|(id, key, kw)| {
            let mut it = PaletteItem::new(&format!("window.{id}"), Kind::Window, &tr(key, lang, &[]))
                .kw(&format!("janela window fenster ventana 窗口 snap encaixar {kw}"))
                .icon(Icon::Symbol("view-grid-symbolic"))
                .act(ItemAction::Command(format!("win.{id}")));
            let sc = match *id {
                "left" => Some("Ctrl+Alt+Shift+←"),
                "right" => Some("Ctrl+Alt+Shift+→"),
                "top" => Some("Ctrl+Alt+Shift+↑"),
                "bottom" => Some("Ctrl+Alt+Shift+↓"),
                "max" => Some("Ctrl+Alt+↩"),
                _ => None,
            };
            if let Some(s) = sc {
                it = it.shortcut(s);
            }
            it
        })
        .collect()
}

pub fn ids() -> impl Iterator<Item = &'static str> {
    POSITIONS.iter().map(|p| p.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_item_per_position() {
        let it = items(Language::Pt);
        assert_eq!(it.len(), 17);
        assert_eq!(it[0].action, ItemAction::Command("win.left".into()));
        assert_eq!(it[0].title, "Janela na metade esquerda");
    }
}
