//! Palette strings in the 5 languages. The texts follow the Mac app (`Sources/Menew/L10n.swift`)
//! whenever the same string exists there; the Linux-only ones are marked "linux" below.
//!
//! Column order: en, pt, de, es, zh. Placeholders are `{}` (in order) or `{1}`, `{2}` (positional,
//! needed by some Chinese texts), filled by [`fmt`].

use crate::i18n::Language;

fn column(lang: Language) -> usize {
    match lang {
        Language::En => 0,
        Language::Pt => 1,
        Language::De => 2,
        Language::Es => 3,
        Language::Zh => 4,
    }
}

/// Translation of `key`, or `None` if the key is not in the palette table.
pub fn lookup(key: &str, lang: Language) -> Option<&'static str> {
    TABLE.iter().find(|(k, _)| *k == key).map(|(_, v)| v[column(lang)])
}

/// Fill `{}` / `{1}` placeholders.
pub fn fmt(template: &str, args: &[&str]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut next = 0usize;
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '{' {
            out.push(c);
            continue;
        }
        let mut digits = String::new();
        while let Some(&d) = chars.peek() {
            if d.is_ascii_digit() {
                digits.push(d);
                chars.next();
            } else {
                break;
            }
        }
        if chars.peek() == Some(&'}') {
            chars.next();
            let idx = if digits.is_empty() {
                let i = next;
                next += 1;
                i
            } else {
                digits.parse::<usize>().unwrap_or(1).saturating_sub(1)
            };
            if let Some(a) = args.get(idx) {
                out.push_str(a);
            }
        } else {
            out.push('{');
            out.push_str(&digits);
        }
    }
    out
}

/// Translate and fill in one call.
pub fn tr(key: &str, lang: Language, args: &[&str]) -> String {
    match lookup(key, lang) {
        Some(t) => fmt(t, args),
        None => key.to_string(),
    }
}

/// Every key of the table (used by the tests).
pub fn keys() -> impl Iterator<Item = &'static str> {
    TABLE.iter().map(|(k, _)| *k)
}

pub fn row(key: &str) -> Option<&'static [&'static str; 5]> {
    TABLE.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
}

