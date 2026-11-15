//! The tray popover (port of the menu bar popover: `NowView.swift`, `ActionsView.swift`,
//! `Favorites.swift`). A StatusNotifierItem menu can't hold custom views, so a left click on the
//! tray icon opens this small window with the same two tabs: Now (a search box for the palette, the
//! computer's health, running timers, world clocks, favorites) and Actions (favorites, then every
//! action in groups).

use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use menew_core::current_language;
use menew_core::strings::tr;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// One action button: command id, title, freedesktop icon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionEntry {
    pub command: String,
    pub title: String,
    pub icon: String,
}

/// What the Now tab shows, filled by the daemon.
#[derive(Debug, Clone, Default)]
pub struct NowData {
    /// "All good" or what needs attention.
    pub status: String,
    /// (label, value) pills: CPU 12%, RAM 45%, Disk 120 GB, Battery 80%.
    pub metrics: Vec<(String, String)>,
    /// Running timers, focus, keep awake, paste queue.
    pub running: Vec<String>,
    /// (city, time)
    pub clocks: Vec<(String, String)>,
}

pub struct Hooks {
    pub run: Rc<dyn Fn(&str)>,
    /// Open the palette with this text.
    pub search: Rc<dyn Fn(&str)>,
    /// The groups of actions: (group title, actions).
    pub groups: Rc<dyn Fn() -> Vec<(String, Vec<ActionEntry>)>>,
    pub favorites: Rc<dyn Fn() -> Vec<String>>,
    pub set_favorites: Rc<dyn Fn(Vec<String>)>,
    pub now: Rc<dyn Fn() -> NowData>,
    pub open_window: Rc<dyn Fn()>,
}

pub struct Popover {
    window: adw::ApplicationWindow,
    hooks: Hooks,
    now_box: gtk4::Box,
    now_favs: gtk4::FlowBox,
    actions_box: gtk4::Box,
    editing: Cell<bool>,
    search: gtk4::SearchEntry,
    stack: adw::ViewStack,
    /// The groups as last shown (to find titles of favorites).
    catalog: RefCell<Vec<ActionEntry>>,
}

fn action_button(a: &ActionEntry) -> gtk4::Button {
    let b = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(4).width_request(96).build();
    b.append(&gtk4::Image::builder().icon_name(&a.icon).pixel_size(22).build());
    b.append(&gtk4::Label::builder().label(&a.title).wrap(true).justify(gtk4::Justification::Center).max_width_chars(14).lines(2).ellipsize(gtk4::pango::EllipsizeMode::End).css_classes(["caption"]).build());
    gtk4::Button::builder().child(&b).css_classes(["flat", "menew-action"]).tooltip_text(&a.title).build()
}

