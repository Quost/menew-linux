//! The Menew window (port of `MenewWindow.swift`): a sidebar like GNOME Settings with the sections
//! (Overview, Notes, Performance, Settings...), each a page built by its own module and handed in by
//! the daemon. Closing the window only hides it; Menew keeps running in the tray.

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// One section of the sidebar.
pub struct Section {
    pub id: &'static str,
    pub title: String,
    pub icon: &'static str,
    pub page: gtk4::Widget,
    /// Extra buttons for the header bar of this section.
    pub header: Vec<gtk4::Widget>,
    /// Called each time the section is shown (refresh its data).
    pub on_show: Option<Rc<dyn Fn()>>,
}

pub struct MenewWindow {
    window: adw::ApplicationWindow,
    split: adw::NavigationSplitView,
    list: gtk4::ListBox,
    stack: gtk4::Stack,
    content_page: adw::NavigationPage,
    header: adw::HeaderBar,
    sections: RefCell<Vec<Section>>,
    header_widgets: RefCell<Vec<gtk4::Widget>>,
}

impl MenewWindow {
    pub fn new(app: &adw::Application, sections: Vec<Section>) -> Rc<Self> {
        let window = adw::ApplicationWindow::builder().application(app).title("Menew").default_width(980).default_height(680).hide_on_close(true).build();
        window.set_size_request(360, 400);

        let list = gtk4::ListBox::builder().css_classes(["navigation-sidebar"]).selection_mode(gtk4::SelectionMode::Single).build();
        for s in &sections {
            let row = gtk4::Box::builder().spacing(12).margin_top(8).margin_bottom(8).margin_start(6).margin_end(6).build();
            row.append(&gtk4::Image::from_icon_name(s.icon));
            row.append(&gtk4::Label::builder().label(&s.title).xalign(0.0).hexpand(true).build());
            list.append(&row);
        }
        let side_scroll = gtk4::ScrolledWindow::builder().hscrollbar_policy(gtk4::PolicyType::Never).vexpand(true).child(&list).build();
        let side_toolbar = adw::ToolbarView::new();
        side_toolbar.add_top_bar(&adw::HeaderBar::builder().title_widget(&adw::WindowTitle::new("Menew", "")).build());
        side_toolbar.set_content(Some(&side_scroll));
        let sidebar = adw::NavigationPage::builder().title("Menew").child(&side_toolbar).build();

        let stack = gtk4::Stack::builder().transition_type(gtk4::StackTransitionType::Crossfade).hhomogeneous(false).vhomogeneous(false).build();
        for s in &sections {
            stack.add_named(&s.page, Some(s.id));
        }
        let header = adw::HeaderBar::new();
        let content_toolbar = adw::ToolbarView::new();
        content_toolbar.add_top_bar(&header);
        content_toolbar.set_content(Some(&stack));
        let content_page = adw::NavigationPage::builder().title(sections.first().map(|s| s.title.as_str()).unwrap_or("Menew")).tag("content").child(&content_toolbar).build();

        let split = adw::NavigationSplitView::builder().sidebar(&sidebar).content(&content_page).min_sidebar_width(200.0).max_sidebar_width(260.0).build();
        window.set_content(Some(&split));

        // narrow windows collapse the sidebar (phones, tiled halves)
        let bp = adw::Breakpoint::new(adw::BreakpointCondition::new_length(adw::BreakpointConditionLengthType::MaxWidth, 600.0, adw::LengthUnit::Sp));
        bp.add_setter(&split, "collapsed", Some(&true.to_value()));
        window.add_breakpoint(bp);

        let me = Rc::new(Self { window, split, list, stack, content_page, header, sections: RefCell::new(sections), header_widgets: RefCell::new(vec![]) });
        let weak = Rc::downgrade(&me);
        me.list.connect_row_selected(move |_, row| {
            if let (Some(me), Some(row)) = (weak.upgrade(), row) {
                me.activate_index(row.index() as usize);
            }
        });
        if let Some(r) = me.list.row_at_index(0) {
            me.list.select_row(Some(&r));
        }
        me
    }

    fn activate_index(&self, i: usize) {
        let (id, title, on_show, header) = {
            let s = self.sections.borrow();
            let Some(sec) = s.get(i) else { return };
            (sec.id, sec.title.clone(), sec.on_show.clone(), sec.header.clone())
        };
        self.stack.set_visible_child_name(id);
        self.content_page.set_title(&title);
        for w in self.header_widgets.borrow_mut().drain(..) {
            self.header.remove(&w);
        }
        for w in header {
            self.header.pack_end(&w);
            self.header_widgets.borrow_mut().push(w);
        }
        self.split.set_show_content(true);
        if let Some(f) = on_show {
            f();
        }
    }

    /// Open the window on a section (`notes`, `performance`, `settings`...).
    pub fn show_section(&self, id: &str) {
        let idx = self.sections.borrow().iter().position(|s| s.id == id);
        if let Some(i) = idx {
            if let Some(row) = self.list.row_at_index(i as i32) {
                if self.list.selected_row().as_ref() == Some(&row) {
                    self.activate_index(i);
                } else {
                    self.list.select_row(Some(&row));
                }
            }
        }
        self.present();
    }

    /// Show or hide a section in the sidebar (a feature turned off).
    pub fn set_section_visible(&self, id: &str, visible: bool) {
        let idx = self.sections.borrow().iter().position(|s| s.id == id);
        if let Some(r) = idx.and_then(|i| self.list.row_at_index(i as i32)) {
            r.set_visible(visible);
        }
    }

    pub fn present(&self) {
        self.window.set_visible(true);
        self.window.present();
    }

    pub fn visible_section(&self) -> Option<String> {
        self.window.is_visible().then(|| self.stack.visible_child_name().map(|s| s.to_string())).flatten()
    }

    pub fn window(&self) -> &adw::ApplicationWindow {
        &self.window
    }

    /// Run `f` every `secs` while the window shows section `id` (live graphs).
    pub fn every(self: &Rc<Self>, id: &'static str, secs: u32, f: impl Fn() + 'static) {
        let weak = Rc::downgrade(self);
        glib::timeout_add_seconds_local(secs, move || {
            let Some(me) = weak.upgrade() else { return glib::ControlFlow::Break };
            if me.visible_section().as_deref() == Some(id) {
                f();
            }
            glib::ControlFlow::Continue
        });
    }
}

/// A titled group of rows (`adw::PreferencesGroup`) inside a scrolling, centered page.
pub fn page() -> (gtk4::ScrolledWindow, gtk4::Box) {
    let body = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(18).margin_top(18).margin_bottom(24).margin_start(12).margin_end(12).build();
    let clamp = adw::Clamp::builder().maximum_size(820).child(&body).build();
    let scroll = gtk4::ScrolledWindow::builder().hscrollbar_policy(gtk4::PolicyType::Never).vexpand(true).child(&clamp).build();
    (scroll, body)
}