#[rustfmt::skip]
static TABLE: &[(&str, [&str; 5])] = &[
    // ---- kinds, empty states, tips (Palette.swift) ----
    ("kind_calc", ["Calculator", "Calculadora", "Rechner", "Calculadora", "计算器"]),
    ("kind_action", ["Action", "Ação", "Aktion", "Acción", "操作"]),
    ("kind_app", ["App", "App", "App", "App", "应用"]),
    ("kind_clip", ["Clipboard", "Clipboard", "Zwischenablage", "Portapapeles", "剪贴板"]),
    ("kind_tip", ["Tip", "Dica", "Tipp", "Consejo", "提示"]),
    ("nothing_found", ["Nothing found for “{}”", "Nada encontrado para “{}”", "Nichts gefunden für „{}“", "No se encontró nada para «{}»", "未找到 “{}”"]),
    ("recent_suggested", ["Recent and suggested", "Recentes e sugestões", "Zuletzt und Vorschläge", "Recientes y sugerencias", "最近与建议"]),
    ("try_typing", ["Try typing", "Experimente digitar", "Probier mal", "Prueba a escribir", "试试输入"]),
    ("tip_timer", ["timer 10m tea", "timer 10m chá", "timer 10m Tee", "temporizador 10m té", "timer 10m 茶"]),
    // linux: the countdown is in the tray, not in a menu bar
    ("tip_timer_desc", ["Countdown in the tray", "Contagem na bandeja", "Countdown in der Statusleiste", "Cuenta atrás en la bandeja", "托盘倒计时"]),
    ("tip_emoji", ["emoji rocket", "emoji foguete", "emoji rakete", "emoji cohete", "emoji 火箭"]),
    ("tip_emoji_desc", ["Emoji and symbols", "Emoji e símbolos", "Emoji und Symbole", "Emoji y símbolos", "表情与符号"]),
    ("tip_tz", ["3pm in Lisbon", "15h em Lisboa", "15h in Lissabon", "15h en Lisboa", "15h 到 里斯本"]),
    ("tip_tz_desc", ["Time zones", "Fusos horários", "Zeitzonen", "Zonas horarias", "时区"]),
    // linux: the Mac zh text is "340 的 20%", which its own parser does not accept; this one parses
    ("tip_math", ["20% of 340", "20% de 340", "20% von 340", "20% de 340", "20% 的 340"]),
    ("tip_math_desc", ["Math and conversions", "Contas e conversões", "Rechnen und Umrechnen", "Cálculos y conversiones", "计算与换算"]),
    ("copied_value", ["Copied: {}", "Copiado: {}", "Kopiert: {}", "Copiado: {}", "已复制：{}"]),
    // ---- calculator, currency, time zones, timestamps ----
    ("ecb_rate", ["ECB rate", "cotação do BCE", "EZB-Kurs", "cambio del BCE", "欧洲央行汇率"]),
    ("convert_title", ["Convert {} to {}", "Converter {} em {}", "{} in {} umrechnen", "Convertir {} a {}", "将 {} 换算为 {}"]),
    ("fx_fetch_sub", ["↩ fetches today's rate (frankfurter.app)", "↩ busca a cotação do dia (frankfurter.app)", "↩ lädt den Tageskurs (frankfurter.app)", "↩ busca el cambio del día (frankfurter.app)", "↩ 获取今日汇率（frankfurter.app）"]),
    ("fx_fail", ["Couldn't fetch the rate", "Não deu para buscar a cotação", "Kurs konnte nicht geladen werden", "No se pudo obtener el cambio", "无法获取汇率"]),
    ("here", ["{} here", "{} aqui", "{} hier", "{} aquí", "此处 {}"]),
    ("time_in", ["{} in {}", "{} em {}", "{} in {}", "{} en {}", "{2} {1}"]),
    ("tomorrow", ["tomorrow", "amanhã", "morgen", "mañana", "明天"]),
    ("yesterday", ["yesterday", "ontem", "gestern", "ayer", "昨天"]),
    ("same_time", ["same time", "mesma hora", "gleiche Zeit", "misma hora", "时间相同"]),
    ("timestamp_sub", ["Unix timestamp · ↩ copies {}", "Timestamp Unix · ↩ copia {}", "Unix-Zeitstempel · ↩ kopiert {}", "Marca de tiempo Unix · ↩ copia {}", "Unix 时间戳 · ↩ 复制 {}"]),
    ("city_new_york", ["New York", "Nova York", "New York", "Nueva York", "纽约"]),
    ("city_tokyo", ["Tokyo", "Tóquio", "Tokio", "Tokio", "东京"]),
    ("city_london", ["London", "Londres", "London", "Londres", "伦敦"]),
    ("city_lisbon", ["Lisbon", "Lisboa", "Lissabon", "Lisboa", "里斯本"]),
    ("city_shanghai", ["Shanghai", "Xangai", "Shanghai", "Shanghái", "上海"]),
    ("city_berlin", ["Berlin", "Berlim", "Berlin", "Berlín", "柏林"]),
    ("city_mexico_city", ["Mexico City", "Cidade do México", "Mexiko-Stadt", "Ciudad de México", "墨西哥城"]),
    ("city_india", ["India", "Índia", "Indien", "India", "印度"]),
    // ---- text transforms (TextTransforms.swift) ----
    ("tt_upper", ["UPPERCASE", "MAIÚSCULAS", "GROSSBUCHSTABEN", "MAYÚSCULAS", "全部大写"]),
    ("tt_lower", ["lowercase", "minúsculas", "kleinbuchstaben", "minúsculas", "全部小写"]),
    ("tt_title", ["Title Case", "Primeira Letra Maiúscula", "Erster Buchstabe Groß", "Primera Letra Mayúscula", "首字母大写"]),
    ("tt_sentence", ["Sentence case", "Só a primeira maiúscula", "Satzanfang groß", "Tipo oración", "句首字母大写"]),
    ("tt_no_accents", ["Remove accents", "Remover acentos", "Akzente entfernen", "Quitar acentos", "去除重音符号"]),
    ("tt_trim", ["Remove extra spaces", "Remover espaços extras", "Überflüssige Leerzeichen entfernen", "Quitar espacios extra", "移除多余空格"]),
    ("tt_sort_lines", ["Sort lines", "Ordenar linhas", "Zeilen sortieren", "Ordenar líneas", "按行排序"]),
    ("tt_reverse_lines", ["Reverse lines", "Inverter linhas", "Zeilen umkehren", "Invertir líneas", "反转行顺序"]),
    ("tt_unique_lines", ["Remove duplicate lines", "Remover linhas repetidas", "Doppelte Zeilen entfernen", "Quitar líneas duplicadas", "删除重复行"]),
    ("tt_json_pretty", ["Format JSON", "Formatar JSON", "JSON formatieren", "Formatear JSON", "格式化 JSON"]),
    ("tt_json_minify", ["Minify JSON", "Compactar JSON", "JSON komprimieren", "Compactar JSON", "压缩 JSON"]),
    ("tt_base64_encode", ["Encode Base64", "Codificar em Base64", "Base64 kodieren", "Codificar en Base64", "Base64 编码"]),
    ("tt_base64_decode", ["Decode Base64", "Decodificar Base64", "Base64 dekodieren", "Decodificar Base64", "Base64 解码"]),
    ("tt_url_encode", ["URL-encode", "Codificar para URL", "URL-kodieren", "Codificar para URL", "URL 编码"]),
    ("tt_url_decode", ["URL-decode", "Decodificar URL", "URL-dekodieren", "Decodificar URL", "URL 解码"]),
    ("tt_count", ["Count characters and words", "Contar caracteres e palavras", "Zeichen und Wörter zählen", "Contar caracteres y palabras", "统计字符和字数"]),
    ("tt_camel", ["camelCase", "camelCase", "camelCase", "camelCase", "camelCase（驼峰）"]),
    ("tt_snake", ["snake_case", "snake_case", "snake_case", "snake_case", "snake_case（下划线）"]),
    ("tt_kebab", ["kebab-case", "kebab-case", "kebab-case", "kebab-case", "kebab-case（连字符）"]),
    ("tt_jwt", ["Decode JWT", "Decodificar JWT", "JWT dekodieren", "Decodificar JWT", "解码 JWT"]),
    ("tt_fail_json", ["Invalid JSON", "JSON inválido", "Ungültiges JSON", "JSON no válido", "无效的 JSON"]),
    ("tt_fail_base64", ["Not valid Base64", "Não é Base64 válido", "Kein gültiges Base64", "No es Base64 válido", "不是有效的 Base64"]),
    ("tt_fail_jwt", ["Not a valid JWT", "Não é um JWT válido", "Kein gültiges JWT", "No es un JWT válido", "不是有效的 JWT"]),
    ("tt_fail", ["Couldn't transform this text", "Não deu para transformar este texto", "Dieser Text konnte nicht umgewandelt werden", "No se pudo transformar este texto", "无法转换此文本"]),
    ("tt_done", ["Text transformed", "Texto transformado", "Text umgewandelt", "Texto transformado", "文本已转换"]),
    ("tt_too_large", ["Text too large to preview", "Texto grande demais para a prévia", "Text zu groß für eine Vorschau", "Texto demasiado grande para la vista previa", "文本过大，无法预览"]),
    ("tt_stats_one", ["{} characters · {} words · {} line", "{} caracteres · {} palavras · {} linha", "{} Zeichen · {} Wörter · {} Zeile", "{} caracteres · {} palabras · {} línea", "{} 个字符 · {} 个词 · {} 行"]),
    ("tt_stats_many", ["{} characters · {} words · {} lines", "{} caracteres · {} palavras · {} linhas", "{} Zeichen · {} Wörter · {} Zeilen", "{} caracteres · {} palabras · {} líneas", "{} 个字符 · {} 个词 · {} 行"]),
    // ---- quick links (QuickLinks.swift) ----
    ("link_type", ["{}: type what to search", "{}: digite o que buscar", "{}: Suchbegriff eingeben", "{}: escribe qué buscar", "{}：输入要搜索的内容"]),
    ("link_pair", ["{}: {}", "{}: {}", "{}: {}", "{}: {}", "{}：{}"]),
    ("link_open", ["Open {}", "Abrir {}", "{} öffnen", "Abrir {}", "打开 {}"]),
    ("link_search", ["Search “{}” on {}", "Buscar “{}” no {}", "„{}“ auf {} suchen", "Buscar “{}” en {}", "在 {2} 上搜索“{1}”"]),
    ("paste_hint", ["↩ pastes into the front app", "↩ cola no app da frente", "↩ fügt in die vordere App ein", "↩ pega en la app de delante", "↩ 粘贴到前台应用"]),
    // ---- timers and alarms (Timers.swift) ----
    ("alarm_at", ["Alarm at {}", "Alarme às {}", "Wecker um {}", "Alarma a las {}", "{} 的闹钟"]),
    ("alarm_at_label", ["Alarm at {}: {}", "Alarme às {}: {}", "Wecker um {}: {}", "Alarma a las {}: {}", "{} 的闹钟：{}"]),
    ("timer_until", ["Timer until {}", "Timer até {}", "Timer bis {}", "Temporizador hasta las {}", "计时至 {}"]),
    ("alarm", ["Alarm", "Alarme", "Wecker", "Alarma", "闹钟"]),
    ("timer_done", ["Timer done", "Timer terminou", "Timer abgelaufen", "Temporizador terminado", "计时结束"]),
    ("time_up", ["Time's up.", "O tempo acabou.", "Die Zeit ist um.", "Se acabó el tiempo.", "时间到了。"]),
    ("timer_cancelled", ["Timer cancelled", "Timer cancelado", "Timer abgebrochen", "Temporizador cancelado", "计时已取消"]),
    ("timer_span", ["{} timer", "Timer de {}", "Timer für {}", "Temporizador de {}", "{} 计时"]),
    ("timer_span_label", ["{} timer: {}", "Timer de {}: {}", "Timer für {}: {}", "Temporizador de {}: {}", "{} 计时：{}"]),
    ("ends_at", ["Ends at {}", "Termina às {}", "Endet um {}", "Termina a las {}", "{} 结束"]),
    ("alarm_sub", ["Plays a sound and notifies you on time", "Toca um som e avisa na hora", "Spielt einen Ton und benachrichtigt pünktlich", "Suena y avisa a la hora", "准时响铃并通知"]),
    ("cancel_timer", ["Cancel the {2} {1}", "Cancelar {} das {}", "{1} um {2} abbrechen", "Cancelar {} de las {}", "取消 {2} 的{1}"]),
    ("word_alarm", ["alarm", "alarme", "Wecker", "alarma", "闹钟"]),
    ("word_timer", ["timer", "timer", "Timer", "temporizador", "计时"]),
    ("seg_alarm", ["ALARM", "ALARME", "WECKER", "ALARMA", "闹钟"]),
    ("seg_timer", ["TIMER", "TIMER", "TIMER", "TIMER", "计时"]),
    // ---- focus (Focus.swift) ----
    ("seg_focus", ["FOCUS", "FOCO", "FOKUS", "ENFOQUE", "专注"]),
    ("seg_break", ["BREAK", "PAUSA", "PAUSE", "DESCANSO", "休息"]),
    ("focus_min", ["Focus for {} min", "Foco por {} min", "Fokus für {} Min.", "Enfoque durante {} min", "专注 {} 分钟"]),
    ("focus_stopped", ["Focus stopped", "Foco encerrado", "Fokus beendet", "Enfoque detenido", "专注已停止"]),
    ("focus_done", ["Focus done", "Foco concluído", "Fokus abgeschlossen", "Enfoque completado", "专注完成"]),
    ("break_5", ["Time for a 5-minute break.", "Hora de uma pausa de 5 minutos.", "Zeit für 5 Minuten Pause.", "Hora de un descanso de 5 minutos.", "休息 5 分钟吧。"]),
    ("nice_work", ["Nice work!", "Bom trabalho!", "Gut gemacht!", "¡Buen trabajo!", "干得好！"]),
    ("break_over", ["Break over", "Pausa encerrada", "Pause vorbei", "Descanso terminado", "休息结束"]),
    ("ready_again", ["Ready for another focus?", "Pronto para outro foco?", "Bereit für die nächste Fokusrunde?", "¿Listo para otro enfoque?", "准备好再专注一次了吗？"]),
    ("stop_focus", ["Stop focus", "Parar foco", "Fokus beenden", "Detener enfoque", "停止专注"]),
    ("start_focus", ["Start focus", "Iniciar foco", "Fokus starten", "Iniciar enfoque", "开始专注"]),
    ("focus_hint", ["Type a time, like “focus 50”", "Digite um tempo, como “foco 50”", "Gib eine Zeit ein, z. B. „fokus 50“", "Escribe un tiempo, como “focus 50”", "输入时长，例如“专注 50”"]),
    ("focus_extend", ["+5 min", "+5 min", "+5 Min.", "+5 min", "+5 分钟"]),
    // ---- script commands (ScriptCommands.swift) ----
    ("script_arg_sub", ["Script · type after the name: {}", "Script · digite depois do nome: {}", "Skript · nach dem Namen tippen: {}", "Script · escribe después del nombre: {}", "脚本 · 在名称后输入：{}"]),
    ("script_pkg_sub", ["Script · {}", "Script · {}", "Skript · {}", "Script · {}", "脚本 · {}"]),
    ("script", ["Script", "Script", "Skript", "Script", "脚本"]),
    ("open_scripts", ["Open scripts folder", "Abrir pasta de scripts", "Skriptordner öffnen", "Abrir carpeta de scripts", "打开脚本文件夹"]),
    ("open_scripts_sub", ["Each file becomes a palette command", "Cada arquivo vira um comando da paleta", "Jede Datei wird zu einem Befehl", "Cada archivo se vuelve un comando", "每个文件都会成为一个命令"]),
    ("run_with", ["Run with “{}”", "Rodar com “{}”", "Mit „{}“ ausführen", "Ejecutar con «{}»", "使用“{}”运行"]),
    ("script_need_arg", ["Type the text after the script’s name", "Digite o texto depois do nome do script", "Tippe den Text nach dem Skriptnamen", "Escribe el texto después del nombre del script", "请在脚本名称后输入文本"]),
    ("script_running", ["Running {}…", "Rodando {}…", "{} läuft …", "Ejecutando {}…", "正在运行 {}…"]),
    ("script_done", ["{} done", "{} concluído", "{} fertig", "{} terminado", "{} 已完成"]),
    ("script_code", ["code {}", "código {}", "Code {}", "código {}", "代码 {}"]),
    ("script_text", ["text", "texto", "Text", "texto", "文本"]),
    ("ok", ["OK", "OK", "OK", "OK", "好"]),
    ("copy", ["Copy", "Copiar", "Kopieren", "Copiar", "复制"]),
    // ---- Menew actions on Linux ----
    ("act_clipboard", ["Clipboard history", "Histórico do clipboard", "Ablageverlauf", "Historial del portapapeles", "剪贴板历史"]),
    ("act_clear", ["Clear clipboard history", "Limpar histórico do clipboard", "Ablageverlauf löschen", "Borrar historial del portapapeles", "清除剪贴板历史"]),
    ("act_clear_sub", ["Deletes every unpinned entry", "Apaga tudo o que não está fixado", "Löscht alle nicht angehefteten Einträge", "Borra todo lo que no está fijado", "删除所有未固定的条目"]),
    ("act_autostart_on", ["Start Menew at login", "Iniciar o Menew com o sistema", "Menew beim Anmelden starten", "Iniciar Menew al iniciar sesión", "登录时启动 Menew"]),
    ("act_autostart_off", ["Don't start Menew at login", "Não iniciar o Menew com o sistema", "Menew nicht beim Anmelden starten", "No iniciar Menew al iniciar sesión", "登录时不启动 Menew"]),
    ("act_quit", ["Quit Menew", "Sair do Menew", "Menew beenden", "Salir de Menew", "退出 Menew"]),
    ("act_uuid", ["Generate UUID", "Gerar UUID", "UUID erzeugen", "Generar UUID", "生成 UUID"]),
    ("act_timestamp", ["Copy Unix timestamp", "Copiar timestamp Unix", "Unix-Zeitstempel kopieren", "Copiar marca de tiempo Unix", "复制 Unix 时间戳"]),
    ("act_sub_clipboard", ["Search and paste what you copied", "Busque e cole o que você copiou", "Durchsuche und füge ein, was du kopiert hast", "Busca y pega lo que copiaste", "搜索并粘贴你复制过的内容"]),
    // ---- results of side effects ----
    ("toast_timer_set", ["Timer until {}", "Timer até {}", "Timer bis {}", "Temporizador hasta las {}", "计时至 {}"]),
    ("toast_alarm_set", ["Alarm at {}", "Alarme às {}", "Wecker um {}", "Alarma a las {}", "{} 的闹钟"]),
    ("toast_focus_started", ["Focus for {} min", "Foco por {} min", "Fokus für {} Min.", "Enfoque durante {} min", "专注 {} 分钟"]),
    ("tray_focus_extend", ["+5 min of focus", "+5 min de foco", "+5 Min. Fokus", "+5 min de enfoque", "专注 +5 分钟"]),
];

