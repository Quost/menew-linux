use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    En,
    Pt,
    De,
    Es,
    Zh,
}

impl Language {
    pub fn code(&self) -> &'static str {
        match self {
            Language::En => "en",
            Language::Pt => "pt",
            Language::De => "de",
            Language::Es => "es",
            Language::Zh => "zh",
        }
    }

    pub fn from_code(code: &str) -> Self {
        let prefix = code.split(['_', '-', '.']).next().unwrap_or("").to_lowercase();
        match prefix.as_str() {
            "pt" => Language::Pt,
            "de" => Language::De,
            "es" => Language::Es,
            "zh" => Language::Zh,
            _ => Language::En,
        }
    }

    pub fn detect() -> Self {
        if let Ok(lang) = std::env::var("LC_ALL") {
            if !lang.is_empty() {
                return Self::from_code(&lang);
            }
        }
        if let Ok(lang) = std::env::var("LANG") {
            if !lang.is_empty() {
                return Self::from_code(&lang);
            }
        }
        Language::En
    }
}

static CURRENT_LANG: AtomicU8 = AtomicU8::new(0); // 0=En, 1=Pt, 2=De, 3=Es, 4=Zh

pub fn set_language(lang: Language) {
    let val = match lang {
        Language::En => 0,
        Language::Pt => 1,
        Language::De => 2,
        Language::Es => 3,
        Language::Zh => 4,
    };
    CURRENT_LANG.store(val, Ordering::Relaxed);
}

pub fn current_language() -> Language {
    match CURRENT_LANG.load(Ordering::Relaxed) {
        1 => Language::Pt,
        2 => Language::De,
        3 => Language::Es,
        4 => Language::Zh,
        _ => Language::En,
    }
}

