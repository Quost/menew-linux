use menew_core::focus::FocusTimer;
use menew_core::quick_links::{self, QuickLink};
use menew_core::recents::Recents;
use menew_core::time_zones::Zone;
use menew_core::timers::TimerStore;
use menew_core::Config;
use std::cell::RefCell;
use std::rc::Rc;
use tracing::info;

/// State of the palette that outlives one search: recent picks, running timers, the focus session
/// and what comes from `config.json`. Shared with the daemon, which ticks the timers.
pub struct PaletteState {
    pub recents: Recents,
    pub timers: TimerStore,
    pub focus: FocusTimer,
    pub links: Vec<QuickLink>,
    pub clocks: Vec<Zone>,
}

impl PaletteState {
    pub fn from_config(cfg: &Config) -> Self {
        let clocks = cfg.world_clocks.iter().filter_map(|id| menew_core::time_zones::named_zone(id)).collect();
        Self {
            recents: Recents::load(),
            timers: TimerStore::default(),
            focus: FocusTimer { breaks: cfg.focus_breaks, ..FocusTimer::default() },
            links: cfg.quick_links.clone().unwrap_or_else(quick_links::defaults),
            clocks,
        }
    }
}

pub type SharedState = Rc<RefCell<PaletteState>>;

#[cfg(target_os = "linux")]
pub mod palette {
    use super::*;
    use gtk4::gdk;
    use gtk4::gio;
    use gtk4::glib;
    use gtk4::prelude::*;
    use libadwaita as adw;
    use libadwaita::prelude::*;
    use menew_core::currency;
    use menew_core::dev_tools;
    use menew_core::focus::{NoHooks, EXTEND_MINUTES};
    use menew_core::timers;
    use menew_core::palette::{results, ClipHit, Icon, ItemAction, Kind, PaletteContext, PaletteItem};
    use menew_core::script_commands::{self, Mode, Script};
    use menew_core::strings::tr;
    use menew_core::time_zones::TzCtx;
    use menew_core::{
        age_between, current_language, decide_paste, format_age, preview, sort_for_display, t, ClipboardDatabase,
        ClipboardEntry, DesktopApp, DesktopScanner, Language, PaletteMode, PasteAction, PREVIEW_MAX_CHARS,
    };
    use std::cell::{Cell, RefCell};
    use std::time::{Duration, Instant};

    /// Square window frame fix: the window itself is transparent and the rounded,
    /// bordered card is drawn by `.menew-card`.
    const CSS: &str = "
        window.menew-palette,
        window.menew-palette.background,
        window.menew-palette.csd,
        window.menew-palette decoration {
            background: transparent;
            background-color: transparent;
            box-shadow: none;
            border: none;
            outline: none;
            border-radius: 0;
            margin: 0;
        }
        .menew-card {
            background-color: @window_bg_color;
            color: @window_fg_color;
            border-radius: 14px;
            border: 1px solid alpha(currentColor, 0.18);
        }
        .menew-footer {
            font-size: 0.85em;
            opacity: 0.7;
        }
        .menew-glyph {
            font-size: 1.5em;
        }
        .menew-section {
            font-size: 0.85em;
            font-weight: bold;
            opacity: 0.7;
        }
    ";

    const LIST_LIMIT: usize = 100;

    pub struct PaletteWindow {
        app: adw::Application,
        db: ClipboardDatabase,
        state: SharedState,
        /// Called after the palette changed timers or focus, so the daemon updates the tray.
        on_state: Rc<dyn Fn()>,
        window: adw::ApplicationWindow,
        search_entry: gtk4::SearchEntry,
        list_box: gtk4::ListBox,
        mode_label: gtk4::Label,
        hint_label: gtk4::Label,
        mode: Cell<PaletteMode>,
        apps: Rc<RefCell<Vec<DesktopApp>>>,
        /// The items of the list in Apps mode, in the order of the rows.
        items: Rc<RefCell<Vec<PaletteItem>>>,
        visible_entries: Rc<RefCell<Vec<ClipboardEntry>>>,
        /// Text of the clipboard as read by GTK while the palette is open.
        clip_text: RefCell<Option<String>>,
        scripts: RefCell<(Option<Instant>, Vec<Script>)>,
        query_empty: Rc<Cell<bool>>,
        /// The text the current list was built for.
        last_query: RefCell<String>,
    }

