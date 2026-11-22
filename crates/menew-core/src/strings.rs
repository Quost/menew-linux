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
    all().find(|(k, _)| *k == key).map(|(_, v)| v[column(lang)])
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
    all().map(|(k, _)| *k)
}

pub fn row(key: &str) -> Option<&'static [&'static str; 5]> {
    all().find(|(k, _)| *k == key).map(|(_, v)| v)
}

/// The palette and clipboard table, then the release 1 table (`strings_r1.rs`).
fn all() -> impl Iterator<Item = &'static (&'static str, [&'static str; 5])> {
    TABLE.iter().chain(crate::strings_r1::TABLE.iter())
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
    // ---- updates (Updater.swift; the strings of the Mac where it has them, "linux" otherwise) ----
    ("upd_check", ["Check for Updates…", "Procurar atualizações…", "Nach Updates suchen…", "Buscar actualizaciones…", "检查更新…"]),
    ("upd_auto", ["Check for updates automatically", "Procurar atualizações automaticamente", "Automatisch nach Updates suchen", "Buscar actualizaciones automáticamente", "自动检查更新"]),
    ("upd_install", ["Install and Relaunch", "Instalar e reabrir", "Installieren und neu starten", "Instalar y reabrir", "安装并重新打开"]),
    ("upd_later", ["Later", "Depois", "Später", "Más tarde", "稍后"]),
    // linux: Sparkle has its own window with these; here they are ours
    ("upd_available_tray", ["Update available: {}", "Atualização disponível: {}", "Update verfügbar: {}", "Actualización disponible: {}", "有可用更新：{}"]),
    ("upd_title", ["Menew {} is available", "O Menew {} está disponível", "Menew {} ist verfügbar", "Menew {} está disponible", "Menew {} 现已推出"]),
    ("upd_body", ["You have version {}. What's new:", "Você tem a versão {}. O que há de novo:", "Du hast Version {}. Neu in dieser Version:", "Tienes la versión {}. Novedades:", "你当前的版本是 {}。更新内容："]),
    ("upd_skip", ["Skip This Version", "Pular esta versão", "Diese Version überspringen", "Omitir esta versión", "跳过此版本"]),
    ("upd_uptodate", ["Menew is up to date ({})", "O Menew está atualizado ({})", "Menew ist auf dem neuesten Stand ({})", "Menew está al día ({})", "Menew 已是最新版本（{}）"]),
    ("upd_check_failed", ["Could not check for updates", "Não foi possível procurar atualizações", "Die Suche nach Updates ist fehlgeschlagen", "No se pudo buscar actualizaciones", "无法检查更新"]),
    ("upd_downloading", ["Downloading Menew {}…", "Baixando o Menew {}…", "Menew {} wird geladen …", "Descargando Menew {}…", "正在下载 Menew {}…"]),
    ("upd_err_verify", ["The update could not be verified (signature or checksum) and was not installed", "A atualização não pôde ser verificada (assinatura ou checksum) e não foi instalada", "Das Update konnte nicht überprüft werden (Signatur oder Prüfsumme) und wurde nicht installiert", "No se pudo verificar la actualización (firma o checksum) y no se instaló", "无法验证更新（签名或校验和），未进行安装"]),
    ("upd_err_download", ["The update could not be downloaded", "Não foi possível baixar a atualização", "Das Update konnte nicht geladen werden", "No se pudo descargar la actualización", "无法下载更新"]),
    ("upd_err_dir", ["Menew cannot replace itself in this folder. The download page was opened", "O Menew não consegue se substituir nesta pasta. A página de download foi aberta", "Menew kann sich in diesem Ordner nicht ersetzen. Die Download-Seite wurde geöffnet", "Menew no puede reemplazarse en esta carpeta. Se abrió la página de descarga", "Menew 无法在此文件夹中替换自身。已打开下载页面"]),
    ("upd_err_install", ["The update was not installed. The download page was opened", "A atualização não foi instalada. A página de download foi aberta", "Das Update wurde nicht installiert. Die Download-Seite wurde geöffnet", "La actualización no se instaló. Se abrió la página de descarga", "更新未安装。已打开下载页面"]),
    ("upd_manual", ["Menew {} is available. This copy was not installed from a package or an AppImage: update it yourself", "O Menew {} está disponível. Esta cópia não foi instalada por pacote ou AppImage: atualize você mesmo", "Menew {} ist verfügbar. Diese Kopie wurde nicht per Paket oder AppImage installiert: bitte selbst aktualisieren", "Menew {} está disponible. Esta copia no se instaló desde un paquete ni AppImage: actualiza tú mismo", "Menew {} 现已推出。此副本不是通过软件包或 AppImage 安装的：请自行更新"]),
    ("upd_disabled", ["Update checks are off in this mode", "A verificação de atualizações está desligada neste modo", "Die Update-Suche ist in diesem Modus aus", "La búsqueda de actualizaciones está desactivada en este modo", "此模式下已关闭更新检查"]),
    ("act_uuid", ["Generate UUID", "Gerar UUID", "UUID erzeugen", "Generar UUID", "生成 UUID"]),
    ("act_timestamp", ["Copy Unix timestamp", "Copiar timestamp Unix", "Unix-Zeitstempel kopieren", "Copiar marca de tiempo Unix", "复制 Unix 时间戳"]),
    ("act_sub_clipboard", ["Search and paste what you copied", "Busque e cole o que você copiou", "Durchsuche und füge ein, was du kopiert hast", "Busca y pega lo que copiaste", "搜索并粘贴你复制过的内容"]),
    ("act_monitor", ["System monitor", "Monitor do sistema", "Systemmonitor", "Monitor del sistema", "系统监控"]),
    ("act_monitor_sub", ["CPU, memory, disk, battery and the busiest processes", "CPU, memória, disco, bateria e os processos que mais gastam", "CPU, Speicher, Festplatte, Batterie und die aktivsten Prozesse", "CPU, memoria, disco, batería y los procesos que más consumen", "CPU、内存、磁盘、电池和最耗资源的进程"]),
    ("tray_monitor", ["System Monitor", "Monitor do Sistema", "Systemmonitor", "Monitor del Sistema", "系统监控"]),
    ("mon_cpu", ["CPU", "CPU", "CPU", "CPU", "CPU"]),
    ("mon_processor", ["Processor", "Processador", "Prozessor", "Procesador", "处理器"]),
    ("mon_cpu_graph", ["CPU graph", "Gráfico da CPU", "CPU-Verlauf", "Gráfico de CPU", "CPU 图表"]),
    ("mon_in_use", ["in use", "em uso", "belegt", "en uso", "使用中"]),
    ("mon_last_2min", ["last 2 min", "últimos 2 min", "letzte 2 Min.", "últimos 2 min", "最近 2 分钟"]),
    ("mon_cores", ["Cores", "Núcleos", "Kerne", "Núcleos", "核心"]),
    ("mon_core_n", ["Core {1}: {2}", "Núcleo {1}: {2}", "Kern {1}: {2}", "Núcleo {1}: {2}", "核心 {1}: {2}"]),
    ("mon_n_cores", ["{} cores", "{} núcleos", "{} Kerne", "{} núcleos", "{} 核"]),
    ("mon_memory", ["Memory", "Memória", "Speicher", "Memoria", "内存"]),
    ("mon_apps", ["Apps", "Apps", "Apps", "Apps", "应用"]),
    ("mon_system", ["System", "Sistema", "System", "Sistema", "系统"]),
    ("mon_compressed", ["Compressed", "Comprimida", "Komprimiert", "Comprimida", "已压缩"]),
    ("mon_swap", ["Swap", "Swap", "Swap", "Swap", "Swap"]),
    ("mon_pressure_normal", ["Normal pressure", "Pressão normal", "Normaler Druck", "Presión normal", "压力正常"]),
    ("mon_pressure_warning", ["High pressure", "Pressão alta", "Hoher Druck", "Presión alta", "压力较高"]),
    ("mon_pressure_critical", ["Critical pressure", "Pressão crítica", "Kritischer Druck", "Presión crítica", "压力严重"]),
    ("mon_disk", ["Disk", "Disco", "Festplatte", "Disco", "磁盘"]),
    ("mon_free", ["{} free", "{} livres", "{} frei", "{} libres", "可用 {}"]),
    ("mon_network", ["Network", "Rede", "Netzwerk", "Red", "网络"]),
    ("mon_download", ["Download", "Download", "Download", "Descarga", "下载"]),
    ("mon_upload", ["Upload", "Upload", "Upload", "Subida", "上传"]),
    ("mon_battery", ["Battery", "Bateria", "Batterie", "Batería", "电池"]),
    ("mon_charging", ["Charging", "Carregando", "Lädt", "Cargando", "正在充电"]),
    ("mon_full_in", ["Full in {}", "Cheia em {}", "Voll in {}", "Llena en {}", "{} 后充满"]),
    ("mon_plugged", ["Plugged in", "Na tomada", "Angeschlossen", "Conectada", "已接通电源"]),
    ("mon_on_battery", ["On battery", "Na bateria", "Im Akkubetrieb", "Con batería", "使用电池"]),
    ("mon_remaining", ["Remaining", "Restante", "Verbleibend", "Restante", "剩余"]),
    ("mon_until_full", ["Until full", "Para encher", "Bis voll", "Hasta llenar", "充满还需"]),
    ("mon_top_cpu", ["Top CPU usage", "Quem está usando a CPU", "Größte CPU-Nutzer", "Quién usa la CPU", "CPU 占用最高"]),
    ("mon_top_mem", ["Top memory usage", "Quem está usando a memória", "Größte Speichernutzer", "Quién usa la memoria", "内存占用最高"]),
    ("mon_loading", ["Loading…", "Carregando…", "Wird geladen…", "Cargando…", "加载中…"]),
    ("mon_quit", ["Quit", "Encerrar", "Beenden", "Salir", "退出"]),
    ("mon_quit_name", ["Quit {}", "Encerrar {}", "{} beenden", "Salir de {}", "退出 {}"]),
    ("mon_force_quit", ["Force Quit", "Forçar encerramento", "Sofort beenden", "Forzar salida", "强制退出"]),
    ("mon_force_quit_name", ["Force quit {}", "Forçar encerramento de {}", "{} sofort beenden", "Forzar salida de {}", "强制退出 {}"]),
    ("mon_not_available", ["Not available", "Não disponível", "Nicht verfügbar", "No disponible", "不可用"]),
    // ---- results of side effects ----
    ("toast_timer_set", ["Timer until {}", "Timer até {}", "Timer bis {}", "Temporizador hasta las {}", "计时至 {}"]),
    ("toast_alarm_set", ["Alarm at {}", "Alarme às {}", "Wecker um {}", "Alarma a las {}", "{} 的闹钟"]),
    ("toast_focus_started", ["Focus for {} min", "Foco por {} min", "Fokus für {} Min.", "Enfoque durante {} min", "专注 {} 分钟"]),
    ("tray_focus_extend", ["+5 min of focus", "+5 min de foco", "+5 Min. Fokus", "+5 min de enfoque", "专注 +5 分钟"]),
    // ---- clipboard: smart actions (SmartClip.swift) ----
    ("smart_meeting", ["Join meeting", "Entrar na reunião", "Meeting beitreten", "Unirse a la reunión", "加入会议"]),
    ("smart_copy_as", ["Copy as {}", "Copiar como {}", "Als {} kopieren", "Copiar como {}", "复制为 {}"]),
    ("smart_color_copied", ["Copied color: {}", "Cor copiada: {}", "Kopierte Farbe: {}", "Color copiado: {}", "已复制的颜色：{}"]),
    ("smart_track", ["Track package {}", "Rastrear encomenda {}", "Sendung {} verfolgen", "Rastrear envío {}", "追踪包裹 {}"]),
    ("smart_call", ["Call {}", "Ligar para {}", "{} anrufen", "Llamar a {}", "拨打 {}"]),
    // linux: the Mac says "With FaceTime or your iPhone"
    ("smart_call_sub", ["With your phone app", "No app de telefone", "Mit deiner Telefon-App", "Con tu app de teléfono", "使用你的电话应用"]),
    ("smart_whatsapp", ["Open chat in WhatsApp", "Abrir conversa no WhatsApp", "Chat in WhatsApp öffnen", "Abrir chat en WhatsApp", "在 WhatsApp 中打开聊天"]),
    ("smart_maps", ["Open in Maps", "Abrir no Mapas", "In Karten öffnen", "Abrir en Mapas", "在地图中打开"]),
    ("smart_mail", ["Write to {}", "Escrever para {}", "An {} schreiben", "Escribir a {}", "写信给 {}"]),
    ("smart_mail_sub", ["In your email app", "No seu app de e-mail", "In deiner Mail-App", "En tu app de correo", "在你的邮件应用中"]),
    ("smart_open", ["Open {}", "Abrir {}", "{} öffnen", "Abrir {}", "打开 {}"]),
    // ---- clipboard: list, filters, details (ClipboardViews.swift) ----
    ("clip_f_all", ["All", "Tudo", "Alle", "Todo", "全部"]),
    ("clip_f_text", ["Text", "Texto", "Text", "Texto", "文本"]),
    ("clip_f_link", ["Links", "Links", "Links", "Enlaces", "链接"]),
    ("clip_f_image", ["Images", "Imagens", "Bilder", "Imágenes", "图片"]),
    ("clip_f_file", ["Files", "Arquivos", "Dateien", "Archivos", "文件"]),
    ("clip_f_pinned", ["Pinned", "Fixados", "Angeheftet", "Fijados", "置顶"]),
    ("clip_f_snippets", ["Snippets", "Snippets", "Textbausteine", "Fragmentos", "文本片段"]),
    ("clip_image", ["Image", "Imagem", "Bild", "Imagen", "图片"]),
    ("clip_files_one", ["1 file", "1 arquivo", "1 Datei", "1 archivo", "1 个文件"]),
    ("clip_files_n", ["{} files", "{} arquivos", "{} Dateien", "{} archivos", "{} 个文件"]),
    ("clip_and_more", ["{} and {} more", "{} e mais {}", "{} und {} weitere", "{} y {} más", "{} 等 {} 个"]),
    ("clip_stats_one", ["{} characters · {} words · {} line", "{} caracteres · {} palavras · {} linha", "{} Zeichen · {} Wörter · {} Zeile", "{} caracteres · {} palabras · {} línea", "{} 个字符 · {} 个词 · {} 行"]),
    ("clip_stats_n", ["{} characters · {} words · {} lines", "{} caracteres · {} palavras · {} linhas", "{} Zeichen · {} Wörter · {} Zeilen", "{} caracteres · {} palabras · {} líneas", "{} 个字符 · {} 个词 · {} 行"]),
    ("clip_items_count", ["{} items", "{} itens", "{} Einträge", "{} elementos", "{} 条"]),
    ("clip_image_missing", ["This image file could not be found.", "O arquivo desta imagem não foi encontrado.", "Die Bilddatei wurde nicht gefunden.", "No se encontró el archivo de esta imagen.", "找不到此图片文件。"]),
    ("clip_source", ["Source", "Origem", "Quelle", "Origen", "来源"]),
    ("clip_copied_at", ["Copied", "Copiado", "Kopiert", "Copiado", "复制时间"]),
    ("clip_content", ["Content", "Conteúdo", "Inhalt", "Contenido", "内容"]),
    ("clip_unknown", ["Unknown", "Desconhecido", "Unbekannt", "Desconocido", "未知"]),
    ("clip_paste", ["Paste", "Colar", "Einsetzen", "Pegar", "粘贴"]),
    ("clip_paste_plain", ["Paste as plain text", "Colar como texto puro", "Als reinen Text einsetzen", "Pegar como texto sin formato", "粘贴为纯文本"]),
    ("clip_copy_only", ["Copy only", "Só copiar", "Nur kopieren", "Solo copiar", "仅复制"]),
    ("clip_queue_add", ["Add to paste queue", "Adicionar à fila de colagem", "Zur Einfüge-Warteschlange hinzufügen", "Añadir a la cola de pegado", "添加到粘贴队列"]),
    ("clip_pin", ["Pin", "Fixar", "Anheften", "Fijar", "置顶"]),
    ("clip_unpin", ["Unpin", "Desafixar", "Lösen", "Desfijar", "取消置顶"]),
    ("clip_delete", ["Delete", "Apagar", "Löschen", "Borrar", "删除"]),
    ("clip_edit", ["Edit", "Editar", "Bearbeiten", "Editar", "编辑"]),
    ("clip_open_link", ["Open link", "Abrir link", "Link öffnen", "Abrir enlace", "打开链接"]),
    // linux: the Mac says "Show in Finder"
    ("clip_show_folder", ["Show in file manager", "Mostrar no gerenciador de arquivos", "Im Dateimanager zeigen", "Mostrar en el gestor de archivos", "在文件管理器中显示"]),
    ("clip_open_image", ["Open image", "Abrir imagem", "Bild öffnen", "Abrir imagen", "打开图片"]),
    ("clip_copy_image_text", ["Copy text from image", "Copiar texto da imagem", "Text aus Bild kopieren", "Copiar texto de la imagen", "拷贝图片中的文字"]),
    ("clip_actions", ["Actions", "Ações", "Aktionen", "Acciones", "操作"]),
    ("kind_snippet", ["Snippet", "Snippet", "Baustein", "Fragmento", "文本片段"]),
    // ---- clipboard: pause ----
    ("clip_pause_15", ["Pause for 15 minutes", "Pausar por 15 minutos", "15 Minuten pausieren", "Pausar 15 minutos", "暂停 15 分钟"]),
    ("clip_pause_60", ["Pause for 1 hour", "Pausar por 1 hora", "1 Stunde pausieren", "Pausar 1 hora", "暂停 1 小时"]),
    ("clip_pause_until", ["Pause until I resume", "Pausar até eu retomar", "Pausieren, bis ich fortsetze", "Pausar hasta que la reanude", "暂停直到我恢复"]),
    ("clip_resume", ["Resume capture", "Retomar captura", "Aufzeichnung fortsetzen", "Reanudar captura", "恢复记录"]),
    ("clip_paused", ["Paused", "Pausado", "Pausiert", "En pausa", "已暂停"]),
    ("clip_paused_until", ["Paused until {}", "Pausado até {}", "Pausiert bis {}", "En pausa hasta las {}", "暂停至 {}"]),
    ("clip_pause_help", ["Pause capture (to copy something without saving it)", "Pausar a captura (para copiar algo sem guardar)", "Aufzeichnung pausieren (etwas kopieren, ohne es zu sichern)", "Pausar la captura (para copiar algo sin guardarlo)", "暂停记录（复制内容但不保存）"]),
    ("act_pause_1h", ["Pause the clipboard for 1 hour", "Pausar o clipboard por 1 hora", "Zwischenablage 1 Stunde pausieren", "Pausar el portapapeles 1 hora", "暂停剪贴板 1 小时"]),
    ("act_resume_clip", ["Resume clipboard capture", "Retomar captura do clipboard", "Zwischenablage-Aufzeichnung fortsetzen", "Reanudar la captura del portapapeles", "恢复剪贴板记录"]),
    ("toast_paused_1h", ["Clipboard paused for 1 hour", "Clipboard pausado por 1 hora", "Zwischenablage 1 Stunde pausiert", "Portapapeles en pausa 1 hora", "剪贴板已暂停 1 小时"]),
    ("toast_resumed", ["Clipboard capture resumed", "Captura retomada", "Aufzeichnung fortgesetzt", "Captura reanudada", "已恢复记录"]),
    // ---- clipboard: paste queue (PasteQueue.swift) ----
    ("queue_label", ["QUEUE", "FILA", "WARTESCHLANGE", "COLA", "队列"]),
    ("queue_title", ["Paste queue", "Fila de colagem", "Einfüge-Warteschlange", "Cola de pegado", "粘贴队列"]),
    ("queue_on", ["Paste queue on", "Fila de colagem ligada", "Einfüge-Warteschlange an", "Cola de pegado activada", "粘贴队列已开启"]),
    ("queue_count", ["Paste queue · {}", "Fila de colagem · {}", "Einfüge-Warteschlange · {}", "Cola de pegado · {}", "粘贴队列 · {}"]),
    ("queue_added", ["In queue: {}", "Na fila: {}", "In der Warteschlange: {}", "En la cola: {}", "队列中：{}"]),
    ("queue_done", ["Queue done", "Fila concluída", "Warteschlange fertig", "Cola terminada", "队列已完成"]),
    ("queue_stop", ["Stop", "Parar", "Stopp", "Detener", "停止"]),
    ("queue_stop_help", ["Stop the paste queue", "Parar a fila de colagem", "Einfüge-Warteschlange beenden", "Detener la cola de pegado", "停止粘贴队列"]),
    ("queue_hint", ["Copy items in the order you'll paste them", "Copie os itens na ordem em que vai colar", "Kopiere die Einträge in der Reihenfolge, in der du sie einfügst", "Copia los elementos en el orden en que los vas a pegar", "按粘贴顺序拷贝各项"]),
    // linux: the Mac says "Next ⌘V: ..." because it follows the key; here the step is "Paste next"
    ("queue_next", ["Next: {}", "Próximo: {}", "Nächstes: {}", "Siguiente: {}", "下一个：{}"]),
    ("queue_toggle_help", ["Paste queue: copy several items, then paste them one by one with Paste next", "Fila de colagem: copie vários itens e cole um por um com Colar o próximo", "Einfüge-Warteschlange: mehrere Einträge kopieren und einzeln mit Nächsten einfügen", "Cola de pegado: copia varios elementos y pégalos uno a uno con Pegar el siguiente", "粘贴队列：拷贝多项，再用“粘贴下一项”逐个粘贴"]),
    ("tray_pause_clip", ["Pause clipboard capture", "Pausar a captura do clipboard", "Zwischenablage-Aufzeichnung pausieren", "Pausar la captura del portapapeles", "暂停剪贴板记录"]),
    ("act_paste_next", ["Paste next from the queue", "Colar o próximo da fila", "Nächsten aus der Warteschlange einfügen", "Pegar el siguiente de la cola", "粘贴队列中的下一项"]),
    ("act_queue_on", ["Start the paste queue", "Ligar a fila de colagem", "Einfüge-Warteschlange starten", "Activar la cola de pegado", "开启粘贴队列"]),
    ("queue_ready", ["Next in the queue is ready. Press Ctrl+V to paste", "O próximo da fila está pronto. Pressione Ctrl+V para colar", "Der nächste Eintrag ist bereit. Zum Einfügen Strg+V drücken", "El siguiente de la cola está listo. Pulsa Ctrl+V para pegar", "队列中的下一项已就绪。按 Ctrl+V 粘贴"]),
    ("queue_empty_toast", ["The paste queue is empty", "A fila de colagem está vazia", "Die Warteschlange ist leer", "La cola de pegado está vacía", "粘贴队列为空"]),
    ("shortcut_paste_next_desc", ["Paste the next item of the Menew paste queue", "Colar o próximo item da fila de colagem do Menew", "Nächsten Eintrag der Menew-Einfüge-Warteschlange einfügen", "Pegar el siguiente elemento de la cola de pegado de Menew", "粘贴 Menew 粘贴队列中的下一项"]),
    // ---- clipboard: text of images (TextRecognizer.swift) ----
    ("ocr_none", ["No text found", "Nenhum texto encontrado", "Kein Text gefunden", "No se encontró texto", "未找到文字"]),
    ("ocr_copied", ["Text copied", "Texto copiado", "Text kopiert", "Texto copiado", "已拷贝文字"]),
    ("ocr_copied_lines", ["Text copied ({} lines)", "Texto copiado ({} linhas)", "Text kopiert ({} Zeilen)", "Texto copiado ({} líneas)", "已拷贝文字（{} 行）"]),
    ("ocr_reading", ["Reading text…", "Lendo texto…", "Text wird gelesen …", "Leyendo texto…", "正在读取文字…"]),
    // linux only: the Mac always has Vision
    ("ocr_missing", ["Install tesseract-ocr to read the text of images", "Instale o tesseract-ocr para ler o texto das imagens", "Installiere tesseract-ocr, um Text aus Bildern zu lesen", "Instala tesseract-ocr para leer el texto de las imágenes", "安装 tesseract-ocr 以读取图片中的文字"]),
    // ---- screenshots, text from the screen, QR codes (Capture.swift, QRCode.swift, Palette.swift; docs/screenshots-spec.md) ----
    ("act_capture", ["Capture screen", "Capturar tela", "Bildschirm aufnehmen", "Capturar pantalla", "截屏"]),
    ("act_capture_sub", ["Area or window, with annotations", "Área ou janela, com anotações", "Bereich oder Fenster, mit Anmerkungen", "Área o ventana, con anotaciones", "区域或窗口，可标注"]),
    ("act_capture_full", ["Capture the whole screen", "Capturar a tela inteira", "Ganzen Bildschirm aufnehmen", "Capturar toda la pantalla", "截取整个屏幕"]),
    ("act_ocr", ["Copy text from screen", "Copiar texto da tela", "Text vom Bildschirm kopieren", "Copiar texto de la pantalla", "拷贝屏幕上的文字"]),
    ("act_qr_scan", ["Read QR code from screen", "Ler QR code da tela", "QR-Code vom Bildschirm lesen", "Leer código QR de la pantalla", "从屏幕读取二维码"]),
    ("act_qr_show", ["Show QR code of copied text", "Mostrar QR code do texto copiado", "QR-Code des kopierten Texts zeigen", "Mostrar código QR del texto copiado", "显示所复制文本的二维码"]),
    ("act_qr_wifi", ["Wi-Fi QR code", "QR code do Wi-Fi", "WLAN-QR-Code", "Código QR del Wi-Fi", "Wi-Fi 二维码"]),
    ("smart_qr", ["Show QR code for the link", "Mostrar QR code do link", "QR-Code für den Link zeigen", "Mostrar código QR del enlace", "显示链接的二维码"]),
    ("qr_phone_sub", ["To open on your phone", "Para abrir no celular", "Zum Öffnen auf dem Handy", "Para abrir en el móvil", "在手机上打开"]),
    // linux only: the description the system shows when it asks to confirm the shortcut
    ("shortcut_capture_desc", ["Capture the screen and annotate it", "Capturar a tela e anotar", "Bildschirm aufnehmen und beschriften", "Capturar la pantalla y anotarla", "截屏并标注"]),
    ("shortcut_capture_text_desc", ["Copy text from the screen", "Copiar texto da tela", "Text vom Bildschirm kopieren", "Copiar texto de la pantalla", "拷贝屏幕上的文字"]),
    ("cap_title", ["Screenshot", "Captura", "Bildschirmfoto", "Captura", "截图"]),
    ("cap_tool_pen", ["Pen (freehand)", "Caneta (risco livre)", "Stift (freihand)", "Lápiz (trazo libre)", "画笔（自由绘制）"]),
    ("cap_tool_arrow", ["Arrow", "Seta", "Pfeil", "Flecha", "箭头"]),
    ("cap_tool_rect", ["Rectangle", "Retângulo", "Rechteck", "Rectángulo", "矩形"]),
    ("cap_tool_text", ["Text", "Texto", "Text", "Texto", "文本"]),
    ("cap_tool_blur", ["Blur (hides sensitive data)", "Borrar (esconde dados sensíveis)", "Verpixeln (verbirgt sensible Daten)", "Difuminar (oculta datos sensibles)", "模糊（隐藏敏感信息）"]),
    ("cap_undo", ["Undo (Ctrl+Z)", "Desfazer (Ctrl+Z)", "Widerrufen (Strg+Z)", "Deshacer (Ctrl+Z)", "撤销（Ctrl+Z）"]),
    ("cap_drag", ["Drag into another app", "Arraste para outro app", "In eine andere App ziehen", "Arrastra a otra app", "拖到其他应用"]),
    ("cap_copy_text", ["Copy text", "Copiar texto", "Text kopieren", "Copiar texto", "拷贝文字"]),
    ("cap_save", ["Save…", "Salvar…", "Sichern …", "Guardar…", "存储…"]),
    ("cap_copy", ["Copy", "Copiar", "Kopieren", "Copiar", "复制"]),
    ("cap_close", ["Close", "Fechar", "Schließen", "Cerrar", "关闭"]),
    ("cap_copied", ["Screenshot copied", "Captura copiada", "Bildschirmfoto kopiert", "Captura copiada", "截图已复制"]),
    ("cap_saved", ["Screenshot saved", "Captura salva", "Bildschirmfoto gespeichert", "Captura guardada", "截图已保存"]),
    ("cap_save_failed", ["Couldn't save the screenshot", "Não deu para salvar a captura", "Bildschirmfoto konnte nicht gespeichert werden", "No se pudo guardar la captura", "无法保存截图"]),
    // linux only: the system (the screenshot portal) does the capture and can fail or be refused
    ("cap_portal_failed", ["Couldn't capture the screen: {}", "Não deu para capturar a tela: {}", "Bildschirm konnte nicht aufgenommen werden: {}", "No se pudo capturar la pantalla: {}", "无法截取屏幕：{}"]),
    ("cap_unreadable", ["Couldn't open the capture", "Não deu para abrir a captura", "Die Aufnahme konnte nicht geöffnet werden", "No se pudo abrir la captura", "无法打开截图"]),
    ("qr_none", ["No QR code found", "Nenhum QR code encontrado", "Kein QR-Code gefunden", "No se encontró ningún código QR", "未找到二维码"]),
    ("qr_link_copied", ["QR link copied", "Link do QR copiado", "QR-Link kopiert", "Enlace del QR copiado", "已复制二维码链接"]),
    ("qr_content_copied", ["QR content copied", "Conteúdo do QR copiado", "QR-Inhalt kopiert", "Contenido del QR copiado", "已复制二维码内容"]),
    ("qr_text_or_link", ["Text or link", "Texto ou link", "Text oder Link", "Texto o enlace", "文本或链接"]),
    ("qr_wifi", ["Wi-Fi", "Wi-Fi", "WLAN", "Wi-Fi", "Wi-Fi"]),
    ("qr_network", ["Network name", "Nome da rede", "Netzwerkname", "Nombre de la red", "网络名称"]),
    ("qr_password", ["Password", "Senha", "Passwort", "Contraseña", "密码"]),
    ("qr_hint_text", ["Point your phone's camera to open it there.", "Aponte a câmera do celular para abrir no telefone.", "Richte die Handykamera darauf, um es dort zu öffnen.", "Apunta la cámara del móvil para abrirlo allí.", "用手机相机扫描即可在手机上打开。"]),
    ("qr_hint_wifi", ["Point your phone's camera to join the network.", "Aponte a câmera do celular para entrar na rede.", "Richte die Handykamera darauf, um dem Netzwerk beizutreten.", "Apunta la cámara del móvil para conectarte a la red.", "用手机相机扫描即可加入网络。"]),
    ("qr_copy_image", ["Copy image", "Copiar imagem", "Bild kopieren", "Copiar imagen", "复制图片"]),
    ("qr_image_copied", ["QR code copied", "QR code copiado", "QR-Code kopiert", "Código QR copiado", "已复制二维码"]),
    // ---- clipboard: snippets (Snippets.swift) ----
    ("snip_new", ["New snippet", "Novo snippet", "Neuer Baustein", "Nuevo fragmento", "新建片段"]),
    ("snip_edit", ["Edit snippet", "Editar snippet", "Baustein bearbeiten", "Editar fragmento", "编辑片段"]),
    ("snip_fill", ["Fill in the snippet", "Preencha o snippet", "Textbaustein ausfüllen", "Completa el fragmento", "填写文本片段"]),
    ("snip_none", ["No snippets yet", "Nenhum snippet ainda", "Noch keine Textbausteine", "Aún no hay fragmentos", "还没有文本片段"]),
    ("snip_title_field", ["Title", "Título", "Titel", "Título", "标题"]),
    ("snip_text_field", ["Text", "Texto", "Text", "Texto", "文本"]),
    ("snip_save", ["Save", "Salvar", "Sichern", "Guardar", "保存"]),
    ("snip_cancel", ["Cancel", "Cancelar", "Abbrechen", "Cancelar", "取消"]),
    ("snip_help", [
        "Use {date}, {time}, {weekday}, {tomorrow} and {clipboard} to fill in on paste, {cursor} for where the cursor lands and {input:Name} to ask before pasting.",
        "Use {date}, {time}, {weekday}, {tomorrow} e {clipboard} para preencher na hora, {cursor} para onde o cursor fica e {input:Nome} para perguntar antes de colar.",
        "Nutze {date}, {time}, {weekday}, {tomorrow} und {clipboard} zum Ausfüllen beim Einfügen, {cursor} für die Cursorposition und {input:Name}, um vorher nachzufragen.",
        "Usa {date}, {time}, {weekday}, {tomorrow} y {clipboard} para rellenar al pegar, {cursor} para dónde queda el cursor y {input:Nombre} para preguntar antes de pegar.",
        "使用 {date}、{time}、{weekday}、{tomorrow} 和 {clipboard} 在粘贴时自动填写，{cursor} 指定光标位置，{input:名称} 在粘贴前询问。",
    ]),
    ("snip_ex_date", ["Today's date", "Data de hoje", "Heutiges Datum", "Fecha de hoy", "今天的日期"]),
    ("snip_ex_sig", ["Email signature", "Assinatura de e-mail", "E-Mail-Signatur", "Firma de correo", "邮件签名"]),
    ("snip_ex_sig_text", ["Best,\nYour name", "Abraços,\nSeu nome", "Viele Grüße\nDein Name", "Saludos,\nTu nombre", "祝好，\n你的名字"]),
    // ---- notes (Notes.swift, NotesViews.swift); the Mac texts, except the ones marked "linux" ----
    ("kind_note", ["Note", "Nota", "Notiz", "Nota", "笔记"]),
    ("notes_title", ["Notes", "Notas", "Notizen", "Notas", "笔记"]),
    ("note_untitled", ["Untitled note", "Nota sem título", "Unbenannte Notiz", "Nota sin título", "无标题笔记"]),
    ("note_protected_content", ["Protected content", "Conteúdo protegido", "Geschützter Inhalt", "Contenido protegido", "内容已保护"]),
    ("pw_username", ["Username", "Usuário", "Benutzername", "Usuario", "用户名"]),
    ("pw_password", ["Password", "Senha", "Passwort", "Contraseña", "密码"]),
    ("notes_search", ["Search notes", "Buscar notas", "Notizen durchsuchen", "Buscar notas", "搜索笔记"]),
    ("notes_new", ["New", "Nova", "Neu", "Nueva", "新建"]),
    ("notes_new_passwords", ["New passwords note", "Nova nota de senhas", "Neue Passwortnotiz", "Nueva nota de contraseñas", "新建密码笔记"]),
    ("notes_all", ["All", "Todas", "Alle", "Todas", "全部"]),
    ("notes_none", ["No notes yet", "Nenhuma nota ainda", "Noch keine Notizen", "Aún no hay notas", "还没有笔记"]),
    ("notes_not_found", ["Nothing found", "Nada encontrado", "Nichts gefunden", "Nada encontrado", "未找到内容"]),
    ("notes_hint", ["Use #tags in the text to organize, and Markdown for **bold**, lists and - [ ] tasks.", "Use #tags no texto para organizar e **negrito**, listas e - [ ] tarefas em Markdown.", "Nutze #Tags im Text zum Ordnen und Markdown für **fett**, Listen und - [ ] Aufgaben.", "Usa #etiquetas en el texto para organizar y Markdown para **negrita**, listas y - [ ] tareas.", "在文字中使用 #标签 整理，并用 Markdown 写 **粗体**、列表和 - [ ] 任务。"]),
    ("notes_copy", ["Copy", "Copiar", "Kopieren", "Copiar", "复制"]),
    ("notes_remove_protection", ["Remove protection", "Remover proteção", "Schutz entfernen", "Quitar protección", "取消保护"]),
    // linux: the Mac says "Protect with Touch ID"
    ("notes_protect", ["Protect with your password", "Proteger com a sua senha", "Mit deinem Passwort schützen", "Proteger con tu contraseña", "使用密码保护"]),
    ("notes_delete", ["Delete", "Apagar", "Löschen", "Borrar", "删除"]),
    ("notes_back", ["Back", "Voltar", "Zurück", "Atrás", "返回"]),
    ("notes_edit", ["Edit", "Editar", "Bearbeiten", "Editar", "编辑"]),
    ("notes_read", ["Read", "Ler", "Lesen", "Leer", "阅读"]),
    ("notes_type", ["Type", "Tipo", "Typ", "Tipo", "类型"]),
    ("notes_kind_text", ["Text", "Texto", "Text", "Texto", "文本"]),
    ("notes_kind_passwords", ["Passwords", "Senhas", "Passwörter", "Contraseñas", "密码"]),
    ("notes_delete_note", ["Delete note", "Apagar nota", "Notiz löschen", "Borrar nota", "删除笔记"]),
    ("notes_delete_confirm", ["Delete this note?", "Apagar esta nota?", "Diese Notiz löschen?", "¿Borrar esta nota?", "删除这条笔记？"]),
    ("notes_placeholder", ["Title on the first line…\nUse #tags, **bold**, lists and - [ ] tasks.", "Título na primeira linha…\nUse #tags, **negrito**, listas e - [ ] tarefas.", "Titel in der ersten Zeile…\nNutze #Tags, **fett**, Listen und - [ ] Aufgaben.", "Título en la primera línea…\nUsa #etiquetas, **negrita**, listas y - [ ] tareas.", "第一行是标题…\n可使用 #标签、**粗体**、列表和 - [ ] 任务。"]),
    // linux: the Mac says "Protected in the macOS Keychain"
    ("notes_status_protected", ["Protected in the system keyring", "Protegida no chaveiro do sistema", "Im System-Schlüsselbund geschützt", "Protegida en el llavero del sistema", "已在系统密钥环中保护"]),
    ("notes_status_saved", ["Saved · {} characters", "Salvo · {} caracteres", "Gespeichert · {} Zeichen", "Guardado · {} caracteres", "已保存 · {} 个字符"]),
    ("notes_created", ["Note created", "Nota criada", "Notiz erstellt", "Nota creada", "笔记已创建"]),
    ("pw_title_placeholder", ["Title (e.g. Home Wi-Fi)", "Título (ex.: Wi-Fi de casa)", "Titel (z. B. WLAN zu Hause)", "Título (p. ej., Wi-Fi de casa)", "标题（例如：家里的 Wi-Fi）"]),
    ("pw_add_field", ["Add field", "Adicionar campo", "Feld hinzufügen", "Añadir campo", "添加字段"]),
    ("pw_name", ["Name", "Nome", "Name", "Nombre", "名称"]),
    ("pw_value", ["Value", "Valor", "Wert", "Valor", "值"]),
    ("pw_remove_field", ["Remove field", "Remover campo", "Feld entfernen", "Quitar campo", "删除字段"]),
    ("pw_hide", ["Hide", "Esconder", "Ausblenden", "Ocultar", "隐藏"]),
    ("pw_show", ["Show", "Mostrar", "Anzeigen", "Mostrar", "显示"]),
    ("pw_copied", ["Copied · clears in 30 s", "Copiado · some em 30 s", "Kopiert · wird in 30 s gelöscht", "Copiado · se borra en 30 s", "已复制 · 30 秒后清除"]),
    ("pw_no_username", ["This note has no username", "Essa nota não tem usuário", "Diese Notiz hat keinen Benutzernamen", "Esta nota no tiene usuario", "这条笔记没有用户名"]),
    ("pw_no_password", ["This note has no password", "Essa nota não tem senha", "Diese Notiz hat kein Passwort", "Esta nota no tiene contraseña", "这条笔记没有密码"]),
    ("clip_f_passwords", ["Passwords", "Senhas", "Passwörter", "Contraseñas", "密码"]),
    ("clip_search_passwords", ["Search passwords", "Buscar nas senhas", "Passwörter durchsuchen", "Buscar contraseñas", "搜索密码"]),
    ("clip_paste_password", ["Paste the password", "Colar a senha", "Passwort einfügen", "Pegar la contraseña", "粘贴密码"]),
    ("clip_paste_username", ["Paste the username", "Colar o usuário", "Benutzernamen einfügen", "Pegar el usuario", "粘贴用户名"]),
    // linux: the Mac says "Your Mac asks for your fingerprint first"
    ("pw_row_hint", ["↩ pastes the password · Alt+↩ pastes the username", "↩ cola a senha · Alt+↩ cola o usuário", "↩ fügt das Passwort ein · Alt+↩ den Benutzernamen", "↩ pega la contraseña · Alt+↩ pega el usuario", "↩ 粘贴密码 · Alt+↩ 粘贴用户名"]),
    ("clip_none_passwords", ["No passwords notes. Create one in Notes › new passwords note.", "Nenhuma nota de senhas. Crie em Notas › nova nota de senhas.", "Keine Passwortnotizen. Leg eine in Notizen › neue Passwortnotiz an.", "No hay notas de contraseñas. Crea una en Notas › nueva nota de contraseñas.", "没有密码笔记。在“笔记 › 新建密码笔记”中创建。"]),
    ("clip_hint_passwords", ["Enter: password  |  Alt+Enter: username  |  Alt+←→: filter  |  Tab: apps", "Enter: senha  |  Alt+Enter: usuário  |  Alt+←→: filtro  |  Tab: apps", "Enter: Passwort  |  Alt+Enter: Benutzername  |  Alt+←→: Filter  |  Tab: Apps", "Enter: contraseña  |  Alt+Enter: usuario  |  Alt+←→: filtro  |  Tab: apps", "Enter: 密码  |  Alt+Enter: 用户名  |  Alt+←→: 筛选  |  Tab: 应用"]),
    // linux only: the keyring and polkit
    ("auth_keyring_only", ["This system cannot ask for your password (no polkit policy or agent): the notes are protected by the keyring unlocked at login.", "Este sistema não consegue pedir a sua senha (sem política ou agente do polkit): as notas ficam protegidas pelo chaveiro desbloqueado no login.", "Dieses System kann dein Passwort nicht abfragen (keine polkit-Richtlinie oder kein Agent): Die Notizen sind durch das beim Anmelden entsperrte Schlüsselbund geschützt.", "Este sistema no puede pedir tu contraseña (sin política ni agente de polkit): las notas quedan protegidas por el llavero desbloqueado al iniciar sesión.", "此系统无法询问你的密码（没有 polkit 策略或代理）：笔记仅由登录时解锁的密钥环保护。"]),
    ("auth_denied", ["Not unlocked: the password was not confirmed", "Não abriu: a senha não foi confirmada", "Nicht geöffnet: Das Passwort wurde nicht bestätigt", "No se abrió: no se confirmó la contraseña", "未打开：密码未确认"]),
    ("vault_unavailable", ["No keyring found. Install and unlock GNOME Keyring or KWallet to use protected notes.", "Nenhum chaveiro encontrado. Instale e desbloqueie o GNOME Keyring ou o KWallet para usar notas protegidas.", "Kein Schlüsselbund gefunden. Installiere und entsperre GNOME-Schlüsselbund oder KWallet, um geschützte Notizen zu nutzen.", "No se encontró ningún llavero. Instala y desbloquea GNOME Keyring o KWallet para usar notas protegidas.", "未找到密钥环。请安装并解锁 GNOME 密钥环或 KWallet 以使用受保护的笔记。"]),
    ("vault_failed", ["The keyring did not answer: {}", "O chaveiro não respondeu: {}", "Das Schlüsselbund hat nicht geantwortet: {}", "El llavero no respondió: {}", "密钥环没有响应：{}"]),
    ("act_new_note", ["New note", "Nova nota", "Neue Notiz", "Nueva nota", "新建笔记"]),
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
        for (key, row) in all() {
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
        for (k, _) in all() {
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
