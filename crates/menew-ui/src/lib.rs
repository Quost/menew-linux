use menew_core::focus::FocusTimer;
use menew_core::paste_queue::PasteQueue;
use menew_core::quick_links::{self, QuickLink};
use menew_core::recents::Recents;
use menew_core::snippets::SnippetStore;
use menew_core::time_zones::Zone;
use menew_core::timers::TimerStore;
use menew_core::Config;
use std::cell::RefCell;
use std::rc::Rc;
use tracing::info;

/// State of the palette that outlives one search: recent picks, running timers, the focus session,
/// the snippets, the paste queue and what comes from `config.json`. Shared with the daemon, which
/// ticks the timers and feeds the queue.
pub struct PaletteState {
    pub recents: Recents,
    pub timers: TimerStore,
    pub focus: FocusTimer,
    pub links: Vec<QuickLink>,
    pub clocks: Vec<Zone>,
    pub snippets: SnippetStore,
    pub queue: PasteQueue,
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
            snippets: SnippetStore::load_default(menew_core::current_language()),
            queue: PasteQueue::default(),
        }
    }
}

pub type SharedState = Rc<RefCell<PaletteState>>;

/// What the palette asks of the daemon about the clipboard capture.
#[derive(Clone)]
pub struct ClipHooks {
    /// Pause the capture for that many minutes, or until resumed (`None`).
    pub pause: Rc<dyn Fn(Option<u32>)>,
    pub resume: Rc<dyn Fn()>,
    pub is_paused: Rc<dyn Fn() -> bool>,
    /// Menew is about to write the clipboard: `(key, skip)`, see `ClipboardService::expect_own_write`.
    pub expect_own: Rc<dyn Fn(String, bool)>,
    /// Read the text of the images that were not read yet.
    pub queue_ocr: Rc<dyn Fn()>,
    /// "Check for Updates…": look for a new version, or show the one already found.
    pub check_updates: Rc<dyn Fn()>,
}

impl ClipHooks {
    /// Hooks that do nothing (the capture is not running).
    pub fn inert() -> Self {
        Self {
            pause: Rc::new(|_| {}),
            resume: Rc::new(|| {}),
            is_paused: Rc::new(|| false),
            expect_own: Rc::new(|_, _| {}),
            queue_ocr: Rc::new(|| {}),
            check_updates: Rc::new(|| {}),
        }
    }
}

