use tracing::info;

#[cfg(target_os = "linux")]
pub mod palette {
    use super::*;
    use gtk4::gdk;
    use gtk4::gio;
    use gtk4::glib;
    use gtk4::prelude::*;
    use libadwaita as adw;
    use libadwaita::prelude::*;
    use menew_core::{
        age_between, decide_paste, evaluate_expression, format_age, format_result, preview,
        sort_for_display, t, ClipboardDatabase, ClipboardEntry, Config, DesktopApp,
        DesktopScanner, PaletteMode, PasteAction, PREVIEW_MAX_CHARS,
    };
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::time::Duration;

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
    ";

    const LIST_LIMIT: usize = 100;

    pub struct PaletteWindow {
        app: adw::Application,
        db: ClipboardDatabase,
        window: adw::ApplicationWindow,
        search_entry: gtk4::SearchEntry,
        list_box: gtk4::ListBox,
        mode_label: gtk4::Label,
        hint_label: gtk4::Label,
        mode: Cell<PaletteMode>,
        apps: Rc<RefCell<Vec<DesktopApp>>>,
        visible_apps: Rc<RefCell<Vec<Option<DesktopApp>>>>,
        visible_entries: Rc<RefCell<Vec<ClipboardEntry>>>,
        last_calc_result: Rc<RefCell<Option<String>>>,
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

    impl PaletteWindow {
        pub fn create(app: &adw::Application, db: ClipboardDatabase) -> Rc<Self> {
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

            let palette = Rc::new(Self {
                app: app.clone(),
                db,
                window,
                search_entry,
                list_box,
                mode_label,
                hint_label,
                mode: Cell::new(PaletteMode::Apps),
                apps: Rc::new(RefCell::new(DesktopScanner::scan_all())),
                visible_apps: Rc::new(RefCell::new(Vec::new())),
                visible_entries: Rc::new(RefCell::new(Vec::new())),
                last_calc_result: Rc::new(RefCell::new(None)),
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
            self.visible_apps.borrow_mut().clear();
            self.visible_entries.borrow_mut().clear();

            match self.mode.get() {
                PaletteMode::Apps => self.fill_apps(query.trim()),
                PaletteMode::Clipboard => self.fill_clipboard(query.trim()),
            }

            let total = match self.mode.get() {
                PaletteMode::Apps => self.visible_apps.borrow().len(),
                PaletteMode::Clipboard => self.visible_entries.borrow().len(),
            } as i32;
            if total == 0 {
                let key = if self.mode.get() == PaletteMode::Clipboard && query.trim().is_empty() {
                    "clipboard_empty"
                } else {
                    "no_results"
                };
                let empty_label = gtk4::Label::builder()
                    .label(t(key))
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

        fn fill_apps(&self, clean_query: &str) {
            if let Some(calc_val) = evaluate_expression(clean_query) {
                let result_str = format_result(calc_val);
                *self.last_calc_result.borrow_mut() = Some(result_str.clone());

                let row = adw::ActionRow::builder()
                    .title(&result_str)
                    .subtitle(t("calc_result"))
                    .icon_name("accessories-calculator")
                    .build();

                self.list_box.append(&row);
                self.visible_apps.borrow_mut().push(None); // None represents calculation
            } else {
                *self.last_calc_result.borrow_mut() = None;
            }

            let scored = DesktopScanner::search(&self.apps.borrow(), clean_query);
            for item in scored.iter().take(20) {
                let app = &item.app;
                let subtitle = app
                    .generic_name
                    .as_deref()
                    .or(app.comment.as_deref())
                    .unwrap_or(&app.exec);

                let icon_name = app.icon.as_deref().unwrap_or("application-x-executable");

                let row = adw::ActionRow::builder()
                    .title(&app.name)
                    .subtitle(subtitle)
                    .icon_name(icon_name)
                    .build();

                self.list_box.append(&row);
                self.visible_apps.borrow_mut().push(Some(app.clone()));
            }
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
            }
            self.set_mode(mode);
            self.window.set_visible(true);
            self.window.present();
            self.search_entry.grab_focus();
            info!("Palette window presented and focused ({:?})", mode);
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
                PaletteMode::Apps => self.visible_apps.borrow().len(),
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

            // Drop borrow before calling self.hide() to prevent RefCell borrow_mut panic
            let item_opt = {
                let visible = self.visible_apps.borrow();
                visible.get(index).cloned()
            };

            if let Some(item_opt) = item_opt {
                match item_opt {
                    Some(app) => {
                        info!("Launching app from palette: {}", app.name);
                        if let Err(e) = app.launch() {
                            tracing::error!("Failed to launch app {}: {}", app.name, e);
                        }
                        self.hide();
                    }
                    None => {
                        let res = self.last_calc_result.borrow().clone();
                        if let Some(res) = res {
                            info!("Copying calculation result to clipboard: {}", res);
                            self.copy_to_clipboard(&res);
                        }
                        self.hide();
                    }
                }
            }
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

            let mode = Config::load().paste_mode;
            let action = decide_paste(mode, menew_core::paste::uinput_writable());
            info!("Clipboard entry selected: paste mode {:?} -> {:?}", mode, action);
            match action {
                PasteAction::NotifyOnly => self.notify_copied(),
                PasteAction::InjectCtrlV => {
                    let this = Rc::clone(self);
                    glib::spawn_future_local(async move {
                        // wait for focus to return to the previous window
                        glib::timeout_future(Duration::from_millis(menew_core::paste::FOCUS_DELAY_MS))
                            .await;
                        let result = gio::spawn_blocking(menew_core::paste::inject_ctrl_v).await;
                        match result {
                            Ok(Ok(())) => info!("Injected Ctrl+V through uinput"),
                            other => {
                                tracing::warn!(
                                    "uinput paste failed ({:?}); falling back to notification",
                                    other
                                );
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

        pub fn copy_to_clipboard(self: &Rc<Self>, text: &str) {
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
