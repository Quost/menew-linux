use tracing::info;

#[cfg(target_os = "linux")]
pub mod palette {
    use super::*;
    use gtk4::gdk;
    use gtk4::glib;
    use gtk4::prelude::*;
    use libadwaita as adw;
    use libadwaita::prelude::*;
    use menew_core::{
        evaluate_expression, format_result, t, DesktopApp, DesktopScanner,
    };
    use std::cell::RefCell;
    use std::rc::Rc;

    pub struct PaletteWindow {
        window: adw::ApplicationWindow,
        search_entry: gtk4::SearchEntry,
        list_box: gtk4::ListBox,
        apps: Rc<RefCell<Vec<DesktopApp>>>,
        visible_apps: Rc<RefCell<Vec<Option<DesktopApp>>>>,
        last_calc_result: Rc<RefCell<Option<String>>>,
    }

    impl PaletteWindow {
        pub fn create(app: &adw::Application) -> Rc<Self> {
            let window = adw::ApplicationWindow::builder()
                .application(app)
                .title("Menew")
                .default_width(640)
                .default_height(440)
                .resizable(false)
                .modal(false)
                .decorated(false)
                .build();

            // Style palette container
            window.add_css_class("menew-palette");

            let main_box = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Vertical)
                .spacing(0)
                .margin_top(8)
                .margin_bottom(8)
                .margin_start(8)
                .margin_end(8)
                .build();

            // Search entry
            let search_entry = gtk4::SearchEntry::builder()
                .placeholder_text(t("search_placeholder"))
                .hexpand(true)
                .margin_top(4)
                .margin_bottom(8)
                .margin_start(4)
                .margin_end(4)
                .build();

            main_box.append(&search_entry);

            // Separator
            let separator = gtk4::Separator::new(gtk4::Orientation::Horizontal);
            main_box.append(&separator);

            // Scrolled window with ListBox
            let scrolled = gtk4::ScrolledWindow::builder()
                .hscrollbar_policy(gtk4::PolicyType::Never)
                .vscrollbar_policy(gtk4::PolicyType::Automatic)
                .vexpand(true)
                .hexpand(true)
                .min_content_height(340)
                .build();

            let list_box = gtk4::ListBox::builder()
                .selection_mode(gtk4::SelectionMode::Single)
                .css_classes(["boxed-list"])
                .build();

            scrolled.set_child(Some(&list_box));
            main_box.append(&scrolled);

            window.set_content(Some(&main_box));

            // Load initial applications
            let apps = Rc::new(RefCell::new(DesktopScanner::scan_all()));
            let visible_apps = Rc::new(RefCell::new(Vec::new()));
            let last_calc_result = Rc::new(RefCell::new(None));

            let palette = Rc::new(Self {
                window,
                search_entry,
                list_box,
                apps,
                visible_apps,
                last_calc_result,
            });

            palette.init_events();
            palette.refresh_list("");

            palette
        }

        fn init_events(self: &Rc<Self>) {
            let self_search = Rc::clone(self);
            self.search_entry.connect_search_changed(move |entry| {
                let query = entry.text().to_string();
                self_search.refresh_list(&query);
            });

            // Handle entry-specific events
            let self_stop = Rc::clone(self);
            self.search_entry.connect_stop_search(move |_| {
                self_stop.hide();
            });

            let self_act = Rc::clone(self);
            self.search_entry.connect_activate(move |_| {
                self_act.activate_selected();
            });

            // Keyboard navigation using PropagationPhase::Capture to intercept before child widgets
            let key_controller = gtk4::EventControllerKey::new();
            key_controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
            let self_key = Rc::clone(self);
            key_controller.connect_key_pressed(move |_, keyval, _, _| {
                match keyval {
                    gdk::Key::Escape => {
                        self_key.hide();
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
                    _ => glib::Propagation::Proceed,
                }
            });
            self.window.add_controller(key_controller);

            // Forward key capture
            self.search_entry.set_key_capture_widget(Some(&self.window));

            // Row activated by mouse click
            let self_row = Rc::clone(self);
            self.list_box.connect_row_activated(move |_, row| {
                let idx = row.index();
                if idx >= 0 {
                    self_row.activate_index(idx as usize);
                }
            });
        }

        pub fn refresh_list(self: &Rc<Self>, query: &str) {
            // Clear current items
            while let Some(child) = self.list_box.first_child() {
                self.list_box.remove(&child);
            }
            self.visible_apps.borrow_mut().clear();

            let clean_query = query.trim();

            // 1. Math evaluation if applicable
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

            // 2. Search applications
            let scored = DesktopScanner::search(&self.apps.borrow(), clean_query);
            let limit = 20.min(scored.len());

            for item in scored.iter().take(limit) {
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

            // If empty
            if self.visible_apps.borrow().is_empty() {
                let empty_label = gtk4::Label::builder()
                    .label(t("no_results"))
                    .margin_top(32)
                    .margin_bottom(32)
                    .css_classes(["dim-label"])
                    .build();
                self.list_box.append(&empty_label);
            } else {
                // Select first row
                if let Some(first_row) = self.list_box.row_at_index(0) {
                    self.list_box.select_row(Some(&first_row));
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

        pub fn show(self: &Rc<Self>) {
            // Reload desktop files to catch newly installed apps
            *self.apps.borrow_mut() = DesktopScanner::scan_all();
            self.search_entry.set_text("");
            self.refresh_list("");
            self.window.set_visible(true);
            self.window.present();
            self.search_entry.grab_focus();
            info!("Palette window presented and focused");
        }

        pub fn hide(self: &Rc<Self>) {
            self.window.set_visible(false);
            self.search_entry.set_text("");
            info!("Palette window hidden");
        }

        fn move_selection(self: &Rc<Self>, delta: i32) {
            let total = self.visible_apps.borrow().len() as i32;
            if total == 0 {
                return;
            }

            let current = self
                .list_box
                .selected_row()
                .map(|r| r.index())
                .unwrap_or(0);

            let next = (current + delta).clamp(0, total - 1);
            if let Some(target_row) = self.list_box.row_at_index(next) {
                self.list_box.select_row(Some(&target_row));
            }
        }

        fn activate_selected(self: &Rc<Self>) {
            if let Some(row) = self.list_box.selected_row() {
                let idx = row.index();
                if idx >= 0 {
                    self.activate_index(idx as usize);
                }
            }
        }

        fn activate_index(self: &Rc<Self>, index: usize) {
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
                        // Calculation result: copy to clipboard
                        if let Some(res) = &*self.last_calc_result.borrow() {
                            info!("Copying calculation result to clipboard: {}", res);
                            self.copy_to_clipboard(res);
                        }
                        self.hide();
                    }
                }
            }
        }

        pub fn copy_to_clipboard(self: &Rc<Self>, text: &str) {
            if let Some(display) = gdk::Display::default() {
                let clipboard = display.clipboard();
                clipboard.set_text(text);
                info!("Text successfully copied to GDK clipboard");
            }
        }
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