#[cfg(target_os = "linux")]
pub mod snapshot;
#[cfg(target_os = "linux")]
pub mod notes_view;
#[cfg(target_os = "linux")]
pub mod screen;

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
    use menew_core::clip_item::{self, collapsed_preview, row_title, text_stats, ClipFilter};
    use menew_core::clip_policy::{self, own_key, PauseState};
    use menew_core::clip_smart::{self, SmartRun};
    use menew_core::focus::{NoHooks, EXTEND_MINUTES};
    use menew_core::paste_queue::QueueEntry;
    use menew_core::snippets::{self, Snippet};
    use menew_core::{ClipKind, Config};
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
    use std::collections::HashMap;
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
        .menew-chip {
            padding: 2px 8px;
            min-height: 0;
            font-size: 0.85em;
            border-radius: 999px;
        }
        .menew-queue {
            background-color: alpha(@accent_bg_color, 0.15);
            border-radius: 10px;
            padding: 4px 8px;
        }
        .menew-preview {
            background-color: alpha(currentColor, 0.05);
            border-radius: 10px;
            padding: 8px;
        }
        .menew-thumb {
            border-radius: 6px;
        }
        .menew-paused {
            color: @warning_color;
        }
    ";

    const LIST_LIMIT: usize = 100;

    /// What a click in the actions menu does.
    type ActionFn = Box<dyn Fn(&Rc<PaletteWindow>)>;

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
        hooks: ClipHooks,
        /// Country of the locale, for the WhatsApp action.
        region: Option<String>,
        // ---- clipboard mode ----
        filter: Cell<ClipFilter>,
        /// The row of filters and the buttons next to it (only in clipboard mode).
        chips_box: gtk4::Box,
        chips: Vec<(ClipFilter, gtk4::ToggleButton)>,
        queue_btn: gtk4::ToggleButton,
        pause_btn: gtk4::MenuButton,
        new_snippet_btn: gtk4::Button,
        queue_bar: gtk4::Box,
        queue_text: gtk4::Label,
        scrolled: gtk4::ScrolledWindow,
        preview: gtk4::Box,
        visible_snippets: Rc<RefCell<Vec<Snippet>>>,
        /// The password notes of the Passwords filter (only their titles are ever in them).
        visible_passwords: Rc<RefCell<Vec<menew_core::notes::Note>>>,
        notes: Rc<crate::notes_view::NotesApp>,
        screen: Rc<crate::screen::ScreenApp>,
        /// Set while the widgets are changed by code, so their handlers do not run.
        syncing: Cell<bool>,
        thumbs: RefCell<HashMap<String, gdk::Texture>>,
        /// Demo mode (`--snapshot`): never reads the machine's apps, scripts or clipboard.
        demo: bool,
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

    /// Snapshot mode never unlocks anything.
    struct NoAuth;
    impl menew_core::notes::Authenticator for NoAuth {
        fn authenticate(&self) -> menew_core::notes::Auth {
            menew_core::notes::Auth::Denied
        }
    }

    impl PaletteWindow {
        pub fn create(app: &adw::Application, db: ClipboardDatabase, state: SharedState, on_state: Rc<dyn Fn()>, hooks: ClipHooks, notes: Rc<crate::notes_view::NotesApp>, screen: Rc<crate::screen::ScreenApp>) -> Rc<Self> {
            Self::build(app, db, state, on_state, hooks, DesktopScanner::scan_all(), false, notes, screen)
        }

        /// Palette for `menew --snapshot`: fixed apps, no scripts folder, no clipboard read, no real data.
        /// Its notes live in memory only (no keyring, nothing read from disk).
        pub fn create_demo(app: &adw::Application, db: ClipboardDatabase, state: SharedState, apps: Vec<DesktopApp>) -> Rc<Self> {
            let store = menew_core::notes::NotesStore::in_memory(Rc::new(menew_core::notes::MemoryVault::default()));
            let notes = crate::notes_view::NotesApp::new(app, store, std::sync::Arc::new(NoAuth));
            let screen = crate::screen::ScreenApp::inert(app);
            Self::build(app, db, state, Rc::new(|| {}), ClipHooks::inert(), apps, true, notes, screen)
        }

        #[allow(clippy::too_many_arguments)]
        fn build(app: &adw::Application, db: ClipboardDatabase, state: SharedState, on_state: Rc<dyn Fn()>, hooks: ClipHooks, apps: Vec<DesktopApp>, demo: bool, notes: Rc<crate::notes_view::NotesApp>, screen: Rc<crate::screen::ScreenApp>) -> Rc<Self> {
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

            // clipboard mode: the filters, the paste queue and pause buttons
            let lang = current_language();
            let chips_box = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Horizontal)
                .spacing(3)
                .margin_start(12)
                .margin_end(12)
                .margin_bottom(6)
                .visible(false)
                .build();
            let mut chips: Vec<(ClipFilter, gtk4::ToggleButton)> = vec![];
            for f in ClipFilter::ALL {
                let b = gtk4::ToggleButton::builder().label(f.label(lang)).css_classes(["flat", "menew-chip"]).build();
                if let Some((_, first)) = chips.first() {
                    b.set_group(Some(first));
                }
                chips_box.append(&b);
                chips.push((f, b));
            }
            chips[0].1.set_active(true);
            let new_snippet_btn = gtk4::Button::builder()
                .icon_name("list-add-symbolic")
                .tooltip_text(tr("snip_new", lang, &[]))
                .css_classes(["flat", "circular"])
                .valign(gtk4::Align::Center)
                .visible(false)
                .build();
            let spacer = gtk4::Box::builder().hexpand(true).build();
            let queue_btn = gtk4::ToggleButton::builder()
                .icon_name("view-list-ordered-symbolic")
                .tooltip_text(tr("queue_toggle_help", lang, &[]))
                .css_classes(["flat", "circular"])
                .valign(gtk4::Align::Center)
                .build();
            let pause_btn = gtk4::MenuButton::builder()
                .icon_name("media-playback-pause-symbolic")
                .tooltip_text(tr("clip_pause_help", lang, &[]))
                .css_classes(["flat", "circular"])
                .valign(gtk4::Align::Center)
                .build();
            chips_box.append(&spacer);
            chips_box.append(&new_snippet_btn);
            chips_box.append(&queue_btn);
            chips_box.append(&pause_btn);
            main_box.append(&chips_box);

            // the paste queue banner
            let queue_bar = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Horizontal)
                .spacing(8)
                .margin_start(12)
                .margin_end(12)
                .margin_bottom(6)
                .css_classes(["menew-queue"])
                .visible(false)
                .build();
            queue_bar.append(&gtk4::Image::from_icon_name("view-list-ordered-symbolic"));
            let queue_text = gtk4::Label::builder().halign(gtk4::Align::Start).hexpand(true).ellipsize(gtk4::pango::EllipsizeMode::End).build();
            queue_bar.append(&queue_text);
            let queue_stop = gtk4::Button::builder().label(tr("queue_stop", lang, &[])).tooltip_text(tr("queue_stop_help", lang, &[])).css_classes(["flat"]).build();
            queue_bar.append(&queue_stop);
            main_box.append(&queue_bar);

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

            // clipboard mode: what the selected entry holds
            let preview = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Vertical)
                .spacing(4)
                .margin_start(8)
                .margin_end(8)
                .margin_top(6)
                .height_request(116)
                .css_classes(["menew-preview"])
                .visible(false)
                .build();
            main_box.append(&preview);

            // Footer: current mode on the left, key hints on the right
            let footer = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Horizontal)
                .spacing(12)
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
                .ellipsize(gtk4::pango::EllipsizeMode::End)
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
                apps: Rc::new(RefCell::new(apps)),
                demo,
                items,
                visible_entries: Rc::new(RefCell::new(Vec::new())),
                clip_text: RefCell::new(None),
                scripts: RefCell::new((None, Vec::new())),
                query_empty,
                last_query: RefCell::new(String::new()),
                hooks,
                region: clip_smart::region_from_env(),
                filter: Cell::new(ClipFilter::All),
                chips_box,
                chips,
                queue_btn,
                pause_btn,
                new_snippet_btn,
                queue_bar,
                queue_text,
                scrolled,
                preview,
                visible_snippets: Rc::new(RefCell::new(Vec::new())),
                visible_passwords: Rc::new(RefCell::new(Vec::new())),
                notes,
                screen,
                syncing: Cell::new(false),
                thumbs: RefCell::new(HashMap::new()),
            });

            {
                let this = Rc::clone(&palette);
                queue_stop.connect_clicked(move |_| this.stop_queue());
            }
            palette.build_pause_menu();
            palette.init_events();
            palette.apply_mode_chrome();
            palette.refresh_list("");
            {
                // headless test sessions: the clipboard is set while a window of Menew is mapped
                let weak = Rc::downgrade(&palette);
                palette.screen.set_map_hook(Rc::new(move || {
                    if let Some(p) = weak.upgrade() {
                        p.test_mode_map_window();
                    }
                }));
            }

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
                let alt = state.contains(gdk::ModifierType::ALT_MASK);
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
                    gdk::Key::Return | gdk::Key::KP_Enter if alt && in_clipboard && self_key.filter.get() == ClipFilter::Passwords => {
                        if let Some(i) = self_key.selected_index() {
                            self_key.paste_password(i, true);
                        }
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
                    // the actions of the selected entry (right click works too)
                    gdk::Key::k | gdk::Key::K if ctrl && in_clipboard => {
                        self_key.show_actions();
                        glib::Propagation::Stop
                    }
                    // the filters
                    gdk::Key::Right if alt && in_clipboard => {
                        self_key.set_filter(self_key.filter.get().next());
                        glib::Propagation::Stop
                    }
                    gdk::Key::Left if alt && in_clipboard => {
                        self_key.set_filter(self_key.filter.get().prev());
                        glib::Propagation::Stop
                    }
                    gdk::Key::n | gdk::Key::N if ctrl && in_clipboard && self_key.filter.get() == ClipFilter::Snippets => {
                        self_key.edit_snippet(None);
                        glib::Propagation::Stop
                    }
                    gdk::Key::e | gdk::Key::E if ctrl && in_clipboard => {
                        self_key.edit_selected_snippet();
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

            // the preview follows the selection
            let self_sel = Rc::clone(self);
            self.list_box.connect_row_selected(move |_, _| {
                if self_sel.mode.get() == PaletteMode::Clipboard {
                    self_sel.update_preview();
                }
            });

            // right click: the actions of the entry under the pointer
            let click = gtk4::GestureClick::new();
            click.set_button(3);
            let self_click = Rc::clone(self);
            click.connect_pressed(move |_, _, _, y| {
                if self_click.mode.get() != PaletteMode::Clipboard {
                    return;
                }
                if let Some(row) = self_click.list_box.row_at_y(y as i32) {
                    self_click.list_box.select_row(Some(&row));
                    self_click.show_actions();
                }
            });
            self.list_box.add_controller(click);

            // the filters
            for (f, button) in &self.chips {
                let this = Rc::clone(self);
                let f = *f;
                button.connect_toggled(move |b| {
                    if b.is_active() && !this.syncing.get() {
                        this.set_filter(f);
                    }
                });
            }
            let this = Rc::clone(self);
            self.queue_btn.connect_toggled(move |b| {
                if !this.syncing.get() && b.is_active() != this.state.borrow().queue.is_on() {
                    this.toggle_queue();
                }
            });
            let this = Rc::clone(self);
            self.new_snippet_btn.connect_clicked(move |_| {
                if this.filter.get() == ClipFilter::Passwords {
                    this.notes.new_passwords();
                } else {
                    this.edit_snippet(None);
                }
            });
        }

        pub fn mode(&self) -> PaletteMode {
            self.mode.get()
        }

        fn apply_mode_chrome(&self) {
            let clip = self.mode.get() == PaletteMode::Clipboard;
            self.chips_box.set_visible(clip);
            self.preview.set_visible(clip);
            self.scrolled.set_min_content_height(if clip { 170 } else { 340 });
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
            self.refresh_clip_chrome();
        }

        /// Pause indicator, queue button and banner, and the "new snippet" button.
        fn refresh_clip_chrome(&self) {
            let lang = current_language();
            let clip = self.mode.get() == PaletteMode::Clipboard;
            self.syncing.set(true);
            let paused = (self.hooks.is_paused)();
            if paused {
                self.pause_btn.add_css_class("menew-paused");
                self.pause_btn.set_icon_name("media-playback-pause-symbolic");
                let st = PauseState::load();
                let label = if st.paused || st.paused_until <= 0.0 {
                    tr("clip_paused", lang, &[])
                } else {
                    let local = chrono::DateTime::from_timestamp(st.paused_until as i64, 0)
                        .map(|d| d.with_timezone(&chrono::Local).naive_local())
                        .map(|d| menew_core::format::format_time(chrono::Timelike::hour(&d), chrono::Timelike::minute(&d), lang))
                        .unwrap_or_default();
                    tr("clip_paused_until", lang, &[&local])
                };
                self.pause_btn.set_tooltip_text(Some(&label));
                if clip {
                    self.mode_label.set_label(&format!("{} \u{b7} {}", t("mode_clipboard"), label));
                }
            } else {
                self.pause_btn.remove_css_class("menew-paused");
                self.pause_btn.set_tooltip_text(Some(&tr("clip_pause_help", lang, &[])));
            }
            let (on, len, next) = {
                let st = self.state.borrow();
                (st.queue.is_on(), st.queue.len(), st.queue.head().map(|e| e.preview.clone()))
            };
            self.queue_btn.set_active(on);
            self.queue_bar.set_visible(clip && on);
            let line = match next {
                Some(p) => format!("{}  \u{b7}  {}", tr("queue_count", lang, &[&len.to_string()]), tr("queue_next", lang, &[&p])),
                None => format!("{}  \u{b7}  {}", tr("queue_title", lang, &[]), tr("queue_hint", lang, &[])),
            };
            self.queue_text.set_label(&line);
            let pw = self.filter.get() == ClipFilter::Passwords;
            self.new_snippet_btn.set_visible(clip && (pw || self.filter.get() == ClipFilter::Snippets));
            self.new_snippet_btn.set_tooltip_text(Some(&tr(if pw { "notes_new_passwords" } else { "snip_new" }, lang, &[])));
            if clip {
                let hint = if pw { tr("clip_hint_passwords", lang, &[]) } else { t("clipboard_hint").to_string() };
                self.hint_label.set_label(&hint);
                self.search_entry.set_placeholder_text(Some(&if pw { tr("clip_search_passwords", lang, &[]) } else { t("clipboard_placeholder").to_string() }));
            }
            self.syncing.set(false);
        }

        /// Pick a filter chip (and show its entries).
        pub fn set_filter(self: &Rc<Self>, f: ClipFilter) {
            self.filter.set(f);
            self.sync_chips();
            self.refresh_clip_chrome();
            let q = self.search_entry.text().to_string();
            self.refresh_list(&q);
        }

        fn sync_chips(&self) {
            self.syncing.set(true);
            for (f, b) in &self.chips {
                b.set_active(*f == self.filter.get());
            }
            self.syncing.set(false);
        }

        /// The pause menu next to the filters.
        fn build_pause_menu(self: &Rc<Self>) {
            let lang = current_language();
            let popover = gtk4::Popover::new();
            let list = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(2).margin_top(4).margin_bottom(4).margin_start(4).margin_end(4).build();
            let entries: Vec<(String, Option<Option<u32>>)> = vec![
                (tr("clip_pause_15", lang, &[]), Some(Some(15))),
                (tr("clip_pause_60", lang, &[]), Some(Some(60))),
                (tr("clip_pause_until", lang, &[]), Some(None)),
                (tr("clip_resume", lang, &[]), None),
            ];
            for (label, minutes) in entries {
                let b = gtk4::Button::builder().label(&label).css_classes(["flat"]).build();
                if let Some(lbl) = b.child().and_then(|c| c.downcast::<gtk4::Label>().ok()) {
                    lbl.set_halign(gtk4::Align::Start);
                }
                let this = Rc::clone(self);
                let pop = popover.clone();
                b.connect_clicked(move |_| {
                    pop.popdown();
                    match minutes {
                        Some(m) => this.pause_clipboard(m),
                        None => this.resume_clipboard(),
                    }
                });
                list.append(&b);
            }
            popover.set_child(Some(&list));
            self.pause_btn.set_popover(Some(&popover));
        }

        fn pause_clipboard(self: &Rc<Self>, minutes: Option<u32>) {
            let lang = current_language();
            (self.hooks.pause)(minutes);
            let text = match minutes {
                Some(60) => tr("toast_paused_1h", lang, &[]),
                Some(15) => tr("clip_pause_15", lang, &[]),
                Some(_) => tr("clip_paused", lang, &[]),
                None => tr("clip_pause_until", lang, &[]),
            };
            self.notify("menew-clip-pause", &text, None);
            self.refresh_clip_chrome();
            self.state_changed();
        }

        fn resume_clipboard(self: &Rc<Self>) {
            (self.hooks.resume)();
            self.notify("menew-clip-pause", &tr("toast_resumed", current_language(), &[]), None);
            self.refresh_clip_chrome();
            self.state_changed();
        }

        /// Switch mode and reset the query.
        pub fn set_mode(self: &Rc<Self>, mode: PaletteMode) {
            self.mode.set(mode);
            self.filter.set(ClipFilter::All);
            self.sync_chips();
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
            self.visible_snippets.borrow_mut().clear();
            self.visible_passwords.borrow_mut().clear();
            self.query_empty.set(query.trim().is_empty());
            *self.last_query.borrow_mut() = query.to_string();

            match self.mode.get() {
                PaletteMode::Apps => self.fill_apps(query),
                PaletteMode::Clipboard => self.fill_clipboard(query.trim()),
            }

            let total = self.row_count() as i32;
            if total == 0 {
                let text = if self.mode.get() == PaletteMode::Clipboard && self.filter.get() == ClipFilter::Snippets && query.trim().is_empty() {
                    tr("snip_none", current_language(), &[])
                } else if self.mode.get() == PaletteMode::Clipboard && self.filter.get() == ClipFilter::Passwords && query.trim().is_empty() {
                    tr("clip_none_passwords", current_language(), &[])
                } else if self.mode.get() == PaletteMode::Clipboard && query.trim().is_empty() && self.filter.get() == ClipFilter::All {
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
                if self.mode.get() == PaletteMode::Clipboard {
                    self.update_preview();
                }
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
            if self.mode.get() == PaletteMode::Clipboard {
                self.update_preview();
            }
        }

        /// How many rows the current list has.
        fn row_count(&self) -> usize {
            match self.mode.get() {
                PaletteMode::Apps => self.items.borrow().len(),
                PaletteMode::Clipboard => self.visible_entries.borrow().len() + self.visible_snippets.borrow().len() + self.visible_passwords.borrow().len(),
            }
        }

        /// Scripts of the folder, re-read at most every 3 seconds (the palette asks on every key).
        fn scripts(&self) -> Vec<Script> {
            let mut cache = self.scripts.borrow_mut();
            if self.demo {
                return Vec::new();
            }
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
                self.db.get_newest_text().ok().flatten().map(|e| e.text_content)
            });
            text.filter(|t| !t.is_empty())
        }

        fn fill_apps(self: &Rc<Self>, raw: &str) {
            let lang = current_language();
            let scripts = self.scripts();
            let clip_text = self.clipboard_text();
            let paused = (self.hooks.is_paused)();
            let items = {
                let state = self.state.borrow();
                let notes_store = self.notes.store.borrow();
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
                    clip: menew_core::palette::ClipContext {
                        region: self.region.as_deref(),
                        snippets: &state.snippets.snippets,
                        paused,
                        queue_on: state.queue.is_on(),
                        queue_len: state.queue.len(),
                    },
                    notes: &notes_store.notes,
                };
                let db = self.db.clone();
                let clip_search = move |q: &str| -> Vec<ClipHit> {
                    db.search(q, 6)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|e| ClipHit {
                            id: e.id,
                            title: if e.kind == ClipKind::Text { preview(&e.text_content, PREVIEW_MAX_CHARS) } else { row_title(&e, lang) },
                            kind: e.kind,
                            text: e.text_content,
                        })
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
            let lang = current_language();
            let filter = self.filter.get();

            // the password notes: only their titles, the content stays in the keyring
            if filter == ClipFilter::Passwords {
                let found: Vec<menew_core::notes::Note> = {
                    let store = self.notes.store.borrow();
                    menew_core::notes::password_notes(&store.notes, clean_query, lang).into_iter().cloned().collect()
                };
                for n in &found {
                    let row = adw::ActionRow::builder()
                        .title(glib::markup_escape_text(&n.title(lang)))
                        .subtitle("\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}")
                        .title_lines(1)
                        .subtitle_lines(1)
                        .icon_name("dialog-password-symbolic")
                        .build();
                    self.list_box.append(&row);
                }
                *self.visible_passwords.borrow_mut() = found;
                return;
            }

            // the snippets are not history entries: their own list
            if filter == ClipFilter::Snippets {
                let found: Vec<Snippet> = self.state.borrow().snippets.matching(clean_query).into_iter().cloned().collect();
                for s in &found {
                    let row = adw::ActionRow::builder()
                        .title(&s.title)
                        .subtitle(collapsed_preview(&s.text))
                        .title_lines(1)
                        .subtitle_lines(1)
                        .use_markup(false)
                        .icon_name("insert-text-symbolic")
                        .build();
                    self.list_box.append(&row);
                }
                *self.visible_snippets.borrow_mut() = found;
                return;
            }

            let mut entries = self.db.filtered(filter, clean_query, LIST_LIMIT).unwrap_or_default();
            sort_for_display(&mut entries);

            let now = now_ms();
            for entry in &entries {
                let first_line = row_title(entry, lang);
                let title = if first_line.is_empty() { "...".to_string() } else { first_line.chars().take(PREVIEW_MAX_CHARS).collect() };
                let age = format_age(age_between(now, entry.created_at));
                // "Pinned · Firefox · 2 min", like the line under a row on the Mac
                let mut parts: Vec<String> = vec![];
                if entry.is_pinned {
                    parts.push(t("pinned").to_string());
                }
                if let Some(app) = &entry.source_app {
                    parts.push(app.clone());
                }
                parts.push(age);
                let subtitle = parts.join(" \u{00b7} ");

                let row = adw::ActionRow::builder().title(&title).subtitle(&subtitle).title_lines(1).use_markup(false).build();
                row.add_prefix(&self.thumb(entry));
                if entry.is_pinned {
                    row.add_suffix(&gtk4::Image::from_icon_name("view-pin-symbolic"));
                }
                self.list_box.append(&row);
            }
            *self.visible_entries.borrow_mut() = entries;
        }

        /// The small picture at the start of a row: the image itself, the icon of the file, the color, or an icon.
        fn thumb(&self, e: &ClipboardEntry) -> gtk4::Widget {
            match e.kind {
                ClipKind::Image => {
                    if let Some(tex) = self.db.image_path(e).and_then(|p| self.thumbnail(&p, 72, 72)) {
                        let pic = gtk4::Picture::new();
                        pic.set_paintable(Some(&tex));
                        pic.set_size_request(36, 36);
                        pic.set_can_shrink(true);
                        pic.add_css_class("menew-thumb");
                        pic.set_overflow(gtk4::Overflow::Hidden);
                        return pic.upcast();
                    }
                    gtk4::Image::from_icon_name("image-x-generic-symbolic").upcast()
                }
                ClipKind::File => {
                    let first = clip_item::file_paths(e).first().map(|p| p.to_string()).unwrap_or_default();
                    let (ctype, _) = gio::content_type_guess(Some(first.as_str()), &[]);
                    let img = if std::path::Path::new(&first).is_dir() {
                        gtk4::Image::from_icon_name("folder")
                    } else {
                        gtk4::Image::from_gicon(&gio::content_type_get_icon(&ctype))
                    };
                    img.set_pixel_size(28);
                    img.upcast()
                }
                ClipKind::Text => {
                    if let Some(rgb) = clip_item::hex_color(e) {
                        swatch(rgb, 28).upcast()
                    } else if clip_item::url(e).is_some() {
                        gtk4::Image::from_icon_name("web-browser-symbolic").upcast()
                    } else {
                        gtk4::Image::from_icon_name("edit-paste-symbolic").upcast()
                    }
                }
            }
        }

        /// An image file scaled to fit `w` x `h`, kept for the next time the list is built.
        fn thumbnail(&self, path: &std::path::Path, w: i32, h: i32) -> Option<gdk::Texture> {
            let key = format!("{}:{w}x{h}", path.display());
            if let Some(t) = self.thumbs.borrow().get(&key) {
                return Some(t.clone());
            }
            let pixbuf = gtk4::gdk_pixbuf::Pixbuf::from_file_at_scale(path, w, h, true).ok()?;
            let tex = gdk::Texture::for_pixbuf(&pixbuf);
            let mut cache = self.thumbs.borrow_mut();
            if cache.len() > 200 {
                cache.clear();
            }
            cache.insert(key, tex.clone());
            Some(tex)
        }

        /// What the selected entry holds, below the list (the preview pane of the Mac).
        fn update_preview(&self) {
            while let Some(child) = self.preview.first_child() {
                self.preview.remove(&child);
            }
            let lang = current_language();
            let label = |text: &str, dim: bool, lines: i32| {
                let l = gtk4::Label::builder().label(text).halign(gtk4::Align::Start).xalign(0.0).wrap(true).wrap_mode(gtk4::pango::WrapMode::WordChar).lines(lines).ellipsize(gtk4::pango::EllipsizeMode::End).build();
                if dim {
                    l.add_css_class("dim-label");
                    l.add_css_class("caption");
                }
                l
            };
            let Some(idx) = self.selected_index() else { return };
            let n_entries = self.visible_entries.borrow().len();
            if self.filter.get() == ClipFilter::Passwords {
                if idx < self.visible_passwords.borrow().len() {
                    self.preview.append(&label(&tr("pw_row_hint", lang, &[]), true, 2));
                }
                return;
            }
            if idx >= n_entries {
                let snip = self.visible_snippets.borrow().get(idx - n_entries).cloned();
                if let Some(s) = snip {
                    self.preview.append(&label(&s.text, false, 5));
                }
                return;
            }
            let Some(e) = self.visible_entries.borrow().get(idx).cloned() else { return };
            let when = chrono::DateTime::from_timestamp_millis(e.created_at)
                .map(|d| d.with_timezone(&chrono::Local).format("%d/%m %H:%M").to_string())
                .unwrap_or_default();
            let source = e.source_app.clone().unwrap_or_else(|| tr("clip_unknown", lang, &[]));
            let details = format!("{}: {}  \u{b7}  {}: {}", tr("clip_source", lang, &[]), source, tr("clip_copied_at", lang, &[]), when);
            match e.kind {
                ClipKind::Image => {
                    let tex = self.db.image_path(&e).and_then(|p| self.thumbnail(&p, 480, 160));
                    match tex {
                        Some(tex) => {
                            let pic = gtk4::Picture::new();
                            pic.set_paintable(Some(&tex));
                            pic.set_can_shrink(true);
                            pic.set_height_request(70);
                            pic.set_vexpand(true);
                            self.preview.append(&pic);
                        }
                        None => self.preview.append(&label(&tr("clip_image_missing", lang, &[]), false, 2)),
                    }
                    self.preview.append(&label(&format!("{}  \u{b7}  {}", row_title(&e, lang), details), true, 1));
                    if let Some(ocr) = e.ocr_text.as_deref().filter(|t| !t.is_empty()) {
                        self.preview.append(&label(&collapsed_preview(ocr), true, 1));
                    }
                }
                ClipKind::File => {
                    let paths = clip_item::file_paths(&e);
                    let shown: Vec<&str> = paths.iter().take(4).copied().collect();
                    self.preview.append(&label(&shown.join("\n"), false, 4));
                    self.preview.append(&label(&format!("{}  \u{b7}  {}", clip_item::files_count_text(paths.len(), lang), details), true, 1));
                }
                ClipKind::Text => {
                    if let Some(rgb) = clip_item::hex_color(&e) {
                        let row = gtk4::Box::builder().orientation(gtk4::Orientation::Horizontal).spacing(10).build();
                        row.append(&swatch(rgb, 56));
                        let hex = e.text_content.trim().trim_start_matches('#').to_uppercase();
                        row.append(&label(&format!("#{hex}\nRGB {}, {}, {}", rgb.0, rgb.1, rgb.2), false, 2));
                        self.preview.append(&row);
                    } else if let Some(u) = clip_item::url(&e) {
                        self.preview.append(&label(&clip_smart::host_of(&u).unwrap_or_default(), false, 1));
                        self.preview.append(&label(&u, true, 2));
                    } else {
                        let shown: String = e.text_content.lines().take(5).collect::<Vec<_>>().join("\n").chars().take(400).collect();
                        self.preview.append(&label(&shown, false, 5));
                    }
                    self.preview.append(&label(&format!("{}  \u{b7}  {}", text_stats(&e.text_content, lang), details), true, 1));
                }
            }
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
            if mode == PaletteMode::Apps && !self.demo {
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
            if formats.mime_types().iter().any(|m| clip_policy::is_secret_type(m)) {
                return;
            }
            let this = Rc::clone(self);
            clipboard.read_text_async(gio::Cancellable::NONE, move |res| {
                if let Ok(Some(text)) = res {
                    *this.clip_text.borrow_mut() = Some(text.to_string());
                    let q = this.search_entry.text().to_string();
                    if this.window.is_visible() && this.mode.get() == PaletteMode::Apps {
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
            if !test_mode() {
                self.hide();
            }
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
                ItemAction::PasteEntry(id) => {
                    if let Ok(Some(e)) = self.db.get(id) {
                        self.paste_entry(&e, true);
                    }
                }
                ItemAction::PasteSnippet(id) => self.paste_snippet(&id, true),
                ItemAction::OpenNote(id) => self.notes.open(&id),
                ItemAction::NewNote => {
                    self.notes.new_note();
                }
                ItemAction::NewPasswordsNote => {
                    self.notes.new_passwords();
                }
                ItemAction::PauseClipboard(m) => self.pause_clipboard(m),
                ItemAction::ResumeClipboard => self.resume_clipboard(),
                ItemAction::ToggleQueue => self.toggle_queue(),
                ItemAction::PasteNext => self.paste_next(),
                ItemAction::Capture => self.screen.capture(crate::screen::Purpose::Edit, true),
                ItemAction::CaptureFull => self.screen.capture(crate::screen::Purpose::Edit, false),
                ItemAction::CaptureText => self.screen.capture(crate::screen::Purpose::Text, true),
                ItemAction::QrScan => self.screen.capture(crate::screen::Purpose::Qr, true),
                ItemAction::QrShow(text) => self.screen.show_qr(text, false),
                ItemAction::QrWifi => self.screen.show_qr(None, true),
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
                ItemAction::CheckUpdates => (self.hooks.check_updates)(),
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
            let entry = self.visible_entries.borrow().get(index).cloned();
            if let Some(e) = entry {
                self.paste_entry(&e, true);
                return;
            }
            let n = self.visible_entries.borrow().len();
            let snippet = self.visible_snippets.borrow().get(index.wrapping_sub(n)).cloned();
            if let Some(s) = snippet {
                self.paste_snippet(&s.id, true);
            } else if self.filter.get() == ClipFilter::Passwords {
                self.paste_password(index, false);
            }
        }

        /// Copy to the clipboard, hide the palette, then paste into the previous app (uinput)
        /// or fall back to a notification.
        fn commit_clipboard_text(self: &Rc<Self>, text: &str) {
            (self.hooks.expect_own)(own_key(ClipKind::Text, text), false);
            self.copy_to_clipboard(text);
            self.finish_paste(0, menew_core::paste::FOCUS_DELAY_MS);
        }

        /// The palette goes away and, when the system allows it, Ctrl+V is pressed for the user in the app
        /// that had the focus (and the cursor goes back `back` characters, for the `{cursor}` of a snippet).
        /// Otherwise a notification says to press Ctrl+V.
        fn finish_paste(self: &Rc<Self>, back: usize, delay_ms: u64) {
            if !test_mode() {
                self.hide();
            }

            let mode = menew_core::Config::load().paste_mode;
            let action = decide_paste(mode, menew_core::paste::uinput_writable());
            info!("Clipboard entry selected: paste mode {:?} -> {:?}", mode, action);
            match action {
                PasteAction::NotifyOnly => self.notify_copied(),
                PasteAction::InjectCtrlV => {
                    let this = Rc::clone(self);
                    glib::spawn_future_local(async move {
                        // wait for focus to return to the previous window
                        glib::timeout_future(Duration::from_millis(delay_ms)).await;
                        let result = gio::spawn_blocking(menew_core::paste::inject_ctrl_v).await;
                        match result {
                            Ok(Ok(())) => info!("Injected Ctrl+V through uinput"),
                            other => {
                                tracing::warn!("uinput paste failed ({:?}); falling back to notification", other);
                                this.notify_copied();
                                return;
                            }
                        }
                        if back > 0 {
                            glib::timeout_future(Duration::from_millis(100)).await;
                            let _ = gio::spawn_blocking(move || menew_core::paste::inject_left(back)).await;
                            info!("Moved the cursor back {} characters (snippet {{cursor}})", back);
                        }
                    });
                }
            }
        }

        /// Put an entry of the history on the clipboard in its own type: the text, the image file as it is, the files as
        /// addresses (and the type the file managers of GNOME ask for).
        fn write_entry_to_clipboard(&self, e: &ClipboardEntry, skip_own: bool) -> bool {
            self.test_mode_map_window();
            let Some(display) = gdk::Display::default() else { return false };
            let clipboard = display.clipboard();
            match e.kind {
                ClipKind::Text => {
                    (self.hooks.expect_own)(own_key(ClipKind::Text, &e.text_content), skip_own);
                    clipboard.set_text(&e.text_content);
                    true
                }
                ClipKind::Image => {
                    let Some(path) = self.db.image_path(e) else { return false };
                    let Ok(data) = std::fs::read(&path) else {
                        tracing::warn!("The image file {} of entry {} is gone", path.display(), e.id);
                        return false;
                    };
                    let mime = clip_policy::image_mime_of(e.image_file.as_deref().unwrap_or("x.png"));
                    (self.hooks.expect_own)(own_key(ClipKind::Image, e.image_file.as_deref().unwrap_or("")), skip_own);
                    let provider = gdk::ContentProvider::for_bytes(mime, &glib::Bytes::from_owned(data));
                    clipboard.set_content(Some(&provider)).is_ok()
                }
                ClipKind::File => {
                    let paths = clip_item::file_paths(e);
                    let uris: Vec<String> = paths.iter().map(|p| clip_policy::path_to_uri(p)).collect();
                    (self.hooks.expect_own)(own_key(ClipKind::File, &e.text_content), skip_own);
                    let list = glib::Bytes::from_owned(format!("{}\r\n", uris.join("\r\n")).into_bytes());
                    let gnome = glib::Bytes::from_owned(format!("copy\n{}", uris.join("\n")).into_bytes());
                    let plain = glib::Bytes::from_owned(paths.join("\n").into_bytes());
                    let provider = gdk::ContentProvider::new_union(&[
                        gdk::ContentProvider::for_bytes("text/uri-list", &list),
                        gdk::ContentProvider::for_bytes("x-special/gnome-copied-files", &gnome),
                        gdk::ContentProvider::for_bytes("text/plain;charset=utf-8", &plain),
                    ]);
                    clipboard.set_content(Some(&provider)).is_ok()
                }
            }
        }

        /// Pick an entry: it goes on the clipboard (moving to the top of the history), and is pasted
        /// into the previous app when `paste` is true.
        fn paste_entry(self: &Rc<Self>, e: &ClipboardEntry, paste: bool) {
            if !self.write_entry_to_clipboard(e, false) {
                return;
            }
            let _ = self.db.touch(e.id);
            info!("Clipboard entry {} ({}) put on the clipboard", e.id, e.kind.as_str());
            if paste {
                self.finish_paste(0, menew_core::paste::FOCUS_DELAY_MS);
            } else {
                self.hide();
                self.notify("menew-copied", &tr("copied_value", current_language(), &[&row_title(e, current_language())]), None);
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
            let n = self.visible_entries.borrow().len();
            if idx >= n {
                let id = self.visible_snippets.borrow().get(idx - n).map(|s| s.id.clone());
                if let Some(id) = id {
                    self.state.borrow_mut().snippets.delete(&id);
                    let q = self.search_entry.text().to_string();
                    self.refresh_list_selecting(&q, Some(Keep::Index(idx)));
                }
                return;
            }
            let id = self.visible_entries.borrow().get(idx).map(|e| e.id);
            if let Some(id) = id {
                if let Err(e) = self.db.delete_entry(id) {
                    tracing::error!("Failed to delete entry {}: {}", id, e);
                    return;
                }
                // gone from the history, gone from the queue
                let removed = self.state.borrow_mut().queue.remove(id);
                if removed {
                    self.queue_changed();
                }
                let q = self.search_entry.text().to_string();
                self.refresh_list_selecting(&q, Some(Keep::Index(idx)));
            }
        }

        // ---- the actions menu of an entry (Ctrl+K, right click) ----

        fn act(label: String, f: impl Fn(&Rc<PaletteWindow>) + 'static) -> (String, ActionFn) {
            (label, Box::new(f))
        }

        fn actions_for(self: &Rc<Self>, idx: usize) -> Vec<(String, ActionFn)> {
            let lang = current_language();
            let n = self.visible_entries.borrow().len();
            let mut out: Vec<(String, ActionFn)> = vec![];
            if idx >= n {
                let Some(s) = self.visible_snippets.borrow().get(idx - n).cloned() else { return out };
                let id = s.id.clone();
                out.push(Self::act(tr("clip_paste", lang, &[]), move |p| p.paste_snippet(&id, true)));
                let id = s.id.clone();
                out.push(Self::act(tr("clip_copy_only", lang, &[]), move |p| p.paste_snippet(&id, false)));
                let snip = s.clone();
                out.push(Self::act(tr("clip_edit", lang, &[]), move |p| p.edit_snippet(Some(snip.clone()))));
                let id = s.id.clone();
                out.push(Self::act(tr("clip_delete", lang, &[]), move |p| {
                    p.state.borrow_mut().snippets.delete(&id);
                    let q = p.search_entry.text().to_string();
                    p.refresh_list(&q);
                }));
                return out;
            }
            let Some(e) = self.visible_entries.borrow().get(idx).cloned() else { return out };
            let ec = e.clone();
            out.push(Self::act(tr("clip_paste", lang, &[]), move |p| p.paste_entry(&ec, true)));
            let ec = e.clone();
            out.push(Self::act(tr("clip_copy_only", lang, &[]), move |p| p.paste_entry(&ec, false)));
            let ec = e.clone();
            out.push(Self::act(tr("clip_queue_add", lang, &[]), move |p| p.queue_add(&ec)));
            let id = e.id;
            out.push(Self::act(tr(if e.is_pinned { "clip_unpin" } else { "clip_pin" }, lang, &[]), move |p| {
                let _ = p.db.toggle_pinned(id);
                let q = p.search_entry.text().to_string();
                p.refresh_list_selecting(&q, Some(Keep::EntryId(id)));
            }));
            match e.kind {
                ClipKind::Text => {
                    // what the Mac offers for the copied text: call, maps, mail, track, color...; the plain link is the Open link below
                    for a in clip_smart::actions(&e.text_content, lang, self.region.as_deref()).into_iter().filter(|a| a.id != "link") {
                        let run = a.run.clone();
                        out.push(Self::act(a.title, move |p| p.run_smart(&run)));
                    }
                    if let Some(u) = clip_item::url(&e) {
                        out.push(Self::act(tr("clip_open_link", lang, &[]), move |p| {
                            p.hide();
                            p.open_uri(&u)
                        }));
                    }
                }
                ClipKind::File => {
                    if let Some(first) = clip_item::file_paths(&e).first().map(|s| s.to_string()) {
                        out.push(Self::act(tr("clip_show_folder", lang, &[]), move |p| {
                            let parent = std::path::Path::new(&first).parent().map(|d| d.to_path_buf()).unwrap_or_default();
                            p.hide();
                            p.open_uri(&gio::File::for_path(parent).uri());
                        }));
                    }
                }
                ClipKind::Image => {
                    let ec = e.clone();
                    out.push(Self::act(tr("clip_copy_image_text", lang, &[]), move |p| p.copy_image_text(&ec)));
                    if let Some(path) = self.db.image_path(&e) {
                        out.push(Self::act(tr("clip_open_image", lang, &[]), move |p| {
                            p.hide();
                            p.open_uri(&gio::File::for_path(&path).uri());
                        }));
                    }
                }
            }
            let id = e.id;
            out.push(Self::act(tr("clip_delete", lang, &[]), move |p| {
                let _ = p.db.delete_entry(id);
                if p.state.borrow_mut().queue.remove(id) {
                    p.queue_changed();
                }
                let q = p.search_entry.text().to_string();
                p.refresh_list(&q);
            }));
            out
        }

        /// The actions menu of the selected entry.
        fn show_actions(self: &Rc<Self>) {
            let Some(idx) = self.selected_index() else { return };
            let actions = self.actions_for(idx);
            let Some(row) = self.list_box.row_at_index(idx as i32) else { return };
            if actions.is_empty() {
                return;
            }
            let popover = gtk4::Popover::new();
            popover.set_parent(&row);
            popover.set_position(gtk4::PositionType::Bottom);
            let list = gtk4::ListBox::builder().selection_mode(gtk4::SelectionMode::Single).css_classes(["navigation-sidebar"]).build();
            for (label, _) in &actions {
                list.append(&gtk4::Label::builder().label(label).halign(gtk4::Align::Start).margin_top(6).margin_bottom(6).margin_start(8).margin_end(8).build());
            }
            let actions = Rc::new(actions);
            {
                let this = Rc::clone(self);
                let pop = popover.clone();
                let actions = Rc::clone(&actions);
                list.connect_row_activated(move |_, r| {
                    pop.popdown();
                    if let Some((label, f)) = actions.get(r.index() as usize) {
                        info!("Palette: clipboard action '{}'", label);
                        f(&this);
                    }
                });
            }
            popover.set_child(Some(&list));
            popover.connect_closed(|p| {
                let p = p.clone();
                glib::idle_add_local_once(move || p.unparent());
            });
            popover.popup();
            if let Some(first) = list.row_at_index(0) {
                list.select_row(Some(&first));
                first.grab_focus();
            }
        }

        fn run_smart(self: &Rc<Self>, run: &SmartRun) {
            match run {
                SmartRun::Open(uri) => {
                    self.hide();
                    self.open_uri(uri);
                }
                SmartRun::Copy(text) => {
                    self.hide();
                    self.copied(text);
                }
                SmartRun::ShowQr(text) => {
                    self.hide();
                    self.screen.show_qr(Some(text.clone()), false);
                }
            }
        }

        // ---- text of images ----

        /// "Copy text from image": the text read in the background, or read now.
        fn copy_image_text(self: &Rc<Self>, e: &ClipboardEntry) {
            let lang = current_language();
            let id = e.id;
            let known = e.ocr_text.clone().filter(|t| !t.is_empty());
            if let Some(text) = known {
                self.hide();
                self.copy_ocr_text(&text);
                return;
            }
            let Some(path) = self.db.image_path(e) else { return };
            self.hide();
            self.notify("menew-ocr", &tr("ocr_reading", lang, &[]), None);
            let this = Rc::clone(self);
            glib::spawn_future_local(async move {
                let result = gio::spawn_blocking(move || menew_core::ocr::recognize(&path)).await;
                match result {
                    Ok(Ok(text)) if !text.is_empty() => {
                        let _ = this.db.set_ocr_text(id, &text);
                        this.copy_ocr_text(&text);
                    }
                    Ok(Ok(_)) => {
                        let _ = this.db.set_ocr_text(id, "");
                        this.notify("menew-ocr", &tr("ocr_none", lang, &[]), None);
                    }
                    Ok(Err(menew_core::ocr::OcrError::Missing)) => this.notify("menew-ocr", &tr("ocr_missing", lang, &[]), None),
                    _ => this.notify("menew-ocr", &tr("ocr_none", lang, &[]), None),
                }
            });
        }

        fn copy_ocr_text(&self, text: &str) {
            let lang = current_language();
            self.copy_to_clipboard(text);
            let lines = text.lines().count();
            let msg = if lines <= 1 { tr("ocr_copied", lang, &[]) } else { tr("ocr_copied_lines", lang, &[&lines.to_string()]) };
            self.notify("menew-ocr", &msg, None);
        }

        // ---- the paste queue ----

        fn queue_entry(e: &ClipboardEntry) -> QueueEntry {
            let preview = if e.kind == ClipKind::Text { collapsed_preview(&e.text_content) } else { row_title(e, current_language()) };
            QueueEntry { id: e.id, preview }
        }

        /// The queue changed: the banner, the button and the tray.
        pub fn queue_changed(&self) {
            self.refresh_clip_chrome();
            self.state_changed();
        }

        pub fn toggle_queue(self: &Rc<Self>) {
            let lang = current_language();
            {
                let mut st = self.state.borrow_mut();
                st.queue.toggle();
            }
            let on = self.state.borrow().queue.is_on();
            info!("Paste queue {}", if on { "on" } else { "off" });
            if on {
                self.notify("menew-queue", &tr("queue_on", lang, &[]), None);
            }
            self.queue_changed();
        }

        fn stop_queue(self: &Rc<Self>) {
            if self.state.borrow_mut().queue.stop() {
                info!("Paste queue off");
            }
            self.queue_changed();
        }

        /// "Add to paste queue" of an entry of the history.
        fn queue_add(self: &Rc<Self>, e: &ClipboardEntry) {
            let n = self.state.borrow_mut().queue.add(Self::queue_entry(e));
            info!("Paste queue: added entry {} ({} in the queue)", e.id, n);
            self.notify("menew-queue", &tr("queue_added", current_language(), &[&n.to_string()]), None);
            self.queue_changed();
        }

        /// Something was copied: it joins the queue when the queue is on, and an open list shows it.
        pub fn on_clip_saved(self: &Rc<Self>, id: i64) {
            let Ok(Some(e)) = self.db.get(id) else { return };
            let joined = self.state.borrow_mut().queue.captured(Self::queue_entry(&e));
            if let Some(n) = joined {
                info!("Paste queue: copied entry {} joined ({} in the queue)", id, n);
                self.notify("menew-queue", &tr("queue_added", current_language(), &[&n.to_string()]), None);
                self.queue_changed();
            }
            if self.window.is_visible() && self.mode.get() == PaletteMode::Clipboard {
                let q = self.search_entry.text().to_string();
                self.refresh_list(&q);
            }
        }

        /// The step of the queue: the first item leaves the queue, goes on the clipboard and is pasted
        /// into the app that has the focus (Ctrl+V pressed for the user when uinput allows it, else the user presses it).
        pub fn paste_next(self: &Rc<Self>) {
            let lang = current_language();
            loop {
                let popped = self.state.borrow_mut().queue.paste_next();
                let Some(p) = popped else {
                    info!("Paste queue: nothing to paste");
                    self.notify("menew-queue", &tr("queue_empty_toast", lang, &[]), None);
                    self.queue_changed();
                    return;
                };
                // deleted from the history in the meantime: take the next one
                let Ok(Some(entry)) = self.db.get(p.entry.id) else { continue };
                if !self.write_entry_to_clipboard(&entry, true) {
                    continue;
                }
                info!("Paste queue: next item {} '{}' ({} left)", entry.id, p.entry.preview, p.remaining);
                if p.remaining == 0 {
                    self.notify("menew-queue", &tr("queue_done", lang, &[]), None);
                }
                self.queue_changed();
                let mode = Config::load().paste_mode;
                match decide_paste(mode, menew_core::paste::uinput_writable()) {
                    PasteAction::NotifyOnly => self.notify("menew-queue-next", &tr("queue_ready", lang, &[]), None),
                    PasteAction::InjectCtrlV => {
                        let this = Rc::clone(self);
                        glib::spawn_future_local(async move {
                            let result = gio::spawn_blocking(menew_core::paste::inject_ctrl_v).await;
                            if !matches!(result, Ok(Ok(()))) {
                                this.notify("menew-queue-next", &tr("queue_ready", lang, &[]), None);
                            }
                        });
                    }
                }
                return;
            }
        }

        // ---- snippets ----

        /// Paste a snippet (or only copy it): ask its `{input:}` fields, fill in the variables, put it
        /// on the clipboard without keeping it in the history, and move the cursor to `{cursor}`.
        pub fn notes(&self) -> &Rc<crate::notes_view::NotesApp> {
            &self.notes
        }

        /// The screen: capture, annotation editor, text from the screen and QR codes.
        pub fn screen(&self) -> &Rc<crate::screen::ScreenApp> {
            &self.screen
        }

        /// The password (or username) of the `index`-th row of the Passwords filter: asks the user, puts it on the
        /// clipboard as a confidential value (cleared after 30 s) and pastes it into the previous app.
        fn paste_password(self: &Rc<Self>, index: usize, username: bool) {
            let id = self.visible_passwords.borrow().get(index).map(|n| n.id.clone());
            let Some(id) = id else { return };
            let this = Rc::clone(self);
            self.notes.copy_password_field(&id, username, move |res| {
                if res == Some(true) {
                    this.finish_paste(0, menew_core::paste::FOCUS_DELAY_MS);
                }
            });
        }

        /// Test hook (`MENEW_TEST=1 menew __password-paste <index> <password|username>`): Enter or Alt+Enter on the
        /// `index`-th row of the Passwords filter (shown first).
        pub fn test_password_paste(self: &Rc<Self>, index: usize, username: bool) -> bool {
            self.show_mode(PaletteMode::Clipboard);
            self.set_filter(ClipFilter::Passwords);
            let exists = index < self.visible_passwords.borrow().len();
            if exists {
                self.paste_password(index, username);
            }
            exists
        }

        fn paste_snippet(self: &Rc<Self>, id: &str, paste: bool) {
            let Some(snip) = self.state.borrow().snippets.find(id).cloned() else { return };
            let fields = snippets::fields_in(&snip.text);
            let this = Rc::clone(self);
            let asked = !fields.is_empty();
            let (title, raw) = (snip.title.clone(), snip.text.clone());
            let title_go = title.clone();
            let go = move |text: String| {
                let needs_clip = text.contains("{clipboard}");
                let this2 = Rc::clone(&this);
                let title = title_go.clone();
                let finish = move |clip: String| {
                    let (expanded, back) = snippets::expand_with_cursor(&text, &snippets::local_now(), &clip, current_language());
                    (this2.hooks.expect_own)(own_key(ClipKind::Text, &expanded), true);
                    this2.copy_to_clipboard(&expanded);
                    info!("Snippet '{}' expanded ({} characters, cursor back {})", title, expanded.chars().count(), back);
                    if paste {
                        this2.finish_paste(back, if asked { 350 } else { menew_core::paste::FOCUS_DELAY_MS });
                    } else {
                        this2.hide();
                        this2.notify("menew-copied", &tr("copied_value", current_language(), &[&title]), None);
                    }
                };
                if needs_clip {
                    // the clipboard as it is now
                    let Some(display) = gdk::Display::default() else { return finish(String::new()) };
                    display.clipboard().read_text_async(gio::Cancellable::NONE, move |res| {
                        finish(res.ok().flatten().map(|t| t.to_string()).unwrap_or_default());
                    });
                } else {
                    finish(String::new());
                }
            };
            if asked {
                self.ask_fields(&title, fields, raw, go);
            } else {
                go(raw);
            }
        }

        /// A small window with one entry per `{input:Name}`; `done` gets the text filled in (nothing when cancelled).
        fn ask_fields(self: &Rc<Self>, title: &str, fields: Vec<String>, text: String, done: impl Fn(String) + 'static) {
            let lang = current_language();
            let heading = tr("snip_fill", lang, &[]);
            let dialog = adw::MessageDialog::new(gtk4::Window::NONE, Some(heading.as_str()), Some(title));
            let grid = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(6).build();
            let mut entries: Vec<(String, gtk4::Entry)> = vec![];
            for f in &fields {
                grid.append(&gtk4::Label::builder().label(f).halign(gtk4::Align::Start).build());
                let e = gtk4::Entry::builder().activates_default(true).width_chars(30).build();
                grid.append(&e);
                entries.push((f.clone(), e));
            }
            dialog.set_extra_child(Some(&grid));
            dialog.add_response("cancel", &tr("snip_cancel", lang, &[]));
            dialog.add_response("paste", &tr("clip_paste", lang, &[]));
            dialog.set_response_appearance("paste", adw::ResponseAppearance::Suggested);
            dialog.set_default_response(Some("paste"));
            dialog.set_close_response("cancel");
            dialog.connect_response(None, move |_, response| {
                if response == "paste" {
                    let values: Vec<(String, String)> = entries.iter().map(|(n, e)| (n.clone(), e.text().to_string())).collect();
                    done(snippets::apply(&text, &values));
                }
            });
            dialog.present();
            if let Some(first) = grid.first_child().and_then(|l| l.next_sibling()) {
                first.grab_focus();
            }
        }

        fn edit_selected_snippet(self: &Rc<Self>) {
            let Some(idx) = self.selected_index() else { return };
            let n = self.visible_entries.borrow().len();
            if idx >= n {
                let s = self.visible_snippets.borrow().get(idx - n).cloned();
                self.edit_snippet(s);
            }
        }

        /// Create or edit a snippet in a small window (title and text, with the list of variables).
        fn edit_snippet(self: &Rc<Self>, existing: Option<Snippet>) {
            let lang = current_language();
            let is_new = existing.is_none();
            let heading = tr(if is_new { "snip_new" } else { "snip_edit" }, lang, &[]);
            let dialog = adw::MessageDialog::new(gtk4::Window::NONE, Some(heading.as_str()), None);
            let body = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(8).build();
            let title = gtk4::Entry::builder().placeholder_text(tr("snip_title_field", lang, &[])).text(existing.as_ref().map(|s| s.title.as_str()).unwrap_or("")).width_chars(40).build();
            body.append(&title);
            let view = gtk4::TextView::builder().wrap_mode(gtk4::WrapMode::WordChar).top_margin(6).bottom_margin(6).left_margin(6).right_margin(6).build();
            view.buffer().set_text(existing.as_ref().map(|s| s.text.as_str()).unwrap_or(""));
            let scroll = gtk4::ScrolledWindow::builder().min_content_height(120).child(&view).css_classes(["card"]).build();
            body.append(&scroll);
            body.append(&gtk4::Label::builder().label(tr("snip_help", lang, &[])).wrap(true).xalign(0.0).css_classes(["dim-label", "caption"]).build());
            dialog.set_extra_child(Some(&body));
            dialog.add_response("cancel", &tr("snip_cancel", lang, &[]));
            dialog.add_response("save", &tr("snip_save", lang, &[]));
            dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
            dialog.set_close_response("cancel");
            let this = Rc::clone(self);
            dialog.connect_response(None, move |_, response| {
                if response != "save" {
                    return;
                }
                let buf = view.buffer();
                let text = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();
                if text.trim().is_empty() {
                    return;
                }
                let mut name = title.text().to_string();
                if name.trim().is_empty() {
                    name = text.lines().next().unwrap_or("Snippet").to_string();
                }
                let mut s = existing.clone().unwrap_or_else(|| Snippet::new("", ""));
                s.title = name;
                s.text = text;
                this.state.borrow_mut().snippets.upsert(s);
                let q = this.search_entry.text().to_string();
                this.refresh_list(&q);
            });
            dialog.present();
        }

        /// Test hook (`MENEW_TEST=1 menew __clipboard-view <filter> [query]`): show the clipboard list with that
        /// filter and search and log what it holds.
        pub fn test_clipboard_view(self: &Rc<Self>, filter: &str, query: &str) {
            self.show_mode(PaletteMode::Clipboard);
            self.search_entry.set_text(query);
            self.set_filter(ClipFilter::from_id(filter).unwrap_or(ClipFilter::All));
            self.refresh_list(query);
            let lang = current_language();
            let entries = self.visible_entries.borrow().clone();
            let snippets = self.visible_snippets.borrow().clone();
            let passwords = self.visible_passwords.borrow().clone();
            info!("Test hook: clipboard view filter={} query={:?} -> {} entries", self.filter.get().id(), query, entries.len() + snippets.len() + passwords.len());
            for (i, e) in entries.iter().enumerate().take(20) {
                info!(
                    "Test hook: clip[{}] [{}] {} | pinned={} | source={} | ocr={}",
                    i,
                    e.kind.as_str(),
                    row_title(e, lang),
                    e.is_pinned,
                    e.source_app.clone().unwrap_or_default(),
                    e.ocr_text.as_deref().map(|t| t.chars().take(40).collect::<String>().replace('\n', " ")).unwrap_or_else(|| "(not read)".into())
                );
            }
            for (i, s) in snippets.iter().enumerate().take(20) {
                info!("Test hook: snippet[{}] {} | {}", i, s.title, collapsed_preview(&s.text));
            }
            for (i, n) in passwords.iter().enumerate().take(20) {
                info!("Test hook: password[{}] {}", i, n.title(lang));
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

        /// `--snapshot`: show the Apps list for `text`, with `clip` as the copied text (never the real clipboard).
        pub fn demo_apps(self: &Rc<Self>, text: &str, clip: Option<&str>) {
            self.show_mode(PaletteMode::Apps);
            *self.clip_text.borrow_mut() = clip.map(str::to_string);
            self.search_entry.set_text(text);
            self.refresh_list(text);
            if !self.items.borrow().is_empty() {
                if let Some(row) = self.list_box.row_at_index(0) {
                    self.list_box.select_row(Some(&row));
                }
            }
        }

        /// `--snapshot`: show the clipboard list with that filter and the first row selected.
        pub fn demo_clipboard(self: &Rc<Self>, filter: ClipFilter, query: &str) {
            self.show_mode(PaletteMode::Clipboard);
            self.search_entry.set_text(query);
            self.set_filter(filter);
            self.refresh_list(query);
            if let Some(row) = self.list_box.row_at_index(0) {
                self.list_box.select_row(Some(&row));
            }
        }

        /// `--snapshot`: draw the palette card (with a soft shadow and a transparent margin) to a PNG.
        pub fn capture_png(&self, path: &std::path::Path) -> Result<(), String> {
            use gtk4::{graphene, gsk};
            let child = self.window.content().ok_or("no content")?;
            let (w, h) = (child.width() as f32, child.height() as f32);
            if w < 1.0 || h < 1.0 {
                return Err("the palette has no size yet".into());
            }
            let margin = 32.0_f32;
            let snap = gtk4::Snapshot::new();
            let card = gsk::RoundedRect::from_rect(graphene::Rect::new(margin, margin, w, h), 14.0);
            snap.append_outset_shadow(&card, &gdk::RGBA::new(0.0, 0.0, 0.0, 0.30), 0.0, 8.0, 0.0, 24.0);
            snap.translate(&graphene::Point::new(margin, margin));
            gtk4::WidgetPaintable::new(Some(&child)).snapshot(&snap, w as f64, h as f64);
            let node = snap.to_node().ok_or("nothing was drawn")?;
            let renderer = self.window.native().and_then(|n| n.renderer()).ok_or("no renderer")?;
            let texture = renderer.render_texture(&node, Some(&graphene::Rect::new(0.0, 0.0, w + 2.0 * margin, h + 2.0 * margin)));
            texture.save_to_png(path).map_err(|e| e.to_string())
        }

        /// Headless test sessions only let a client with a mapped window set the selection: with `MENEW_TEST=1`
        /// the window is shown first. Real desktops are not asked for this (see docs/parity.md).
        fn test_mode_map_window(&self) {
            if std::env::var("MENEW_TEST").map(|v| v == "1").unwrap_or(false) && !self.window.is_visible() {
                self.window.set_visible(true);
                self.window.present();
                info!("Test mode: window shown so the headless compositor accepts the selection");
            }
        }

        pub fn copy_to_clipboard(&self, text: &str) {
            self.test_mode_map_window();
            if let Some(display) = gdk::Display::default() {
                let clipboard = display.clipboard();
                clipboard.set_text(text);
                info!("Text successfully copied to GDK clipboard");
            }
        }
    }

    /// A rounded square of one color (the swatch of a copied `#6C5CE7`).
    fn swatch(rgb: (u8, u8, u8), size: i32) -> gtk4::DrawingArea {
        let area = gtk4::DrawingArea::builder().content_width(size).content_height(size).valign(gtk4::Align::Center).build();
        area.set_draw_func(move |_, cr, w, h| {
            use std::f64::consts::{FRAC_PI_2, PI};
            let (w, h, r) = (w as f64, h as f64, 6.0);
            cr.set_source_rgb(rgb.0 as f64 / 255.0, rgb.1 as f64 / 255.0, rgb.2 as f64 / 255.0);
            cr.new_sub_path();
            cr.arc(w - r, r, r, -FRAC_PI_2, 0.0);
            cr.arc(w - r, h - r, r, 0.0, FRAC_PI_2);
            cr.arc(r, h - r, r, FRAC_PI_2, PI);
            cr.arc(r, r, r, PI, 3.0 * FRAC_PI_2);
            cr.close_path();
            let _ = cr.fill();
        });
        area
    }

    /// `MENEW_TEST=1`: the headless sessions only let a client with a mapped window set the selection,
    /// so the palette stays open after a pick.
    fn test_mode() -> bool {
        std::env::var("MENEW_TEST").map(|v| v == "1").unwrap_or(false)
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