impl Popover {
    pub fn new(app: &adw::Application, hooks: Hooks) -> Rc<Self> {
        let lang = current_language();
        let window = adw::ApplicationWindow::builder().application(app).title("Menew").default_width(400).default_height(600).hide_on_close(true).build();
        let stack = adw::ViewStack::new();

        // ---- Now ----
        let search = gtk4::SearchEntry::builder().placeholder_text(tr("pop_search", lang, &[])).build();
        let now_box = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(10).build();
        let now_favs = gtk4::FlowBox::builder().selection_mode(gtk4::SelectionMode::None).max_children_per_line(4).min_children_per_line(3).homogeneous(true).build();
        let now = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(12).margin_top(12).margin_bottom(12).margin_start(12).margin_end(12).build();
        now.append(&search);
        now.append(&now_box);
        now.append(&gtk4::Label::builder().label(tr("pop_favorites", lang, &[])).xalign(0.0).css_classes(["heading"]).build());
        now.append(&now_favs);
        let now_scroll = gtk4::ScrolledWindow::builder().hscrollbar_policy(gtk4::PolicyType::Never).vexpand(true).child(&now).build();
        stack.add_titled_with_icon(&now_scroll, Some("now"), &tr("pop_now", lang, &[]), "go-home-symbolic");

        // ---- Actions ----
        let actions_box = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(12).margin_top(12).margin_bottom(12).margin_start(12).margin_end(12).build();
        let actions_scroll = gtk4::ScrolledWindow::builder().hscrollbar_policy(gtk4::PolicyType::Never).vexpand(true).child(&actions_box).build();
        stack.add_titled_with_icon(&actions_scroll, Some("actions"), &tr("pop_actions", lang, &[]), "view-app-grid-symbolic");

        let header = adw::HeaderBar::new();
        let switcher = adw::ViewSwitcher::builder().stack(&stack).policy(adw::ViewSwitcherPolicy::Wide).build();
        header.set_title_widget(Some(&switcher));
        let open = gtk4::Button::builder().icon_name("sidebar-show-symbolic").tooltip_text(tr("tray_open_window", lang, &[])).build();
        header.pack_end(&open);
        let tv = adw::ToolbarView::new();
        tv.add_top_bar(&header);
        tv.set_content(Some(&stack));
        window.set_content(Some(&tv));

        let me = Rc::new(Self { window, hooks, now_box, now_favs, actions_box, editing: Cell::new(false), search, stack, catalog: RefCell::new(vec![]) });
        let w = Rc::downgrade(&me);
        open.connect_clicked(move |_| {
            if let Some(m) = w.upgrade() {
                m.window.set_visible(false);
                (m.hooks.open_window)();
            }
        });
        let w = Rc::downgrade(&me);
        me.search.connect_activate(move |e| {
            if let Some(m) = w.upgrade() {
                let t = e.text().to_string();
                e.set_text("");
                m.window.set_visible(false);
                (m.hooks.search)(&t);
            }
        });
        let w = Rc::downgrade(&me);
        me.search.connect_changed(move |e| {
            // typing goes straight to the palette, like the Mac's Now tab search box
            if let Some(m) = w.upgrade() {
                let t = e.text().to_string();
                if !t.is_empty() {
                    e.set_text("");
                    m.window.set_visible(false);
                    (m.hooks.search)(&t);
                }
            }
        });
        let esc = gtk4::EventControllerKey::new();
        let w = Rc::downgrade(&me);
        esc.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                if let Some(m) = w.upgrade() {
                    m.window.set_visible(false);
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        me.window.add_controller(esc);
        // refresh the Now tab while it is open
        let w = Rc::downgrade(&me);
        glib::timeout_add_seconds_local(2, move || {
            let Some(m) = w.upgrade() else { return glib::ControlFlow::Break };
            if m.window.is_visible() {
                m.fill_now();
            }
            glib::ControlFlow::Continue
        });
        me
    }

    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }

    pub fn toggle(self: &Rc<Self>) {
        if self.window.is_visible() {
            self.window.set_visible(false);
        } else {
            self.show();
        }
    }

    pub fn show(self: &Rc<Self>) {
        self.rebuild();
        self.stack.set_visible_child_name("now");
        self.window.present();
        self.search.grab_focus();
    }

    /// Rebuild both tabs (the actions follow the state: mute, dark mode...).
    pub fn rebuild(self: &Rc<Self>) {
        let groups = (self.hooks.groups)();
        *self.catalog.borrow_mut() = groups.iter().flat_map(|g| g.1.clone()).collect();
        self.fill_now();
        self.fill_favorites(&self.now_favs, false);
        self.fill_actions(&groups);
    }

    fn run(self: &Rc<Self>, cmd: &str) {
        self.window.set_visible(false);
        (self.hooks.run)(cmd);
    }