#[cfg(test)]
mod tests {
    use super::*;

    const LANGS: [Language; 5] = [Language::En, Language::Pt, Language::De, Language::Es, Language::Zh];

    fn placeholders(s: &str) -> usize {
        let mut n = 0;
        let mut distinct = std::collections::BTreeSet::new();
        let mut next = 1;
        let b: Vec<char> = s.chars().collect();
        let mut i = 0;
        while i < b.len() {
            if b[i] == '{' {
                let mut j = i + 1;
                let mut d = String::new();
                while j < b.len() && b[j].is_ascii_digit() {
                    d.push(b[j]);
                    j += 1;
                }
                if j < b.len() && b[j] == '}' {
                    n += 1;
                    distinct.insert(if d.is_empty() {
                        next += 1;
                        next - 1
                    } else {
                        d.parse::<usize>().unwrap()
                    });
                    i = j;
                }
            }
            i += 1;
        }
        let _ = n;
        distinct.len()
    }

    #[test]
    fn every_key_is_filled_in_all_5_languages() {
        for (key, row) in TABLE {
            for (i, v) in row.iter().enumerate() {
                assert!(!v.trim().is_empty(), "{key}: empty text for {:?}", LANGS[i]);
                assert!(!v.contains('\u{2014}'), "{key}: em-dash in {:?}", LANGS[i]);
            }
            // the placeholders must be the same in every language
            let n = placeholders(row[0]);
            for (i, v) in row.iter().enumerate() {
                assert_eq!(placeholders(v), n, "{key}: placeholders differ in {:?}", LANGS[i]);
            }
        }
    }