    /// Operation simulated by the `MENEW_TEST=1` hooks.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TestOp {
        Select,
        Pin,
        Delete,
    }

    enum Keep {
        Index(usize),
        EntryId(i64),
    }

    fn now_secs() -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }

    impl PaletteWindow {
        pub fn create(app: &adw::Application, db: ClipboardDatabase, state: SharedState, on_state: Rc<dyn Fn()>) -> Rc<Self> {
            if let Some(display) = gdk::Display::default() {
                let provider = gtk4::CssProvider::new();
                provider.load_from_data(CSS);
                gtk4::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }

            let window = adw::ApplicationWindow::builder()
                .application(app)
                .title("Menew")
                .default_width(640)
                .default_height(460)
                .resizable(false)
                .modal(false)
                .decorated(false)
                .build();

            window.add_css_class("menew-palette");

            let main_box = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Vertical)
                .spacing(0)
                .css_classes(["menew-card"])
                .overflow(gtk4::Overflow::Hidden)
                .build();

            let search_entry = gtk4::SearchEntry::builder()
                .placeholder_text(t("search_placeholder"))
                .hexpand(true)
                .margin_top(12)
                .margin_bottom(8)
                .margin_start(12)
                .margin_end(12)
                .build();
            main_box.append(&search_entry);

            main_box.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

            let scrolled = gtk4::ScrolledWindow::builder()
                .hscrollbar_policy(gtk4::PolicyType::Never)
                .vscrollbar_policy(gtk4::PolicyType::Automatic)
                .vexpand(true)
                .hexpand(true)
                .min_content_height(340)
                .margin_top(8)
                .margin_start(8)
                .margin_end(8)
                .build();

            let list_box = gtk4::ListBox::builder()
                .selection_mode(gtk4::SelectionMode::Single)
                .css_classes(["boxed-list"])
                .build();

            scrolled.set_child(Some(&list_box));
            main_box.append(&scrolled);

            // Footer: current mode on the left, key hints on the right
            let footer = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Horizontal)
                .margin_top(6)
                .margin_bottom(8)
                .margin_start(14)
                .margin_end(14)
                .build();
            let mode_label = gtk4::Label::builder()
                .halign(gtk4::Align::Start)
                .css_classes(["menew-footer", "heading"])
                .build();
            let hint_label = gtk4::Label::builder()
                .halign(gtk4::Align::End)
                .hexpand(true)
                .css_classes(["menew-footer"])
                .build();
            footer.append(&mode_label);
            footer.append(&hint_label);
            main_box.append(&footer);

            window.set_content(Some(&main_box));

            let items: Rc<RefCell<Vec<PaletteItem>>> = Rc::new(RefCell::new(Vec::new()));
            let query_empty = Rc::new(Cell::new(true));

            // "Recent and suggested" above the first row of an empty search, "Try typing" above the tips
            {
                let items = Rc::clone(&items);
                let query_empty = Rc::clone(&query_empty);
                list_box.set_header_func(move |row, _before| {
                    let items = items.borrow();
                    let i = row.index();
                    if i < 0 || items.is_empty() {
                        row.set_header(gtk4::Widget::NONE);
                        return;
                    }
                    let i = i as usize;
                    let lang = current_language();
                    let text = match items.get(i) {
                        Some(it) if it.kind == Kind::Tip && (i == 0 || items[i - 1].kind != Kind::Tip) => Some(tr("try_typing", lang, &[])),
                        Some(it) if i == 0 && query_empty.get() && it.kind != Kind::Tip => Some(tr("recent_suggested", lang, &[])),
                        _ => None,
                    };
                    match text {
                        Some(txt) => {
                            let label = gtk4::Label::builder()
                                .label(txt)
                                .halign(gtk4::Align::Start)
                                .margin_top(8)
                                .margin_bottom(4)
                                .margin_start(6)
                                .css_classes(["menew-section"])
                                .build();
                            row.set_header(Some(&label));
                        }
                        None => row.set_header(gtk4::Widget::NONE),
                    }
                });
            }

            let palette = Rc::new(Self {
                app: app.clone(),
                db,
                state,
                on_state,
                window,
                search_entry,
                list_box,
                mode_label,
                hint_label,
                mode: Cell::new(PaletteMode::Apps),
                apps: Rc::new(RefCell::new(DesktopScanner::scan_all())),
                items,
                visible_entries: Rc::new(RefCell::new(Vec::new())),
                clip_text: RefCell::new(None),
                scripts: RefCell::new((None, Vec::new())),
                query_empty,
                last_query: RefCell::new(String::new()),
            });

            palette.init_events();
            palette.apply_mode_chrome();
            palette.refresh_list("");

            palette
        }

        fn init_events(self: &Rc<Self>) {
            let self_search = Rc::clone(self);
            self.search_entry.connect_search_changed(move |entry| {
                let query = entry.text().to_string();
                self_search.refresh_list(&query);
            });

            let self_stop = Rc::clone(self);
            self.search_entry.connect_stop_search(move |_| {
                self_stop.hide();
            });

            let self_act = Rc::clone(self);
            self.search_entry.connect_activate(move |_| {
                self_act.activate_selected();
            });

            // Capture phase: intercept before child widgets (Tab would otherwise move focus)
            let key_controller = gtk4::EventControllerKey::new();
            key_controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
            let self_key = Rc::clone(self);
            key_controller.connect_key_pressed(move |_, keyval, _, state| {
                let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
                let in_clipboard = self_key.mode.get() == PaletteMode::Clipboard;
                match keyval {
                    gdk::Key::Escape => {
                        self_key.hide();
                        glib::Propagation::Stop
                    }
                    gdk::Key::Tab | gdk::Key::ISO_Left_Tab => {
                        self_key.set_mode(self_key.mode.get().toggled());
                        glib::Propagation::Stop
                    }
                    gdk::Key::Return | gdk::Key::KP_Enter => {
                        self_key.activate_selected();
                        glib::Propagation::Stop
                    }
                    gdk::Key::Down => {
                        self_key.move_selection(1);
                        glib::Propagation::Stop
                    }
                    gdk::Key::Up => {
                        self_key.move_selection(-1);
                        glib::Propagation::Stop
                    }
                    gdk::Key::p | gdk::Key::P if ctrl && in_clipboard => {
                        self_key.pin_selected();
                        glib::Propagation::Stop
                    }
                    gdk::Key::Delete | gdk::Key::KP_Delete if ctrl && in_clipboard => {
                        self_key.delete_selected();
                        glib::Propagation::Stop
                    }
                    // Ctrl+1..9 runs the Nth result (Cmd+1..9 on the Mac)
                    k if ctrl
                        && !in_clipboard
                        && k.to_unicode().and_then(|c| c.to_digit(10)).map(|d| (1..=9).contains(&d)).unwrap_or(false) =>
                    {
                        let n = k.to_unicode().and_then(|c| c.to_digit(10)).unwrap_or(1) as usize - 1;
                        self_key.activate_index(n);
                        glib::Propagation::Stop
                    }
                    _ => glib::Propagation::Proceed,
                }
            });
            self.window.add_controller(key_controller);

            self.search_entry.set_key_capture_widget(Some(&self.window));

            let self_row = Rc::clone(self);
            self.list_box.connect_row_activated(move |_, row| {
                let idx = row.index();
                if idx >= 0 {
                    self_row.activate_index(idx as usize);
                }
            });
        }

        pub fn mode(&self) -> PaletteMode {
            self.mode.get()
        }

        fn apply_mode_chrome(&self) {
            match self.mode.get() {
                PaletteMode::Apps => {
                    self.search_entry.set_placeholder_text(Some(t("search_placeholder")));
                    self.mode_label.set_label(t("mode_apps"));
                    self.hint_label.set_label(t("apps_hint"));
                }
                PaletteMode::Clipboard => {
                    self.search_entry.set_placeholder_text(Some(t("clipboard_placeholder")));
                    self.mode_label.set_label(t("mode_clipboard"));
                    self.hint_label.set_label(t("clipboard_hint"));
                }
            }
        }

        /// Switch mode and reset the query.
        pub fn set_mode(self: &Rc<Self>, mode: PaletteMode) {
            self.mode.set(mode);
            self.apply_mode_chrome();
            self.search_entry.set_text("");
            self.refresh_list("");
        }

        pub fn refresh_list(self: &Rc<Self>, query: &str) {
            self.refresh_list_selecting(query, None);
        }

        /// Rebuild the list. `keep` selects the row at that index (clamped) or the entry with that id.
        fn refresh_list_selecting(self: &Rc<Self>, query: &str, keep: Option<Keep>) {
            while let Some(child) = self.list_box.first_child() {
                self.list_box.remove(&child);
            }
            self.items.borrow_mut().clear();
            self.visible_entries.borrow_mut().clear();
            self.query_empty.set(query.trim().is_empty());
            *self.last_query.borrow_mut() = query.to_string();

            match self.mode.get() {
                PaletteMode::Apps => self.fill_apps(query),
                PaletteMode::Clipboard => self.fill_clipboard(query.trim()),
            }

            let total = match self.mode.get() {
                PaletteMode::Apps => self.items.borrow().len(),
                PaletteMode::Clipboard => self.visible_entries.borrow().len(),
            } as i32;
            if total == 0 {
                let text = if self.mode.get() == PaletteMode::Clipboard && query.trim().is_empty() {
                    t("clipboard_empty").to_string()
                } else if self.mode.get() == PaletteMode::Clipboard {
                    t("no_results").to_string()
                } else {
                    tr("nothing_found", current_language(), &[query.trim()])
                };
                let empty_label = gtk4::Label::builder()
                    .label(text)
                    .margin_top(32)
                    .margin_bottom(32)
                    .css_classes(["dim-label"])
                    .build();
                self.list_box.append(&empty_label);
                return;
            }

            let mut target = 0;
            match keep {
                Some(Keep::Index(i)) => target = (i as i32).clamp(0, total - 1),
                Some(Keep::EntryId(id)) => {
                    if let Some(pos) = self.visible_entries.borrow().iter().position(|e| e.id == id) {
                        target = pos as i32;
                    }
                }
                None => {}
            }
            if let Some(row) = self.list_box.row_at_index(target) {
                self.list_box.select_row(Some(&row));
            }
        }

        /// Scripts of the folder, re-read at most every 3 seconds (the palette asks on every key).
        fn scripts(&self) -> Vec<Script> {
            let mut cache = self.scripts.borrow_mut();
            let stale = cache.0.map(|t| t.elapsed() > Duration::from_secs(3)).unwrap_or(true);
            if stale {
                *cache = (Some(Instant::now()), script_commands::scan(&script_commands::folder()));
            }
            cache.1.clone()
        }

        /// The text that transforms apply to: what GTK read from the clipboard, else the newest
        /// history entry (GNOME does not give a background read, the extension fills the history).
        fn clipboard_text(&self) -> Option<String> {
            let gtk = self.clip_text.borrow().clone();
            let text = gtk.or_else(|| {
                self.db.get_newest().ok().flatten().map(|e| e.text_content)
            });
            text.filter(|t| !t.is_empty())
        }

        fn fill_apps(self: &Rc<Self>, raw: &str) {
            let lang = current_language();
            let scripts = self.scripts();
            let clip_text = if raw.trim().is_empty() { None } else { self.clipboard_text() };
            let items = {
                let state = self.state.borrow();
                let apps = self.apps.borrow();
                let ctx = PaletteContext {
                    lang,
                    apps: &apps,
                    recents: &state.recents,
                    clipboard_text: clip_text,
                    tz: TzCtx::system(),
                    links: &state.links,
                    world_clocks: &state.clocks,
                    timers: &state.timers,
                    focus_running: state.focus.is_running(),
                    scripts: &scripts,
                    autostart_on: menew_core::autostart::is_enabled(),
                };
                let db = self.db.clone();
                let clip_search = move |q: &str| -> Vec<ClipHit> {
                    db.search(q, 6)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|e| ClipHit { id: e.id, title: preview(&e.text_content, PREVIEW_MAX_CHARS), text: e.text_content })
                        .collect()
                };
                results(&ctx, raw, &clip_search)
            };
            for item in &items {
                self.list_box.append(&self.build_row(item, lang));
            }
            *self.items.borrow_mut() = items;
        }

        fn build_row(&self, item: &PaletteItem, lang: Language) -> adw::ActionRow {
            let row = adw::ActionRow::builder()
                .title(&item.title)
                .use_markup(false)
                .title_lines(1)
                .subtitle_lines(1)
                .build();
            if let Some(sub) = &item.subtitle {
                row.set_subtitle(sub);
            }
            match &item.icon {
                Icon::Symbol(name) => row.add_prefix(&gtk4::Image::from_icon_name(name)),
                Icon::App(name) => {
                    row.add_prefix(&gtk4::Image::from_icon_name(name.as_deref().unwrap_or("application-x-executable")))
                }
                Icon::Glyph(g) => row.add_prefix(&gtk4::Label::builder().label(g).css_classes(["menew-glyph"]).build()),
            }
            if let Some(sc) = &item.shortcut {
                row.add_suffix(&gtk4::Label::builder().label(sc).css_classes(["dim-label", "caption"]).build());
            }
            row.add_suffix(&gtk4::Label::builder().label(item.kind.label(lang)).css_classes(["dim-label", "caption"]).build());
            row
        }

        fn fill_clipboard(&self, clean_query: &str) {
            let result = if clean_query.is_empty() {
                self.db.get_recent(LIST_LIMIT)
            } else {
                self.db.search(clean_query, LIST_LIMIT)
            };
            let mut entries = result.unwrap_or_default();
            sort_for_display(&mut entries);

            let now = now_ms();
            for entry in &entries {
                let first_line = preview(&entry.text_content, PREVIEW_MAX_CHARS);
                let title = if first_line.is_empty() { "...".to_string() } else { first_line };
                let age = format_age(age_between(now, entry.created_at));
                let subtitle = if entry.is_pinned {
                    format!("{} \u{00b7} {}", t("pinned"), age)
                } else {
                    age
                };

                let row = adw::ActionRow::builder()
                    .title(&title)
                    .subtitle(&subtitle)
                    .title_lines(1)
                    .use_markup(false)
                    .icon_name("edit-paste-symbolic")
                    .build();
                if entry.is_pinned {
                    row.add_suffix(&gtk4::Image::from_icon_name("view-pin-symbolic"));
                }
                self.list_box.append(&row);
            }
            *self.visible_entries.borrow_mut() = entries;
        }

        pub fn toggle(self: &Rc<Self>) {
            if self.window.is_visible() {
                self.hide();
            } else {
                self.show();
            }
        }

        /// Show in Apps mode.
        pub fn show(self: &Rc<Self>) {
            self.show_mode(PaletteMode::Apps);
        }

        /// Show (or switch an already visible palette) in the given mode.
        pub fn show_mode(self: &Rc<Self>, mode: PaletteMode) {
            if mode == PaletteMode::Apps {
                // Reload desktop files to catch newly installed apps
                *self.apps.borrow_mut() = DesktopScanner::scan_all();
                self.read_clipboard();
            }
            self.set_mode(mode);
            self.window.set_visible(true);
            self.window.present();
            self.search_entry.grab_focus();
            info!("Palette window presented and focused ({:?})", mode);
        }

        /// Read the clipboard text for the transforms (not when it carries a password hint).
        fn read_clipboard(self: &Rc<Self>) {
            *self.clip_text.borrow_mut() = None;
            let Some(display) = gdk::Display::default() else { return };
            let clipboard = display.clipboard();
            let formats = clipboard.formats();
            if formats.contain_mime_type("x-kde-passwordManagerHint") || formats.contain_mime_type("application/x-kde-passwordManagerHint") {
                return;
            }
            let this = Rc::clone(self);
            clipboard.read_text_async(gio::Cancellable::NONE, move |res| {
                if let Ok(Some(text)) = res {
                    *this.clip_text.borrow_mut() = Some(text.to_string());
                    let q = this.search_entry.text().to_string();
                    if this.window.is_visible() && this.mode.get() == PaletteMode::Apps && !q.trim().is_empty() {
                        this.refresh_list(&q);
                    }
                }
            });
        }

        /// Toggle with the given mode: hide when already visible in that mode.
        pub fn toggle_mode(self: &Rc<Self>, mode: PaletteMode) {
            if self.window.is_visible() && self.mode.get() == mode {
                self.hide();
            } else {
                self.show_mode(mode);
            }
        }

        pub fn hide(self: &Rc<Self>) {
            self.window.set_visible(false);
            self.search_entry.set_text("");
            info!("Palette window hidden");
        }

        fn move_selection(self: &Rc<Self>, delta: i32) {
            let total = match self.mode.get() {
                PaletteMode::Apps => self.items.borrow().len(),
                PaletteMode::Clipboard => self.visible_entries.borrow().len(),
            } as i32;
            if total == 0 {
                return;
            }

            let current = self.list_box.selected_row().map(|r| r.index()).unwrap_or(0);

            let next = (current + delta).clamp(0, total - 1);
            if let Some(target_row) = self.list_box.row_at_index(next) {
                self.list_box.select_row(Some(&target_row));
            }
        }

        fn selected_index(&self) -> Option<usize> {
            self.list_box
                .selected_row()
                .map(|r| r.index())
                .filter(|i| *i >= 0)
                .map(|i| i as usize)
        }

        fn activate_selected(self: &Rc<Self>) {
            if let Some(idx) = self.selected_index() {
                self.activate_index(idx);
            }
        }

        fn activate_index(self: &Rc<Self>, index: usize) {
            if self.mode.get() == PaletteMode::Clipboard {
                self.activate_clipboard_index(index);
                return;
            }

            // GtkSearchEntry delays "search-changed" by 150 ms: make sure the list is the one for the
            // text on screen before running anything (typing fast and pressing Enter).
            let typed = self.search_entry.text().to_string();
            if *self.last_query.borrow() != typed {
                self.refresh_list(&typed);
            }

            // Drop the borrow before hiding or running anything that may borrow again
            let item = self.items.borrow().get(index).cloned();
            let Some(item) = item else { return };
            info!("Palette: running item {} ({})", item.id, item.action.describe());
            if item.keep_open {
                self.run_action(item.action);
                return;
            }
            if item.remember {
                let mut st = self.state.borrow_mut();
                st.recents.remember(&item.id);
                let _ = st.recents.save_to(&menew_core::recents::default_path());
            }
            self.hide();
            // let the palette go away and the previous window get the keyboard back
            let this = Rc::clone(self);
            glib::timeout_add_local_once(Duration::from_millis(50), move || this.run_action(item.action));
        }

        // ---- what the items do ----

        fn notify(&self, id: &str, title: &str, body: Option<&str>) {
            let n = gio::Notification::new(title);
            if let Some(b) = body {
                n.set_body(Some(b));
            }
            self.app.send_notification(Some(id), &n);
            info!("Notification sent: {}{}", title, body.map(|b| format!(" / {b}")).unwrap_or_default());
        }

        fn copied(&self, text: &str) {
            self.copy_to_clipboard(text);
            self.notify("menew-copied", &tr("copied_value", current_language(), &[text]), None);
        }

        fn open_uri(&self, uri: &str) {
            match gio::AppInfo::launch_default_for_uri(uri, gio::AppLaunchContext::NONE) {
                Ok(()) => info!("Opened {}", uri),
                Err(e) => tracing::warn!("Could not open {}: {}", uri, e),
            }
        }

        fn state_changed(&self) {
            (self.on_state)();
        }

        fn run_action(self: &Rc<Self>, action: ItemAction) {
            let lang = current_language();
            match action {
                ItemAction::None => {}
                ItemAction::Copy(text) => self.copied(&text),
                ItemAction::OpenUri(uri) => self.open_uri(&uri),
                ItemAction::LaunchApp(id) => {
                    let app = self.apps.borrow().iter().find(|a| a.id == id).cloned();
                    if let Some(app) = app {
                        info!("Launching app from palette: {}", app.name);
                        if let Err(e) = app.launch() {
                            tracing::error!("Failed to launch app {}: {}", app.name, e);
                        }
                    }
                }
                ItemAction::PasteText(text) | ItemAction::PasteClip(text) => self.commit_clipboard_text(&text),
                ItemAction::TransformCopy { transform, text } => match transform.apply(&text, lang) {
                    None => self.notify("menew-text", transform.failure(lang), None),
                    Some(result) if transform.is_info() => self.notify("menew-text", &result, None),
                    Some(result) => {
                        self.notify("menew-text", &tr("tt_done", lang, &[]), None);
                        self.commit_clipboard_text(&result);
                    }
                },
                ItemAction::FetchRate { amount, from, to } => {
                    info!("Currency: fetching {} -> {} (requested with Enter)", from, to);
                    let this = Rc::clone(self);
                    glib::spawn_future_local(async move {
                        let (f, t2) = (from.clone(), to.clone());
                        let rate = gio::spawn_blocking(move || currency::fetch_rate(&f, &t2)).await.ok().flatten();
                        match rate {
                            Some(r) => info!("Currency: {} {} = {} {}", amount, from, amount * r, to),
                            None => this.notify("menew-fx", &tr("fx_fail", lang, &[]), None),
                        }
                        let q = this.search_entry.text().to_string();
                        if this.window.is_visible() {
                            this.refresh_list(&q);
                        }
                    });
                }
                ItemAction::StartTimer { label, secs } => {
                    let end = now_secs() + secs;
                    self.state.borrow_mut().timers.add(&label, end, false);
                    let when = timers::clock(end, &TzCtx::system(), lang);
                    self.notify("menew-timer-new", &tr("toast_timer_set", lang, &[&when]), (!label.is_empty()).then_some(label.as_str()));
                    self.state_changed();
                }
                ItemAction::StartAlarm { label, end } => {
                    self.state.borrow_mut().timers.add(&label, end, true);
                    let when = timers::clock(end, &TzCtx::system(), lang);
                    self.notify("menew-timer-new", &tr("toast_alarm_set", lang, &[&when]), (!label.is_empty()).then_some(label.as_str()));
                    self.state_changed();
                }
                ItemAction::CancelTimer(id) => {
                    if self.state.borrow_mut().timers.cancel(id) {
                        self.notify("menew-timer-new", &tr("timer_cancelled", lang, &[]), None);
                        self.state_changed();
                    }
                }
                ItemAction::FocusStart(minutes) => {
                    self.state.borrow_mut().focus.start(minutes, now_secs(), &mut NoHooks);
                    self.notify("menew-focus", &tr("toast_focus_started", lang, &[&minutes.to_string()]), None);
                    self.state_changed();
                }
                ItemAction::FocusStop => {
                    if self.state.borrow_mut().focus.stop(&mut NoHooks) {
                        self.notify("menew-focus", &tr("focus_stopped", lang, &[]), None);
                        self.state_changed();
                    }
                }
                ItemAction::FocusExtend => {
                    self.state.borrow_mut().focus.extend(EXTEND_MINUTES, now_secs());
                    self.state_changed();
                }
                ItemAction::RunScript { script, argument } => self.run_script(script, argument),
                ItemAction::OpenScriptsFolder => match script_commands::ensure_folder() {
                    Ok(dir) => self.open_uri(&gio::File::for_path(&dir).uri()),
                    Err(e) => tracing::warn!("Could not create the scripts folder: {}", e),
                },
                ItemAction::FillQuery(q) => {
                    self.search_entry.set_text(&q);
                    self.search_entry.set_position(-1);
                    self.search_entry.grab_focus();
                }
                ItemAction::OpenClipboard => {
                    self.show_mode(PaletteMode::Clipboard);
                }
                ItemAction::ClearClipboard => match self.db.clear_unpinned() {
                    Ok(n) => info!("Palette: cleared {} unpinned clipboard entries", n),
                    Err(e) => tracing::warn!("Palette: failed to clear clipboard history: {}", e),
                },
                ItemAction::ToggleAutostart => {
                    let want = !menew_core::autostart::is_enabled();
                    match menew_core::autostart::set_enabled(want) {
                        Ok(()) => info!("Palette: Start at login set to {}", want),
                        Err(e) => tracing::warn!("Palette: could not change autostart: {}", e),
                    }
                }
                ItemAction::GenerateUuid => self.copied(&dev_tools::uuid_v4()),
                ItemAction::CopyUnixTimestamp => self.copied(&dev_tools::now_secs().to_string()),
                ItemAction::Quit => self.app.quit(),
            }
        }

        fn run_script(self: &Rc<Self>, script: Script, argument: Option<String>) {
            let lang = current_language();
            if script.argument.is_some() && argument.as_deref().unwrap_or("").is_empty() {
                self.notify("menew-script", &tr("script_need_arg", lang, &[]), None);
                return;
            }
            if script.mode != Mode::Silent {
                self.notify("menew-script", &tr("script_running", lang, &[&script.title]), None);
            }
            let this = Rc::clone(self);
            glib::spawn_future_local(async move {
                let s = script.clone();
                let result = gio::spawn_blocking(move || script_commands::execute(&s, argument.as_deref())).await;
                let Ok(r) = result else { return };
                let last = r.output.lines().rev().find(|l| !l.trim().is_empty()).map(str::to_string);
                if r.status != 0 {
                    let why = r
                        .error
                        .lines()
                        .rev()
                        .find(|l| !l.trim().is_empty())
                        .map(str::to_string)
                        .or(last)
                        .unwrap_or_else(|| tr("script_code", lang, &[&r.status.to_string()]));
                    this.notify("menew-script", &format!("{}: {}", script.title, why), None);
                    return;
                }
                match script.mode {
                    Mode::Silent => this.app.withdraw_notification("menew-script"),
                    Mode::Compact | Mode::Inline => {
                        let msg = last.unwrap_or_else(|| tr("script_done", lang, &[&script.title]));
                        this.notify("menew-script", &msg, None);
                    }
                    Mode::FullOutput => {
                        this.app.withdraw_notification("menew-script");
                        this.show_output(&script.title, &r.output);
                    }
                }
            });
        }

        /// The full output of a script in a small dialog, with a Copy button.
        fn show_output(self: &Rc<Self>, title: &str, text: &str) {
            let lang = current_language();
            let dialog = adw::MessageDialog::new(gtk4::Window::NONE, Some(title), None);
            let view = gtk4::TextView::builder().editable(false).monospace(true).wrap_mode(gtk4::WrapMode::WordChar).build();
            view.buffer().set_text(text);
            let scroll = gtk4::ScrolledWindow::builder().min_content_height(200).min_content_width(420).child(&view).build();
            dialog.set_extra_child(Some(&scroll));
            dialog.add_response("copy", &tr("copy", lang, &[]));
            dialog.add_response("ok", &tr("ok", lang, &[]));
            dialog.set_default_response(Some("ok"));
            dialog.set_close_response("ok");
            let this = Rc::clone(self);
            let body = text.to_string();
            dialog.connect_response(None, move |_, response| {
                if response == "copy" {
                    this.copy_to_clipboard(&body);
                }
            });
            dialog.present();
        }

        // ---- clipboard mode actions ----

        fn activate_clipboard_index(self: &Rc<Self>, index: usize) {
            let text = {
                let entries = self.visible_entries.borrow();
                entries.get(index).map(|e| e.text_content.clone())
            };
            if let Some(text) = text {
                self.commit_clipboard_text(&text);
            }
        }

        /// Copy to the clipboard, hide the palette, then paste into the previous app (uinput)
        /// or fall back to a notification.
        fn commit_clipboard_text(self: &Rc<Self>, text: &str) {
            self.copy_to_clipboard(text);
            self.hide();

            let mode = menew_core::Config::load().paste_mode;
            let action = decide_paste(mode, menew_core::paste::uinput_writable());
            info!("Clipboard entry selected: paste mode {:?} -> {:?}", mode, action);
            match action {
                PasteAction::NotifyOnly => self.notify_copied(),
                PasteAction::InjectCtrlV => {
                    let this = Rc::clone(self);
                    glib::spawn_future_local(async move {
                        // wait for focus to return to the previous window
                        glib::timeout_future(Duration::from_millis(menew_core::paste::FOCUS_DELAY_MS)).await;
                        let result = gio::spawn_blocking(menew_core::paste::inject_ctrl_v).await;
                        match result {
                            Ok(Ok(())) => info!("Injected Ctrl+V through uinput"),
                            other => {
                                tracing::warn!("uinput paste failed ({:?}); falling back to notification", other);
                                this.notify_copied();
                            }
                        }
                    });
                }
            }
        }

        fn notify_copied(&self) {
            let n = gio::Notification::new(t("copied_title"));
            n.set_body(Some(t("copied_body")));
            self.app.send_notification(Some("menew-copied"), &n);
            info!("Notification sent: {}", t("copied_body"));
        }

        fn pin_selected(self: &Rc<Self>) {
            let Some(idx) = self.selected_index() else { return };
            let id = self.visible_entries.borrow().get(idx).map(|e| e.id);
            if let Some(id) = id {
                if let Err(e) = self.db.toggle_pinned(id) {
                    tracing::error!("Failed to toggle pin for entry {}: {}", id, e);
                    return;
                }
                let q = self.search_entry.text().to_string();
                self.refresh_list_selecting(&q, Some(Keep::EntryId(id)));
            }
        }

        fn delete_selected(self: &Rc<Self>) {
            let Some(idx) = self.selected_index() else { return };
            let id = self.visible_entries.borrow().get(idx).map(|e| e.id);
            if let Some(id) = id {
                if let Err(e) = self.db.delete_entry(id) {
                    tracing::error!("Failed to delete entry {}: {}", id, e);
                    return;
                }
                let q = self.search_entry.text().to_string();
                self.refresh_list_selecting(&q, Some(Keep::Index(idx)));
            }
        }

        /// Test hooks (`MENEW_TEST=1 menew __select|pin|delete-clipboard <index>`): simulate Enter,
        /// Ctrl+P or Ctrl+Delete on the `index`-th (0-based) entry of the unfiltered clipboard list.
        /// Returns false if there is no such entry.
        pub fn test_clipboard_op(self: &Rc<Self>, op: TestOp, index: usize) -> bool {
            self.set_mode(PaletteMode::Clipboard);
            let exists = index < self.visible_entries.borrow().len();
            if exists {
                if let Some(row) = self.list_box.row_at_index(index as i32) {
                    self.list_box.select_row(Some(&row));
                }
                match op {
                    TestOp::Select => self.activate_clipboard_index(index),
                    TestOp::Pin => self.pin_selected(),
                    TestOp::Delete => self.delete_selected(),
                }
            }
            exists
        }

        /// Test hook (`MENEW_TEST=1 menew __palette-query "<text>"`): open the palette in Apps mode,
        /// type `text` in the search box and log the results (kind, title, subtitle).
        pub fn test_query(self: &Rc<Self>, text: &str) {
            self.show_mode(PaletteMode::Apps);
            self.search_entry.set_text(text);
            self.refresh_list(text);
            let lang = current_language();
            let items = self.items.borrow();
            info!("Test hook: palette query {:?} -> {} results", text, items.len());
            for (i, it) in items.iter().enumerate().take(12) {
                info!(
                    "Test hook: result[{}] [{}] {} | {} | {}",
                    i,
                    it.kind.label(lang),
                    it.title,
                    it.subtitle.clone().unwrap_or_default(),
                    it.action.describe()
                );
            }
        }

        /// Test hook (`MENEW_TEST=1 menew __palette-activate <index>`): press Enter on that result.
        pub fn test_activate(self: &Rc<Self>, index: usize) -> bool {
            let exists = index < self.items.borrow().len();
            if exists {
                self.activate_index(index);
            }
            exists
        }

        pub fn copy_to_clipboard(&self, text: &str) {
            if let Some(display) = gdk::Display::default() {
                let clipboard = display.clipboard();
                clipboard.set_text(text);
                info!("Text successfully copied to GDK clipboard");
            }
        }
    }

    fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }
}

#[cfg(not(target_os = "linux"))]
pub mod palette {
    use super::*;

    pub struct PaletteWindow;

    impl PaletteWindow {
        pub fn show(&self) {
            info!("PaletteWindow stub: show requested");
        }

        pub fn hide(&self) {
            info!("PaletteWindow stub: hide requested");
        }

        pub fn toggle(&self) {
            info!("PaletteWindow stub: toggle requested");
        }

        pub fn copy_to_clipboard(&self, _text: &str) {}
    }
}

pub use palette::PaletteWindow;
#[cfg(target_os = "linux")]
pub use palette::TestOp;
