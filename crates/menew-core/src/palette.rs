//! The palette engine (port of `PaletteModel` in `Palette.swift`): what the palette shows for the
//! text typed, in which order, and what each item does. Pure: no GTK, no side effects. The window
//! (`menew-ui`) draws the items and runs their [`ItemAction`]; the daemon (`menew`) owns timers,
//! focus, notifications and the tray.

use crate::calculator;
use crate::clip_item::collapsed_preview;
use crate::clip_smart::{self, SmartRun};
use crate::clipboard_db::ClipKind;
use crate::snippets::Snippet;
use crate::notes::Note;
use crate::currency;
use crate::desktop_entry::DesktopApp;
use crate::dev_tools;
use crate::emoji;
use crate::format::currency_text;
use crate::i18n::Language;
use crate::quick_links::{self, QuickLink};
use crate::recents::Recents;
use crate::script_commands::{self, Script};
use crate::search::{fold, score};
use crate::strings::tr;
use crate::text_transforms::{self, TextTransform};
use crate::time_zones::{self, TimeQuery, TzCtx, Zone};
use crate::timers::{self, TimerStore};

/// Kinds, in the order that breaks ties between equal scores (like `Kind.rawValue` on the Mac).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Calc,
    Action,
    Profile,
    Window,
    App,
    Snippet,
    Note,
    Clip,
    Tip,
}