    #[test]
    fn keys_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for (k, _) in TABLE {
            assert!(seen.insert(*k), "duplicate key {k}");
        }
    }

    #[test]
    fn fmt_handles_sequential_and_positional_placeholders() {
        assert_eq!(fmt("{} in {}", &["a", "b"]), "a in b");
        assert_eq!(fmt("{2} {1}", &["a", "b"]), "b a");
        assert_eq!(fmt("no placeholders", &[]), "no placeholders");
        assert_eq!(fmt("{}", &[]), "");
        assert_eq!(tr("time_in", Language::Zh, &["18:00", "里斯本"]), "里斯本 18:00");
        assert_eq!(tr("no_such_key", Language::En, &[]), "no_such_key");
    }

    /// Every tr call with a literal key must name a key of the table, so no UI string falls back to
    /// showing its key.
    #[test]
    fn every_key_used_in_the_code_exists() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut missing = vec![];
        let mut found = 0;
        for e in walkdir::WalkDir::new(&root).into_iter().filter_map(Result::ok) {
            let p = e.path();
            if p.extension().map(|x| x == "rs").unwrap_or(false) && !p.to_string_lossy().contains("emoji_data") {
                let text = std::fs::read_to_string(p).unwrap_or_default();
                let mut rest = text.as_str();
                while let Some(i) = rest.find("tr(\"") {
                    let after = &rest[i + 4..];
                    if let Some(end) = after.find('"') {
                        let key = &after[..end];
                        // `tr(` also ends other identifiers ("str(", "attr(") and appears in tests
                        let before = rest[..i].chars().last().unwrap_or(' ');
                        if !(before.is_alphanumeric() || before == '_') && key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
                            found += 1;
                            if lookup(key, Language::En).is_none() && key != "no_such_key" {
                                missing.push(format!("{} -> {key}", p.display()));
                            }
                        }
                    }
                    rest = &rest[i + 4..];
                }
            }
        }
        assert!(found > 40, "found only {found} tr() calls, the scan is broken");
        assert!(missing.is_empty(), "keys missing from the table: {missing:?}");
    }
}