    fn fill_now(&self) {
        let lang = current_language();
        while let Some(c) = self.now_box.first_child() {
            self.now_box.remove(&c);
        }
        let d = (self.hooks.now)();
        self.now_box.append(&gtk4::Label::builder().label(&d.status).xalign(0.0).css_classes(["title-4"]).build());
        let pills = gtk4::Box::builder().spacing(6).homogeneous(true).build();
        for (k, v) in &d.metrics {
            let p = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).css_classes(["card"]).build();
            p.append(&gtk4::Label::builder().label(v).css_classes(["heading", "numeric"]).margin_top(6).build());
            p.append(&gtk4::Label::builder().label(k).css_classes(["caption", "dim-label"]).margin_bottom(6).build());
            pills.append(&p);
        }
        self.now_box.append(&pills);
        if !d.running.is_empty() {
            for r in &d.running {
                self.now_box.append(&gtk4::Label::builder().label(r).xalign(0.0).build());
            }
        }
        if !d.clocks.is_empty() {
            self.now_box.append(&gtk4::Label::builder().label(tr("pop_clocks", lang, &[])).xalign(0.0).css_classes(["heading"]).build());
            let grid = gtk4::Box::builder().spacing(6).homogeneous(true).build();
            for (city, time) in d.clocks.iter().take(4) {
                let c = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).css_classes(["card"]).build();
                c.append(&gtk4::Label::builder().label(time).css_classes(["heading", "numeric"]).margin_top(6).build());
                c.append(&gtk4::Label::builder().label(city).css_classes(["caption", "dim-label"]).ellipsize(gtk4::pango::EllipsizeMode::End).margin_bottom(6).build());
                grid.append(&c);
            }
            self.now_box.append(&grid);
        }
    }

    fn entry_for(&self, cmd: &str) -> Option<ActionEntry> {
        self.catalog.borrow().iter().find(|a| a.command == cmd).cloned()
    }

    fn fill_favorites(self: &Rc<Self>, flow: &gtk4::FlowBox, editing: bool) {
        while let Some(c) = flow.first_child() {
            flow.remove(&c);
        }
        for cmd in (self.hooks.favorites)() {
            let Some(a) = self.entry_for(&cmd) else { continue };
            let b = action_button(&a);
            let w = Rc::downgrade(self);
            let c = cmd.clone();
            b.connect_clicked(move |_| {
                let Some(m) = w.upgrade() else { return };
                if editing {
                    let mut f = (m.hooks.favorites)();
                    f.retain(|x| *x != c);
                    (m.hooks.set_favorites)(f);
                    let m2 = Rc::clone(&m);
                    glib::idle_add_local_once(move || m2.rebuild());
                } else {
                    m.run(&c);
                }
            });
            if editing {
                b.add_css_class("menew-editing");
                b.set_tooltip_text(Some(&tr("pop_remove_fav", current_language(), &[&a.title])));
            }
            flow.append(&b);
        }
    }

    fn fill_actions(self: &Rc<Self>, groups: &[(String, Vec<ActionEntry>)]) {
        let lang = current_language();
        while let Some(c) = self.actions_box.first_child() {
            self.actions_box.remove(&c);
        }
        let editing = self.editing.get();
        let top = gtk4::Box::builder().spacing(6).build();
        top.append(&gtk4::Label::builder().label(tr("pop_favorites", lang, &[])).xalign(0.0).hexpand(true).css_classes(["heading"]).build());
        let edit = gtk4::ToggleButton::builder().label(tr(if editing { "pop_done" } else { "pop_edit" }, lang, &[])).active(editing).css_classes(["flat"]).build();
        let w = Rc::downgrade(self);
        edit.connect_toggled(move |b| {
            if let Some(m) = w.upgrade() {
                m.editing.set(b.is_active());
                let m2 = Rc::clone(&m);
                glib::idle_add_local_once(move || m2.rebuild());
            }
        });
        top.append(&edit);
        self.actions_box.append(&top);
        let favs = gtk4::FlowBox::builder().selection_mode(gtk4::SelectionMode::None).max_children_per_line(4).min_children_per_line(3).homogeneous(true).build();
        self.fill_favorites(&favs, editing);
        self.actions_box.append(&favs);
        if editing {
            self.actions_box.append(&gtk4::Label::builder().label(tr("pop_edit_hint", lang, &[])).wrap(true).xalign(0.0).css_classes(["dim-label", "caption"]).build());
        }
        let current = (self.hooks.favorites)();
        for (title, items) in groups {
            if items.is_empty() {
                continue;
            }
            let exp = gtk4::Expander::builder().label(title).expanded(true).build();
            let flow = gtk4::FlowBox::builder().selection_mode(gtk4::SelectionMode::None).max_children_per_line(4).min_children_per_line(3).homogeneous(true).build();
            for a in items {
                if editing && current.contains(&a.command) {
                    continue;
                }
                let b = action_button(a);
                let w = Rc::downgrade(self);
                let c = a.command.clone();
                b.connect_clicked(move |_| {
                    let Some(m) = w.upgrade() else { return };
                    if m.editing.get() {
                        let mut f = (m.hooks.favorites)();
                        if !f.contains(&c) {
                            f.push(c.clone());
                        }
                        (m.hooks.set_favorites)(f);
                        let m2 = Rc::clone(&m);
                        glib::idle_add_local_once(move || m2.rebuild());
                    } else {
                        m.run(&c);
                    }
                });
                flow.append(&b);
            }
            exp.set_child(Some(&flow));
            self.actions_box.append(&exp);
        }
    }
}