impl Kind {
    pub fn label(self, lang: Language) -> String {
        let key = match self {
            Kind::Calc => "kind_calc",
            Kind::Action => "kind_action",
            Kind::App => "kind_app",
            Kind::Clip => "kind_clip",
            Kind::Tip => "kind_tip",
            Kind::Snippet => "kind_snippet",
            Kind::Note => "kind_note",
            Kind::Profile | Kind::Window => "kind_action",
        };
        tr(key, lang, &[])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Icon {
    /// A freedesktop icon name.
    Symbol(&'static str),
    /// An emoji or symbol drawn as text.
    Glyph(String),
    /// The icon of a desktop entry.
    App(Option<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemAction {
    /// Nothing (a hint row).
    None,
    /// Copy this text and say "Copied: ...".
    Copy(String),
    /// Open in the browser.
    OpenUri(String),
    /// Launch the desktop entry with this id.
    LaunchApp(String),
    /// Emoji or symbol: copy and paste into the previous app (or just copy).
    PasteText(String),
    /// Apply the transform to the text and copy the result (or show it, for the counts).
    TransformCopy { transform: TextTransform, text: String },
    /// The only network request: fetch the currency rate, then show the converted value.
    FetchRate { amount: f64, from: String, to: String },
    StartTimer { label: String, secs: f64 },
    StartAlarm { label: String, end: f64 },
    CancelTimer(u64),
    FocusStart(u32),
    FocusStop,
    FocusExtend,
    RunScript { script: Script, argument: Option<String> },
    OpenScriptsFolder,
    /// Put this text in the search box (the tips).
    FillQuery(String),
    OpenClipboard,
    ClearClipboard,
    ToggleAutostart,
    GenerateUuid,
    CopyUnixTimestamp,
    Quit,
    /// Paste an entry of the clipboard history.
    PasteClip(String),
    /// Paste an image or file entry of the history (by id).
    PasteEntry(i64),
    /// Paste the snippet with this id (asks its `{input:}` fields first).
    PasteSnippet(String),
    /// Open this note (by id) in the notes window; protected ones ask for the password first.
    OpenNote(String),
    NewNote,
    NewPasswordsNote,
    /// Pause the capture for that many minutes, or until resumed.
    PauseClipboard(Option<u32>),
    ResumeClipboard,
    /// Turn the paste queue on or off.
    ToggleQueue,
    /// Paste the next item of the queue.
    PasteNext,
    /// Capture an area or window (the system asks which) and open the annotation editor.
    Capture,
    /// Capture the whole screen and open the annotation editor.
    CaptureFull,
    /// Capture an area and copy the text in it.
    CaptureText,
    /// Capture an area and copy the content of the QR code in it (a link is never opened).
    QrScan,
    /// Show the QR code of this text, or of the copied text when `None`.
    QrShow(Option<String>),
    /// Show the QR window in its Wi-Fi mode.
    QrWifi,
}

impl ItemAction {
    /// Short text used by the test vectors in `docs/palette-spec.md`.
    pub fn describe(&self) -> String {
        match self {
            ItemAction::None => "none".into(),
            ItemAction::Copy(t) => format!("copy:{t}"),
            ItemAction::OpenUri(u) => format!("open:{u}"),
            ItemAction::LaunchApp(id) => format!("launch:{id}"),
            ItemAction::PasteText(t) => format!("paste:{t}"),
            ItemAction::TransformCopy { transform, .. } => format!("transform:{}", transform.id()),
            ItemAction::FetchRate { amount, from, to } => format!("fetch-rate:{amount} {from}->{to}"),
            ItemAction::StartTimer { label, secs } => format!("timer:{secs}s:{label}"),
            ItemAction::StartAlarm { label, .. } => format!("alarm:{label}"),
            ItemAction::CancelTimer(id) => format!("cancel-timer:{id}"),
            ItemAction::FocusStart(m) => format!("focus-start:{m}"),
            ItemAction::FocusStop => "focus-stop".into(),
            ItemAction::FocusExtend => "focus-extend".into(),
            ItemAction::RunScript { script, argument } => format!(
                "script:{}:{}",
                script.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                argument.clone().unwrap_or_default()
            ),
            ItemAction::OpenScriptsFolder => "open-scripts-folder".into(),
            ItemAction::FillQuery(q) => format!("fill:{q}"),
            ItemAction::OpenClipboard => "open-clipboard".into(),
            ItemAction::ClearClipboard => "clear-clipboard".into(),
            ItemAction::ToggleAutostart => "toggle-autostart".into(),
            ItemAction::GenerateUuid => "generate-uuid".into(),
            ItemAction::CopyUnixTimestamp => "copy-unix-timestamp".into(),
            ItemAction::Quit => "quit".into(),
            ItemAction::PasteClip(_) => "paste-clip".into(),
            ItemAction::PasteEntry(id) => format!("paste-entry:{id}"),
            ItemAction::PasteSnippet(id) => format!("paste-snippet:{id}"),
            ItemAction::OpenNote(id) => format!("open-note:{id}"),
            ItemAction::NewNote => "new-note".into(),
            ItemAction::NewPasswordsNote => "new-passwords-note".into(),
            ItemAction::PauseClipboard(Some(m)) => format!("pause-clipboard:{m}"),
            ItemAction::PauseClipboard(None) => "pause-clipboard".into(),
            ItemAction::ResumeClipboard => "resume-clipboard".into(),
            ItemAction::ToggleQueue => "toggle-queue".into(),
            ItemAction::PasteNext => "paste-next".into(),
            ItemAction::Capture => "capture".into(),
            ItemAction::CaptureFull => "capture-full".into(),
            ItemAction::CaptureText => "capture-text".into(),
            ItemAction::QrScan => "qr-scan".into(),
            ItemAction::QrShow(None) => "qr-show".into(),
            ItemAction::QrShow(Some(t)) => format!("qr-show:{t}"),
            ItemAction::QrWifi => "qr-wifi".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaletteItem {
    pub id: String,
    pub kind: Kind,
    pub title: String,
    pub subtitle: Option<String>,
    /// Extra words for the search (synonyms, other languages).
    pub keywords: String,
    pub icon: Icon,
    pub shortcut: Option<String>,
    /// Goes to the recents (copied items do not, for privacy).
    pub remember: bool,
    /// Does not close the palette.
    pub keep_open: bool,
    pub action: ItemAction,
}

impl PaletteItem {
    pub fn new(id: &str, kind: Kind, title: &str) -> Self {
        Self {
            id: id.to_string(),
            kind,
            title: title.to_string(),
            subtitle: None,
            keywords: String::new(),
            icon: Icon::Symbol("application-x-executable"),
            shortcut: None,
            remember: true,
            keep_open: false,
            action: ItemAction::None,
        }
    }
    pub fn subtitle(mut self, s: &str) -> Self {
        self.subtitle = Some(s.to_string());
        self
    }
    pub fn kw(mut self, k: &str) -> Self {
        self.keywords = k.to_string();
        self
    }
    pub fn icon(mut self, i: Icon) -> Self {
        self.icon = i;
        self
    }
    pub fn shortcut(mut self, s: &str) -> Self {
        self.shortcut = Some(s.to_string());
        self
    }
    /// Not kept in the recents.
    pub fn forget(mut self) -> Self {
        self.remember = false;
        self
    }
    pub fn keep_open(mut self) -> Self {
        self.keep_open = true;
        self
    }
    pub fn act(mut self, a: ItemAction) -> Self {
        self.action = a;
        self
    }
}

/// A clipboard history entry that matched the search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipHit {
    pub id: i64,
    pub title: String,
    pub text: String,
    /// Images and files are pasted by id, text by its content.
    pub kind: ClipKind,
}

/// What the palette needs to know about the clipboard features besides the text on the clipboard.
#[derive(Clone, Copy, Default)]
pub struct ClipContext<'a> {
    /// Country of the locale (`BR`), for the WhatsApp action.
    pub region: Option<&'a str>,
    pub snippets: &'a [Snippet],
    /// Capture is paused.
    pub paused: bool,
    pub queue_on: bool,
    pub queue_len: usize,
}


/// Everything the engine needs to know, so that it stays pure.
pub struct PaletteContext<'a> {
    pub lang: Language,
    pub apps: &'a [DesktopApp],
    pub recents: &'a Recents,
    /// Text on the clipboard now (None when empty or not text).
    pub clipboard_text: Option<String>,
    pub tz: TzCtx,
    pub links: &'a [QuickLink],
    pub world_clocks: &'a [Zone],
    pub timers: &'a TimerStore,
    pub focus_running: bool,
    pub scripts: &'a [Script],
    pub autostart_on: bool,
    pub clip: ClipContext<'a>,
    /// The notes: protected ones have only their title in `text`.
    pub notes: &'a [Note],
}

const SUGGESTIONS: [&str; 4] = ["action.clipboard", "action.focus", "action.clearclip", "action.uuid"];

/// How many items the search keeps (like the Mac).
pub const MAX_RESULTS: usize = 40;

/// Items for the empty search and for any text typed. `clip_search` returns the clipboard history
/// entries matching a text (the caller owns the database).
pub fn results(ctx: &PaletteContext, raw: &str, clip_search: &dyn Fn(&str) -> Vec<ClipHit>) -> Vec<PaletteItem> {
    let lang = ctx.lang;
    let q = fold(raw.trim());
    let mut pool = commands(ctx);

    if q.is_empty() {
        // no search: recents first, then suggestions, then examples of what can be typed
        let by_id = |id: &str| pool.iter().find(|i| i.id == id).cloned();
        let recent: Vec<PaletteItem> = ctx.recents.list.iter().filter_map(|id| by_id(id)).collect();
        let suggested: Vec<PaletteItem> = SUGGESTIONS
            .iter()
            .filter_map(|id| by_id(id))
            .filter(|s| !recent.iter().any(|r| r.id == s.id))
            .collect();
        // what was just copied comes first: a phone, an address, a color, a meeting link (3 at most)
        let smart: Vec<PaletteItem> = ctx.clipboard_text.as_deref().map(|t| smart_items(t, ctx)).unwrap_or_default().into_iter().take(3).collect();
        let mut out: Vec<PaletteItem> = smart.into_iter().chain(recent).chain(suggested).take(5).collect();
        out.extend(tips(lang));
        return out;
    }

    pool.extend(parametric(&q, lang));
    pool.extend(dev_items(ctx, &q));
    pool.extend(quick_links::items(ctx.links, raw, lang));
    pool.extend(script_commands::items(ctx.scripts, raw, lang));
    pool.extend(emoji::items(raw, &q, lang));
    pool.extend(timers::items(ctx.timers, raw, &ctx.tz, lang));
    if let Some(text) = ctx.clipboard_text.as_deref() {
        pool.extend(transform_items(text, &q, lang));
        pool.extend(smart_items(text, ctx));
    }
    for hit in clip_search(raw).into_iter().take(6) {
        let (icon, action) = match hit.kind {
            ClipKind::Text => ("edit-paste-symbolic", ItemAction::PasteClip(hit.text)),
            ClipKind::Image => ("image-x-generic-symbolic", ItemAction::PasteEntry(hit.id)),
            ClipKind::File => ("text-x-generic-symbolic", ItemAction::PasteEntry(hit.id)),
        };
        pool.push(PaletteItem::new(&format!("clip.{}", hit.id), Kind::Clip, &hit.title).icon(Icon::Symbol(icon)).forget().act(action));
    }

    // math and conversions always on top
    let calc = calculator_items(&q, lang);
    let mut scored: Vec<(PaletteItem, i32)> = pool
        .into_iter()
        .filter_map(|item| {
            let base = match item.kind {
                Kind::Clip => 200,
                Kind::Calc => 900,
                _ => score(&q, &item.title, &item.keywords)?,
            };
            let bonus = ctx.recents.bonus(&item.id);
            Some((item, base + bonus))
        })
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.kind.cmp(&b.0.kind)));
    let mut out: Vec<PaletteItem> = calc.into_iter().chain(scored.into_iter().take(MAX_RESULTS).map(|(i, _)| i)).collect();

    // always last: search the text on the web
    let lower = raw.to_lowercase();
    let command_prefix = emoji::PREFIXES.iter().any(|p| lower.starts_with(p));
    if !command_prefix {
        if let Some(search) = quick_links::web_search_item(ctx.links, raw, lang) {
            out.push(search);
        }
    }
    out
}

/// `SmartClip.paletteItems`: what to do with the copied text (call, open in maps, join the meeting...).
fn smart_items(text: &str, ctx: &PaletteContext) -> Vec<PaletteItem> {
    clip_smart::actions(text, ctx.lang, ctx.clip.region)
        .into_iter()
        .map(|a| {
            let action = match a.run {
                SmartRun::Open(u) => ItemAction::OpenUri(u),
                SmartRun::Copy(t) => ItemAction::Copy(t),
                SmartRun::ShowQr(t) => ItemAction::QrShow(Some(t)),
            };
            PaletteItem::new(&format!("smart.{}", a.id), Kind::Action, &a.title).subtitle(&a.subtitle).kw(&a.keywords).icon(Icon::Symbol(a.icon)).forget().act(action)
        })
        .collect()
}

fn tips(lang: Language) -> Vec<PaletteItem> {
    [
        ("tip_timer", "tip_timer_desc", "alarm-symbolic"),
        ("tip_emoji", "tip_emoji_desc", "face-smile-symbolic"),
        ("tip_tz", "tip_tz_desc", "preferences-system-time-symbolic"),
        ("tip_math", "tip_math_desc", "accessories-calculator"),
    ]
    .iter()
    .map(|(cmd, what, icon)| {
        let c = tr(cmd, lang, &[]);
        PaletteItem::new(&format!("tip.{c}"), Kind::Tip, &c)
            .subtitle(&tr(what, lang, &[]))
            .icon(Icon::Symbol(icon))
            .forget()
            .keep_open()
            .act(ItemAction::FillQuery(c.clone()))
    })
    .collect()
}

/// Everything that can run now: the Menew actions and the installed apps.
fn commands(ctx: &PaletteContext) -> Vec<PaletteItem> {
    let lang = ctx.lang;
    let mut out = vec![];
    out.push(
        PaletteItem::new("action.clipboard", Kind::Action, &tr("act_clipboard", lang, &[]))
            .subtitle(&tr("act_sub_clipboard", lang, &[]))
            .kw("clipboard histórico history copiar colar paste área de transferência zwischenablage portapapeles 剪贴板")
            .icon(Icon::Symbol("edit-paste-symbolic"))
            .shortcut("Ctrl+Alt+V")
            .act(ItemAction::OpenClipboard),
    );
    out.push(
        PaletteItem::new("action.clearclip", Kind::Action, &tr("act_clear", lang, &[]))
            .subtitle(&tr("act_clear_sub", lang, &[]))
            .kw("clipboard limpar clear history histórico apagar delete löschen borrar 清除")
            .icon(Icon::Symbol("edit-clear-all-symbolic"))
            .act(ItemAction::ClearClipboard),
    );
    // pause or resume the capture (`clippause`, `clipresume` on the Mac)
    if ctx.clip.paused {
        out.push(
            PaletteItem::new("action.clipresume", Kind::Action, &tr("act_resume_clip", lang, &[]))
                .kw("clipboard retomar resume pausa pause")
                .icon(Icon::Symbol("media-playback-start-symbolic"))
                .act(ItemAction::ResumeClipboard),
        );
    } else {
        out.push(
            PaletteItem::new("action.clippause", Kind::Action, &tr("act_pause_1h", lang, &[]))
                .kw("clipboard pausar pause privacidade privacy")
                .icon(Icon::Symbol("media-playback-pause-symbolic"))
                .act(ItemAction::PauseClipboard(Some(60))),
        );
    }
    // the paste queue (linux: its step is an action, see `paste_queue.rs`)
    out.push(
        PaletteItem::new("action.queue", Kind::Action, &tr(if ctx.clip.queue_on { "queue_stop_help" } else { "act_queue_on" }, lang, &[]))
            .kw("fila colagem paste queue warteschlange cola pegado 粘贴队列 clipboard")
            .icon(Icon::Symbol("view-list-ordered-symbolic"))
            .act(ItemAction::ToggleQueue),
    );
    if ctx.clip.queue_on && ctx.clip.queue_len > 0 {
        out.push(
            PaletteItem::new("action.pastenext", Kind::Action, &tr("act_paste_next", lang, &[]))
                .subtitle(&tr("queue_count", lang, &[&ctx.clip.queue_len.to_string()]))
                .kw("fila colagem paste queue next proximo naechste siguiente 下一项 clipboard")
                .icon(Icon::Symbol("go-next-symbolic"))
                .shortcut("Ctrl+Alt+N")
                .act(ItemAction::PasteNext),
        );
    }
    // the screen (`Palette.commands`: ocr, screenshot, screenshotfull, qrscan, qrshow, qrwifi)
    out.push(
        PaletteItem::new("action.ocr", Kind::Action, &tr("act_ocr", lang, &[]))
            .kw("ocr texto text capturar capture scan ler read textsniper")
            .icon(Icon::Symbol("edit-find-symbolic"))
            .shortcut("Ctrl+Alt+Shift+T")
            .act(ItemAction::CaptureText),
    );
    out.push(
        PaletteItem::new("action.screenshot", Kind::Action, &tr("act_capture", lang, &[]))
            .subtitle(&tr("act_capture_sub", lang, &[]))
            .kw("print screenshot captura capturar tela imagem anotar annotate cleanshot bildschirmfoto captura 截图")
            .icon(Icon::Symbol("applets-screenshooter-symbolic"))
            .shortcut("Ctrl+Alt+Shift+S")
            .act(ItemAction::Capture),
    );
    out.push(
        PaletteItem::new("action.screenshotfull", Kind::Action, &tr("act_capture_full", lang, &[]))
            .kw("print screenshot tela inteira full screen captura")
            .icon(Icon::Symbol("video-display-symbolic"))
            .act(ItemAction::CaptureFull),
    );
    out.push(
        PaletteItem::new("action.qrscan", Kind::Action, &tr("act_qr_scan", lang, &[]))
            .kw("qr qrcode ler scan escanear código code barcode lesen escanear 二维码 扫描")
            .icon(Icon::Symbol("view-grid-symbolic"))
            .act(ItemAction::QrScan),
    );
    out.push(
        PaletteItem::new("action.qrshow", Kind::Action, &tr("act_qr_show", lang, &[]))
            .subtitle(&tr("qr_phone_sub", lang, &[]))
            .kw("qr qrcode gerar generate mostrar show celular phone link compartilhar share 二维码 生成")
            .icon(Icon::Symbol("view-grid-symbolic"))
            .act(ItemAction::QrShow(None)),
    );
    out.push(
        PaletteItem::new("action.qrwifi", Kind::Action, &tr("act_qr_wifi", lang, &[]))
            .kw("qr qrcode wifi wi-fi rede network senha password visitante guest wlan 无线")
            .icon(Icon::Symbol("network-wireless-symbolic"))
            .act(ItemAction::QrWifi),
    );
    // snippets (`Palette.commands`)
    for s in ctx.clip.snippets {
        out.push(
            PaletteItem::new(&format!("snippet.{}", s.id), Kind::Snippet, &s.title)
                .subtitle(&collapsed_preview(&s.text))
                .kw(&format!("snippet {}", s.text))
                .icon(Icon::Symbol("insert-text-symbolic"))
                .act(ItemAction::PasteSnippet(s.id.clone())),
        );
    }
    // notes (`Palette.commands`): the title and preview, the text as keywords (protected ones: title only)
    for n in ctx.notes {
        let icon = if n.is_passwords() { "dialog-password-symbolic" } else if n.secure { "changes-prevent-symbolic" } else { "text-x-generic-symbolic" };
        out.push(
            PaletteItem::new(&format!("note.{}", n.id), Kind::Note, &n.title(lang))
                .subtitle(&n.preview(lang))
                .kw(if n.secure { "" } else { &n.text })
                .icon(Icon::Symbol(icon))
                .act(ItemAction::OpenNote(n.id.clone())),
        );
    }
    // linux: the Mac has these buttons in the notes tab; here they are also actions of the palette
    out.push(
        PaletteItem::new("action.newnote", Kind::Action, &tr("act_new_note", lang, &[]))
            .kw("nota note notiz nueva nova neu 笔记 new")
            .icon(Icon::Symbol("document-new-symbolic"))
            .act(ItemAction::NewNote),
    );
    out.push(
        PaletteItem::new("action.newpasswords", Kind::Action, &tr("notes_new_passwords", lang, &[]))
            .kw("senha password passwort contraseña 密码 nota note notiz new nova")
            .icon(Icon::Symbol("dialog-password-symbolic"))
            .act(ItemAction::NewPasswordsNote),
    );
    out.push(
        PaletteItem::new(
            "action.autostart",
            Kind::Action,
            &tr(if ctx.autostart_on { "act_autostart_off" } else { "act_autostart_on" }, lang, &[]),
        )
        .kw("autostart login iniciar sistema start startup anmelden iniciar sesión 登录 启动")
        .icon(Icon::Symbol("system-run-symbolic"))
        .act(ItemAction::ToggleAutostart),
    );
    if ctx.focus_running {
        out.push(
            PaletteItem::new("action.focusextend", Kind::Action, &tr("tray_focus_extend", lang, &[]))
                .kw("foco focus pomodoro mais more extend estender mais 5 min verlängern ampliar 延长")
                .icon(Icon::Symbol("list-add-symbolic"))
                .act(ItemAction::FocusExtend),
        );
        out.push(
            PaletteItem::new("action.focus", Kind::Action, &tr("stop_focus", lang, &[]))
                .kw("foco focus pomodoro parar stop timer")
                .icon(Icon::Symbol("media-playback-stop-symbolic"))
                .act(ItemAction::FocusStop),
        );
    } else {
        out.push(
            PaletteItem::new("action.focus", Kind::Action, &tr("start_focus", lang, &[]))
                .subtitle(&tr("focus_hint", lang, &[]))
                .kw("foco focus pomodoro concentrar timer")
                .icon(Icon::Symbol("alarm-symbolic"))
                .act(ItemAction::FocusStart(crate::focus::DEFAULT_MINUTES)),
        );
    }
    out.push(
        PaletteItem::new("action.uuid", Kind::Action, &tr("act_uuid", lang, &[]))
            .kw("uuid guid id gerar generate novo new erzeugen generar 生成")
            .icon(Icon::Symbol("dialog-password-symbolic"))
            .act(ItemAction::GenerateUuid),
    );
    out.push(
        PaletteItem::new("action.timestamp", Kind::Action, &tr("act_timestamp", lang, &[]))
            .subtitle(&dev_tools::now_secs().to_string())
            .kw("timestamp unix epoch agora now segundos seconds zeitstempel marca tiempo 时间戳")
            .icon(Icon::Symbol("preferences-system-time-symbolic"))
            .act(ItemAction::CopyUnixTimestamp),
    );
    out.push(
        PaletteItem::new("action.quit", Kind::Action, &tr("act_quit", lang, &[]))
            .kw("sair quit exit fechar close beenden salir 退出")
            .icon(Icon::Symbol("application-exit-symbolic"))
            .act(ItemAction::Quit),
    );
    for app in ctx.apps {
        let sub = app.generic_name.as_deref().or(app.comment.as_deref()).unwrap_or("");
        let kw = format!("{} {}", app.keywords.join(" "), app.generic_name.clone().unwrap_or_default());
        let mut it = PaletteItem::new(&format!("app:{}", app.id), Kind::App, &app.name)
            .kw(&kw)
            .icon(Icon::App(app.icon.clone()))
            .act(ItemAction::LaunchApp(app.id.clone()));
        if !sub.is_empty() {
            it = it.subtitle(sub);
        }
        out.push(it);
    }
    out
}

/// "focus 50": the Mac's "foco 50" with a number.
fn parametric(q: &str, lang: Language) -> Vec<PaletteItem> {
    let parts: Vec<&str> = q.split(' ').filter(|s| !s.is_empty()).collect();
    if parts.len() != 2 {
        return vec![];
    }
    let (word, arg) = (parts[0], parts[1]);
    let mut out = vec![];
    if ["foco", "focus", "fokus", "pomodoro", "专注"].iter().any(|w| w.starts_with(word)) {
        let digits: String = arg.chars().take_while(|c| c.is_numeric()).collect();
        if let Ok(minutes) = digits.parse::<u32>() {
            if (1..=600).contains(&minutes) {
                out.push(
                    PaletteItem::new("focus.timed", Kind::Action, &tr("focus_min", lang, &[&minutes.to_string()]))
                        .kw(q)
                        .icon(Icon::Symbol("alarm-symbolic"))
                        .forget()
                        .act(ItemAction::FocusStart(minutes)),
                );
            }
        }
    }
    out
}

fn same_zone(ctx: &TzCtx, a: &Zone, b: &Zone) -> bool {
    a == b || (matches!(a, Zone::Local) != matches!(b, Zone::Local) && time_zones::resolve(ctx, a) == time_zones::resolve(ctx, b))
}

/// Time zones, world clocks and pasted Unix timestamps.
fn dev_items(ctx: &PaletteContext, q: &str) -> Vec<PaletteItem> {
    let lang = ctx.lang;
    let tz = &ctx.tz;
    let mut out = vec![];
    if let Some(TimeQuery { ts, from, to }) = time_zones::palette_query(tz, q) {
        let there = time_zones::time(tz, ts, &to, lang);
        let here = if from == Zone::Local && !same_zone(tz, &to, &Zone::Local) {
            tr("here", lang, &[&time_zones::time(tz, ts, &Zone::Local, lang)])
        } else {
            tr("time_in", lang, &[&time_zones::time(tz, ts, &from, lang), &time_zones::city_of_zone(tz, &from, lang)])
        };
        let day = time_zones::day_note(tz, ts, &to, &from, lang).map(|d| format!(" \u{b7} {d}")).unwrap_or_default();
        let id = match &to {
            Zone::Named(n) => n.clone(),
            Zone::Local => "local".into(),
        };
        out.push(
            PaletteItem::new(&format!("clock.{id}"), Kind::Calc, &tr("time_in", lang, &[&there, &time_zones::city_of_zone(tz, &to, lang)]))
                .subtitle(&format!("{here} \u{b7} {}{day}", time_zones::offset_text(tz, &to, ts, &from, lang)))
                .kw(q)
                .icon(Icon::Symbol("preferences-system-time-symbolic"))
                .forget()
                .act(ItemAction::Copy(there)),
        );
    }
    // the configured world clocks (Settings comes in Phase 11): "clocks" lists them
    if q.chars().count() >= 3
        && !ctx.world_clocks.is_empty()
        && ["relogio", "relogios", "clock", "clocks", "uhr", "uhren", "reloj", "relojes", "时钟", "world"].iter().any(|w| w.starts_with(q))
    {
        for z in ctx.world_clocks {
            let t = time_zones::time(tz, tz.now, z, lang);
            let day = time_zones::day_note(tz, tz.now, z, &Zone::Local, lang).map(|d| format!(" \u{b7} {d}")).unwrap_or_default();
            let id = match z {
                Zone::Named(n) => n.clone(),
                Zone::Local => "local".into(),
            };
            out.push(
                PaletteItem::new(&format!("worldclock.{id}"), Kind::Calc, &tr("time_in", lang, &[&t, &time_zones::city_of_zone(tz, z, lang)]))
                    .subtitle(&format!("{}{day}", time_zones::offset_text(tz, z, tz.now, &Zone::Local, lang)))
                    .kw(q)
                    .icon(Icon::Symbol("preferences-system-time-symbolic"))
                    .forget()
                    .act(ItemAction::Copy(t)),
            );
        }
    }
    let local_off = |ts: i64| tz.offset(&Zone::Local, ts).unwrap_or(0);
    if let Some(info) = dev_tools::timestamp_query(q, lang, &local_off) {
        out.push(
            PaletteItem::new("timestamp.date", Kind::Calc, &info.text)
                .subtitle(&tr("timestamp_sub", lang, &[&info.iso]))
                .kw(q)
                .icon(Icon::Symbol("x-office-calendar-symbolic"))
                .forget()
                .act(ItemAction::Copy(info.iso)),
        );
    }
    out
}

/// "12*7,5", "20% de 340", "5 km em mi" and "100 usd em brl".
fn calculator_items(q: &str, lang: Language) -> Vec<PaletteItem> {
    let copy_item = |value: &str, detail: &str, id: &str| {
        PaletteItem::new(id, Kind::Calc, value)
            .subtitle(detail)
            .icon(Icon::Symbol("accessories-calculator"))
            .forget()
            .act(ItemAction::Copy(value.to_string()))
    };
    if let Some((v, from, to)) = currency::query(q) {
        let detail = currency_text(v, &from, lang);
        if let Some(rate) = currency::cached_rate(&from, &to) {
            return vec![copy_item(
                &currency_text(v * rate, &to, lang),
                &format!("{detail} \u{b7} {}", tr("ecb_rate", lang, &[])),
                "calc.fx",
            )];
        }
        // the internet is only used when you ask (Enter)
        return vec![PaletteItem::new("calc.fx.fetch", Kind::Calc, &tr("convert_title", lang, &[&detail, &to]))
            .subtitle(&tr("fx_fetch_sub", lang, &[]))
            .icon(Icon::Symbol("view-refresh-symbolic"))
            .forget()
            .keep_open()
            .act(ItemAction::FetchRate { amount: v, from, to })];
    }
    match calculator::evaluate(q, lang) {
        Some(r) => vec![copy_item(&r.value, &r.detail, "calc")],
        None => vec![],
    }
}

/// "uppercase", "json", "base64": transform the copied text, with a live preview.
fn transform_items(text: &str, q: &str, lang: Language) -> Vec<PaletteItem> {
    if q.chars().count() < 2 || text.is_empty() {
        return vec![];
    }
    text_transforms::ALL
        .iter()
        .filter(|t| score(q, t.title(lang), &t.keywords()).is_some())
        .map(|t| {
            PaletteItem::new(&format!("text.{}", t.id()), Kind::Action, t.title(lang))
                .subtitle(&t.preview(text, lang))
                .kw(&t.keywords())
                .icon(Icon::Symbol("format-text-bold-symbolic"))
                .forget()
                .act(ItemAction::TransformCopy { transform: *t, text: text.to_string() })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timers::TimerStore;

    fn run(query: &str, clocks: &[Zone], timers: &TimerStore, focus_running: bool) -> Vec<PaletteItem> {
        let recents = Recents::default();
        let links = quick_links::defaults();
        let ctx = PaletteContext {
            lang: Language::En,
            apps: &[],
            recents: &recents,
            clipboard_text: None,
            tz: TzCtx { now: 1794657600, local: Some("America/Sao_Paulo".into()) },
            links: &links,
            world_clocks: clocks,
            timers,
            focus_running,
            scripts: &[],
            autostart_on: false,
            clip: Default::default(),
            notes: &[],
        };
        results(&ctx, query, &|_| vec![])
    }

    #[test]
    fn world_clocks_from_the_config_are_listed_for_clocks() {
        let clocks = [Zone::Named("Europe/Lisbon".into()), Zone::Named("Asia/Tokyo".into())];
        let out = run("clocks", &clocks, &TimerStore::default(), false);
        let found: Vec<(&str, &str, Option<&str>)> = out
            .iter()
            .filter(|i| i.id.starts_with("worldclock."))
            .map(|i| (i.title.as_str(), i.id.as_str(), i.subtitle.as_deref()))
            .collect();
        assert_eq!(
            found,
            vec![
                ("12:00 PM in Lisbon", "worldclock.Europe/Lisbon", Some("+3 h")),
                ("9:00 PM in Tokyo", "worldclock.Asia/Tokyo", Some("+12 h")),
            ]
        );
        // nothing configured, nothing listed
        assert!(run("clocks", &[], &TimerStore::default(), false).iter().all(|i| !i.id.starts_with("worldclock.")));
    }

    #[test]
    fn running_timers_can_be_cancelled_from_the_palette() {
        let mut timers = TimerStore::default();
        let id = timers.add("tea", 1794657600.0 + 600.0, false);
        let out = run("timer", &[], &timers, false);
        let cancel = out.iter().find(|i| i.id == format!("timer.cancel.{id}")).expect("a cancel item");
        assert_eq!(cancel.title, "Cancel the 9:10 AM timer");
        assert_eq!(cancel.subtitle.as_deref(), Some("tea"));
        assert_eq!(cancel.action, ItemAction::CancelTimer(id));
    }

    #[test]
    fn focus_actions_follow_the_state() {
        let idle = run("focus", &[], &TimerStore::default(), false);
        assert!(idle.iter().any(|i| i.id == "action.focus" && i.action == ItemAction::FocusStart(25)));
        assert!(!idle.iter().any(|i| i.id == "action.focusextend"));
        let on = run("focus", &[], &TimerStore::default(), true);
        assert!(on.iter().any(|i| i.id == "action.focus" && i.action == ItemAction::FocusStop));
        assert!(on.iter().any(|i| i.id == "action.focusextend" && i.action == ItemAction::FocusExtend));
    }

    #[test]
    fn at_most_40_results_plus_the_web_search() {
        let apps: Vec<DesktopApp> = (0..60)
            .map(|i| DesktopApp {
                id: format!("app{i}"),
                name: format!("Appy {i}"),
                generic_name: None,
                comment: None,
                exec: "x".into(),
                icon: None,
                categories: vec![],
                keywords: vec![],
                terminal: false,
                file_path: std::path::PathBuf::from("/x"),
            })
            .collect();
        let recents = Recents::default();
        let links = quick_links::defaults();
        let timers = TimerStore::default();
        let ctx = PaletteContext {
            lang: Language::En,
            apps: &apps,
            recents: &recents,
            clipboard_text: None,
            tz: TzCtx { now: 1794657600, local: Some("America/Sao_Paulo".into()) },
            links: &links,
            world_clocks: &[],
            timers: &timers,
            focus_running: false,
            scripts: &[],
            autostart_on: false,
            clip: Default::default(),
            notes: &[],
        };
        let out = results(&ctx, "appy", &|_| vec![]);
        assert_eq!(out.len(), MAX_RESULTS + 1);
        assert_eq!(out.last().unwrap().id, "link.search");
    }

    #[test]
    fn clipboard_history_hits_come_after_everything_scored_higher() {
        let recents = Recents::default();
        let links = quick_links::defaults();
        let timers = TimerStore::default();
        let ctx = PaletteContext {
            lang: Language::En,
            apps: &[],
            recents: &recents,
            clipboard_text: None,
            tz: TzCtx { now: 1794657600, local: Some("America/Sao_Paulo".into()) },
            links: &links,
            world_clocks: &[],
            timers: &timers,
            focus_running: false,
            scripts: &[],
            autostart_on: false,
            clip: Default::default(),
            notes: &[],
        };
        let hit = |id: i64, t: &str| ClipHit { id, title: t.into(), text: t.into(), kind: ClipKind::Text };
        let out = results(&ctx, "history", &|_| vec![hit(1, "my history notes")]);
        let ids: Vec<&str> = out.iter().map(|i| i.id.as_str()).collect();
        // actions scored 633 and 627 come first, the clipboard hit (200) after, the web search last
        assert_eq!(ids, vec!["action.clipboard", "action.clearclip", "clip.1", "link.search"]);
        assert_eq!(out[2].action, ItemAction::PasteClip("my history notes".into()));
        assert!(!out[2].remember);
    }
}