pub fn t(key: &'static str) -> &'static str {
    t_in(key, current_language())
}

/// Translate `key` into an explicit language (no global state; used by tests).
pub fn t_in(key: &'static str, lang: Language) -> &'static str {
    match key {
        "app_name" => "Menew",
        "search_placeholder" => match lang {
            Language::Pt => "Digite para buscar aplicativos ou calcular...",
            Language::De => "Tippen Sie, um nach Apps zu suchen oder zu rechnen...",
            Language::Es => "Escribe para buscar aplicaciones o calcular...",
            Language::Zh => "键入以搜索应用或计算...",
            Language::En => "Type to search apps, commands, or calculate...",
        },
        "no_results" => match lang {
            Language::Pt => "Nenhum resultado encontrado",
            Language::De => "Keine Ergebnisse gefunden",
            Language::Es => "No se encontraron resultados",
            Language::Zh => "未找到结果",
            Language::En => "No matching results found",
        },
        "tray_open_palette" => match lang {
            Language::Pt => "Abrir Paleta de Comandos",
            Language::De => "Befehlspalette öffnen",
            Language::Es => "Abrir Paleta de Comandos",
            Language::Zh => "打开命令面板",
            Language::En => "Open Command Palette",
        },
        "tray_quit" => match lang {
            Language::Pt => "Sair do Menew",
            Language::De => "Menew beenden",
            Language::Es => "Salir de Menew",
            Language::Zh => "退出 Menew",
            Language::En => "Quit Menew",
        },
        "tray_clipboard_history" => match lang {
            Language::Pt => "Histórico da Área de Transferência",
            Language::De => "Zwischenablage-Verlauf",
            Language::Es => "Historial del Portapapeles",
            Language::Zh => "剪贴板历史",
            Language::En => "Clipboard History",
        },
        "tray_status_running" => match lang {
            Language::Pt => "Menew está em execução",
            Language::De => "Menew wird ausgeführt",
            Language::Es => "Menew está en ejecución",
            Language::Zh => "Menew 正在运行",
            Language::En => "Menew is running",
        },
        "portal_unavailable_title" => match lang {
            Language::Pt => "Portal de Atalhos Globais Indisponível",
            Language::De => "Portal für globale Tastenkombinationen nicht verfügbar",
            Language::Es => "Portal de Accesos Directos Globales no disponible",
            Language::Zh => "全局快捷键门户不可用",
            Language::En => "Global Shortcuts Portal Unavailable",
        },
        "portal_unavailable_msg" => match lang {
            Language::Pt => "Não foi possível registrar o atalho global via portal. No GNOME 45-47, configure um atalho de teclado personalizado nas Configurações.",
            Language::De => "Globaler Shortcut konnte nicht über das Portal registriert werden. In GNOME 45-47 bitte benutzerdefinierten Shortcut in den Einstellungen anlegen.",
            Language::Es => "No se pudo registrar el acceso directo global mediante el portal. En GNOME 45-47 configure un acceso directo personalizado en Configuración.",
            Language::Zh => "无法通过门户注册全局快捷键。在 GNOME 45-47 上，请在系统设置中配置自定义快捷键。",
            Language::En => "Could not register global shortcut via portal. On GNOME 45-47, please configure a custom shortcut in System Settings.",
        },
        "calc_result" => match lang {
            Language::Pt => "Resultado",
            Language::De => "Ergebnis",
            Language::Es => "Resultado",
            Language::Zh => "计算结果",
            Language::En => "Result",
        },
        "mode_apps" => match lang {
            Language::Pt => "Aplicativos",
            Language::De => "Apps",
            Language::Es => "Aplicaciones",
            Language::Zh => "应用",
            Language::En => "Apps",
        },
        "mode_clipboard" => match lang {
            Language::Pt => "Área de transferência",
            Language::De => "Zwischenablage",
            Language::Es => "Portapapeles",
            Language::Zh => "剪贴板",
            Language::En => "Clipboard",
        },
        "clipboard_placeholder" => match lang {
            Language::Pt => "Buscar no histórico da área de transferência...",
            Language::De => "Zwischenablage-Verlauf durchsuchen...",
            Language::Es => "Buscar en el historial del portapapeles...",
            Language::Zh => "搜索剪贴板历史...",
            Language::En => "Search clipboard history...",
        },
        "clipboard_empty" => match lang {
            Language::Pt => "O histórico da área de transferência está vazio",
            Language::De => "Der Zwischenablage-Verlauf ist leer",
            Language::Es => "El historial del portapapeles está vacío",
            Language::Zh => "剪贴板历史为空",
            Language::En => "Clipboard history is empty",
        },
        "clipboard_hint" => match lang {
            Language::Pt => "Enter: copiar  |  Ctrl+P: fixar  |  Ctrl+Delete: apagar  |  Tab: aplicativos",
            Language::De => "Enter: kopieren  |  Strg+P: anheften  |  Strg+Entf: löschen  |  Tab: Apps",
            Language::Es => "Enter: copiar  |  Ctrl+P: fijar  |  Ctrl+Supr: borrar  |  Tab: aplicaciones",
            Language::Zh => "Enter: 复制  |  Ctrl+P: 固定  |  Ctrl+Delete: 删除  |  Tab: 应用",
            Language::En => "Enter: copy  |  Ctrl+P: pin  |  Ctrl+Delete: delete  |  Tab: apps",
        },
        "apps_hint" => match lang {
            Language::Pt => "Tab: histórico da área de transferência",
            Language::De => "Tab: Zwischenablage-Verlauf",
            Language::Es => "Tab: historial del portapapeles",
            Language::Zh => "Tab: 剪贴板历史",
            Language::En => "Tab: clipboard history",
        },
        "pinned" => match lang {
            Language::Pt => "Fixado",
            Language::De => "Angeheftet",
            Language::Es => "Fijado",
            Language::Zh => "已固定",
            Language::En => "Pinned",
        },
        "age_now" => match lang {
            Language::Pt => "agora",
            Language::De => "jetzt",
            Language::Es => "ahora",
            Language::Zh => "刚刚",
            Language::En => "now",
        },
        "age_min" => match lang {
            Language::Pt => "min",
            Language::De => "Min.",
            Language::Es => "min",
            Language::Zh => "分钟",
            Language::En => "min",
        },
        "age_hour" => match lang {
            Language::Pt => "h",
            Language::De => "Std.",
            Language::Es => "h",
            Language::Zh => "小时",
            Language::En => "h",
        },
        "age_day" => match lang {
            Language::Pt => "d",
            Language::De => "Tg.",
            Language::Es => "d",
            Language::Zh => "天",
            Language::En => "d",
        },
        "copied_title" => match lang {
            Language::Pt => "Copiado",
            Language::De => "Kopiert",
            Language::Es => "Copiado",
            Language::Zh => "已复制",
            Language::En => "Copied",
        },
        "copied_body" => match lang {
            Language::Pt => "Copiado. Pressione Ctrl+V para colar",
            Language::De => "Kopiert. Zum Einfügen Strg+V drücken",
            Language::Es => "Copiado. Pulsa Ctrl+V para pegar",
            Language::Zh => "已复制。按 Ctrl+V 粘贴",
            Language::En => "Copied. Press Ctrl+V to paste",
        },
        "tray_clear_history" => match lang {
            Language::Pt => "Limpar histórico",
            Language::De => "Verlauf löschen",
            Language::Es => "Borrar historial",
            Language::Zh => "清除历史",
            Language::En => "Clear history",
        },
        "tray_start_at_login" => match lang {
            Language::Pt => "Iniciar com o sistema",
            Language::De => "Beim Anmelden starten",
            Language::Es => "Iniciar al iniciar sesión",
            Language::Zh => "登录时启动",
            Language::En => "Start at login",
        },
        "shortcut_clipboard_desc" => match lang {
            Language::Pt => "Abrir o histórico da área de transferência do Menew",
            Language::De => "Menew Zwischenablage-Verlauf öffnen",
            Language::Es => "Abrir el historial del portapapeles de Menew",
            Language::Zh => "打开 Menew 剪贴板历史",
            Language::En => "Open Menew Clipboard History",
        },
        _ => crate::strings::lookup(key, lang).unwrap_or(key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_detection() {
        assert_eq!(Language::from_code("pt_BR.UTF-8"), Language::Pt);
        assert_eq!(Language::from_code("de_DE.UTF-8"), Language::De);
        assert_eq!(Language::from_code("es_ES"), Language::Es);
        assert_eq!(Language::from_code("zh_CN"), Language::Zh);
        assert_eq!(Language::from_code("en_US"), Language::En);
        assert_eq!(Language::from_code("unknown"), Language::En);
    }

    #[test]
    fn every_key_is_translated_in_all_languages() {
        let keys = [
            "mode_apps", "mode_clipboard", "clipboard_placeholder", "clipboard_empty",
            "clipboard_hint", "apps_hint", "pinned", "age_now", "age_min", "age_hour", "age_day",
            "copied_title", "copied_body", "tray_clear_history", "tray_start_at_login",
            "shortcut_clipboard_desc",
        ];
        let langs = [Language::En, Language::Pt, Language::De, Language::Es, Language::Zh];
        for key in keys {
            for lang in langs {
                let v = t_in(key, lang);
                assert_ne!(v, key, "missing {key} for {lang:?}");
                assert!(!v.contains('\u{2014}'), "em-dash in {key}");
            }
            // the translations must actually differ from English somewhere
            assert!(langs.iter().any(|l| t_in(key, *l) != t_in(key, Language::En)), "{key}");
        }
    }

    #[test]
    fn test_translations() {
        set_language(Language::Pt);
        assert_eq!(t("tray_open_palette"), "Abrir Paleta de Comandos");
        set_language(Language::En);
        assert_eq!(t("tray_open_palette"), "Open Command Palette");
    }
}
