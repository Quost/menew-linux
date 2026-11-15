//! The Notes section (port of `NotesViews.swift`): the list with search and #tags on the left, the
//! editor on the right. Text notes are Markdown, styled while typing, with clickable `- [ ]`
//! checklists. Password notes are a title and `key: value` fields with hidden values that are
//! copied marked as a password and cleared from the clipboard after 30 s.

use crate::dialogs;
use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use menew_core::notes::{self, Block, NoteKind, NotesStore, SecretField};
use menew_core::strings::tr;
use menew_core::current_language;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub type SharedNotes = Rc<RefCell<NotesStore>>;

fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

pub struct NotesView {
    store: SharedNotes,
    pub root: gtk4::Widget,
    pub header: Vec<gtk4::Widget>,
    search: gtk4::SearchEntry,
    tags_box: gtk4::Box,
    list: gtk4::ListBox,
    editor_stack: gtk4::Stack,
    text_view: gtk4::TextView,
    fields_box: gtk4::Box,
    title_entry: gtk4::Entry,
    protect_btn: gtk4::ToggleButton,
    kind_btn: gtk4::ToggleButton,
    delete_btn: gtk4::Button,
    selected: RefCell<Option<String>>,
    ids: RefCell<Vec<String>>,
    tag: RefCell<Option<String>>,
    /// Set while code changes the widgets, so their handlers do not save.
    loading: Cell<bool>,
    save_pending: Cell<Option<glib::SourceId>>,
    on_changed: RefCell<Option<Rc<dyn Fn()>>>,
}

impl NotesView {
    pub fn new(store: SharedNotes) -> Rc<Self> {
        let lang = current_language();
        // ---- list side ----
        let search = gtk4::SearchEntry::builder().placeholder_text(tr("note_search", lang, &[])).margin_top(8).margin_start(8).margin_end(8).build();
        let tags_box = gtk4::Box::builder().spacing(4).margin_start(8).margin_end(8).build();
        let tags_scroll = gtk4::ScrolledWindow::builder().vscrollbar_policy(gtk4::PolicyType::Never).child(&tags_box).build();
        let list = gtk4::ListBox::builder().css_classes(["navigation-sidebar"]).build();
        let list_scroll = gtk4::ScrolledWindow::builder().hscrollbar_policy(gtk4::PolicyType::Never).vexpand(true).child(&list).build();
        let left = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(6).width_request(240).build();
        left.append(&search);
        left.append(&tags_scroll);
        left.append(&list_scroll);

        // ---- editor side ----
        let editor_stack = gtk4::Stack::builder().hexpand(true).vexpand(true).build();
        let empty = adw::StatusPage::builder().icon_name("accessories-text-editor-symbolic").title(tr("note_empty", lang, &[])).description(tr("note_empty_sub", lang, &[])).build();
        editor_stack.add_named(&empty, Some("empty"));
        let unlock_btn = gtk4::Button::builder().label(tr("note_unlock", lang, &[])).halign(gtk4::Align::Center).css_classes(["pill", "suggested-action"]).build();
        let locked = adw::StatusPage::builder().icon_name("channel-secure-symbolic").title(tr("note_locked", lang, &[])).child(&unlock_btn).build();
        editor_stack.add_named(&locked, Some("locked"));
        let text_view = gtk4::TextView::builder().wrap_mode(gtk4::WrapMode::WordChar).left_margin(18).right_margin(18).top_margin(14).bottom_margin(14).build();
        setup_tags(&text_view.buffer());
        let text_scroll = gtk4::ScrolledWindow::builder().child(&text_view).build();
        editor_stack.add_named(&text_scroll, Some("text"));
        let title_entry = gtk4::Entry::builder().placeholder_text(tr("note_title_placeholder", lang, &[])).css_classes(["title-3"]).build();
        let fields_box = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(6).build();
        let add_field = gtk4::Button::builder().label(tr("note_add_field", lang, &[])).halign(gtk4::Align::Start).css_classes(["flat"]).build();
        let pw_body = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(12).margin_top(18).margin_bottom(18).margin_start(18).margin_end(18).build();
        pw_body.append(&title_entry);
        pw_body.append(&fields_box);
        pw_body.append(&add_field);
        let pw_scroll = gtk4::ScrolledWindow::builder().child(&adw::Clamp::builder().maximum_size(640).child(&pw_body).build()).build();
        editor_stack.add_named(&pw_scroll, Some("passwords"));

        let paned = gtk4::Paned::builder().orientation(gtk4::Orientation::Horizontal).start_child(&left).end_child(&editor_stack).position(260).shrink_start_child(false).build();

        // ---- header buttons ----
        let new_btn = gtk4::MenuButton::builder().icon_name("list-add-symbolic").tooltip_text(tr("note_new", lang, &[])).build();
        let menu = gio::Menu::new();
        menu.append(Some(&tr("note_new", lang, &[])), Some("notes.new"));
        menu.append(Some(&tr("note_new_passwords", lang, &[])), Some("notes.newpasswords"));
        menu.append(Some(&tr("note_lock_all", lang, &[])), Some("notes.lockall"));
        new_btn.set_menu_model(Some(&menu));
        let protect_btn = gtk4::ToggleButton::builder().icon_name("channel-secure-symbolic").tooltip_text(tr("note_protect_help", lang, &[])).build();
        let kind_btn = gtk4::ToggleButton::builder().icon_name("dialog-password-symbolic").tooltip_text(tr("note_kind_passwords", lang, &[])).build();
        let delete_btn = gtk4::Button::builder().icon_name("user-trash-symbolic").tooltip_text(tr("note_delete", lang, &[])).build();

        let me = Rc::new(Self {
            store,
            root: paned.upcast(),
            header: vec![new_btn.clone().upcast(), delete_btn.clone().upcast(), protect_btn.clone().upcast(), kind_btn.clone().upcast()],
            search,
            tags_box,
            list,
            editor_stack,
            text_view,
            fields_box,
            title_entry,
            protect_btn,
            kind_btn,
            delete_btn,
            selected: RefCell::new(None),
            ids: RefCell::new(vec![]),
            tag: RefCell::new(None),
            loading: Cell::new(false),
            save_pending: Cell::new(None),
            on_changed: RefCell::new(None),
        });

        let group = gio::SimpleActionGroup::new();
        for (name, f) in [
            ("new", Box::new(|m: &Rc<NotesView>| m.new_note(false)) as Box<dyn Fn(&Rc<NotesView>)>),
            ("newpasswords", Box::new(|m: &Rc<NotesView>| m.new_note(true))),
            ("lockall", Box::new(|m: &Rc<NotesView>| {
                m.store.borrow_mut().lock_all();
                m.show_selected();
            })),
        ] {
            let a = gio::SimpleAction::new(name, None);
            let w = Rc::downgrade(&me);
            a.connect_activate(move |_, _| {
                if let Some(m) = w.upgrade() {
                    f(&m);
                }
            });
            group.add_action(&a);
        }
        me.root.insert_action_group("notes", Some(&group));

        let w = Rc::downgrade(&me);
        me.search.connect_search_changed(move |_| {
            if let Some(m) = w.upgrade() {
                m.reload_list();
            }
        });
        let w = Rc::downgrade(&me);
        me.list.connect_row_activated(move |_, row| {
            if let Some(m) = w.upgrade() {
                let id = m.ids.borrow().get(row.index() as usize).cloned();
                if let Some(id) = id {
                    m.select(&id);
                }
            }
        });
        let w = Rc::downgrade(&me);
        me.text_view.buffer().connect_changed(move |buf| {
            if let Some(m) = w.upgrade() {
                restyle(buf);
                if !m.loading.get() {
                    m.schedule_save();
                }
            }
        });
        // click on "[ ]" toggles the task
        let click = gtk4::GestureClick::new();
        let w = Rc::downgrade(&me);
        click.connect_released(move |g, _, x, y| {
            let Some(m) = w.upgrade() else { return };
            let tv = &m.text_view;
            let (bx, by) = tv.window_to_buffer_coords(gtk4::TextWindowType::Widget, x as i32, y as i32);
            let Some(iter) = tv.iter_at_location(bx, by) else { return };
            let buf = tv.buffer();
            let mut start = iter;
            start.set_line_offset(0);
            let mut end = start;
            end.forward_to_line_end();
            let line = buf.text(&start, &end, false).to_string();
            if !matches!(notes::block_of(&line), Block::Task(_)) {
                return;
            }
            let Some(open) = line.find('[') else { return };
            let col = iter.line_offset() as usize;
            let open_c = line[..open].chars().count();
            if col >= open_c && col <= open_c + 3 {
                let new = notes::toggle_task(&line);
                buf.begin_user_action();
                buf.delete(&mut start.clone(), &mut end.clone());
                let mut at = buf.iter_at_line(iter.line()).unwrap_or(buf.start_iter());
                buf.insert(&mut at, &new);
                buf.end_user_action();
                g.set_state(gtk4::EventSequenceState::Claimed);
            }
        });
        me.text_view.add_controller(click);
        let w = Rc::downgrade(&me);
        me.title_entry.connect_changed(move |_| {
            if let Some(m) = w.upgrade() {
                if !m.loading.get() {
                    m.save_fields();
                }
            }
        });
        let w = Rc::downgrade(&me);
        add_field.connect_clicked(move |_| {
            if let Some(m) = w.upgrade() {
                m.add_field_row("", "", true);
                m.save_fields();
            }
        });
        let w = Rc::downgrade(&me);
        unlock_btn.connect_clicked(move |_| {
            if let Some(m) = w.upgrade() {
                m.unlock_selected();
            }
        });
        let w = Rc::downgrade(&me);
        me.protect_btn.connect_toggled(move |b| {
            let Some(m) = w.upgrade() else { return };
            if m.loading.get() {
                return;
            }
            let Some(id) = m.selected.borrow().clone() else { return };
            m.flush_save();
            if b.is_active() && !notes::vault_available() {
                m.toast(&tr("note_vault_missing", current_language(), &[]));
                m.loading.set(true);
                b.set_active(false);
                m.loading.set(false);
                return;
            }
            let ok = m.store.borrow_mut().set_secure(&id, b.is_active());
            if !ok {
                m.toast(&tr("note_vault_missing", current_language(), &[]));
            }
            m.reload_list();
            m.show_selected();
        });
        let w = Rc::downgrade(&me);
        me.kind_btn.connect_toggled(move |b| {
            let Some(m) = w.upgrade() else { return };
            if m.loading.get() {
                return;
            }
            let Some(id) = m.selected.borrow().clone() else { return };
            m.flush_save();
            if b.is_active() && !notes::vault_available() {
                m.toast(&tr("note_vault_missing", current_language(), &[]));
                m.loading.set(true);
                b.set_active(false);
                m.loading.set(false);
                return;
            }
            m.store.borrow_mut().set_kind(&id, if b.is_active() { NoteKind::Passwords } else { NoteKind::Text });
            m.reload_list();
            m.show_selected();
        });
        let w = Rc::downgrade(&me);
        me.delete_btn.connect_clicked(move |b| {
            let Some(m) = w.upgrade() else { return };
            let Some(id) = m.selected.borrow().clone() else { return };
            let lang = current_language();
            let parent = b.root().and_downcast::<gtk4::Window>();
            let w2 = Rc::downgrade(&m);
            dialogs::confirm(parent.as_ref(), &tr("note_delete_ask", lang, &[]), &tr("note_delete_body", lang, &[]), &tr("delete", lang, &[]), &tr("cancel", lang, &[]), true, move || {
                if let Some(m) = w2.upgrade() {
                    m.cancel_save();
                    m.store.borrow_mut().delete(&id);
                    *m.selected.borrow_mut() = None;
                    m.reload_list();
                    m.select_first();
                    m.changed();
                }
            });
        });
        me.reload_list();
        me.select_first();
        me
    }

    pub fn set_on_changed(&self, f: Rc<dyn Fn()>) {
        *self.on_changed.borrow_mut() = Some(f);
    }

    fn changed(&self) {
        let f = self.on_changed.borrow().clone();
        if let Some(f) = f {
            f();
        }
    }

    fn toast(&self, text: &str) {
        if let Some(app) = gio::Application::default() {
            let n = gio::Notification::new(text);
            app.send_notification(Some("menew-notes"), &n);
        }
    }

    /// Reload from disk (another window or the palette may have changed it).
    pub fn refresh(self: &Rc<Self>) {
        self.reload_list();
    }

    pub fn new_note(self: &Rc<Self>, passwords: bool) {
        self.flush_save();
        if passwords && !notes::vault_available() {
            self.toast(&tr("note_vault_missing", current_language(), &[]));
            return;
        }
        let id = if passwords { self.store.borrow_mut().create_passwords(current_language(), now()) } else { self.store.borrow_mut().create("", now()) };
        self.search.set_text("");
        *self.tag.borrow_mut() = None;
        self.reload_list();
        self.select(&id);
        if !passwords {
            self.text_view.grab_focus();
        } else {
            self.title_entry.grab_focus();
        }
        self.changed();
    }

    fn select_first(self: &Rc<Self>) {
        let first = self.ids.borrow().first().cloned();
        match first {
            Some(id) => self.select(&id),
            None => {
                *self.selected.borrow_mut() = None;
                self.show_selected();
            }
        }
    }

    pub fn select(self: &Rc<Self>, id: &str) {
        self.flush_save();
        *self.selected.borrow_mut() = Some(id.to_string());
        let idx = self.ids.borrow().iter().position(|x| x == id);
        if let Some(i) = idx {
            if let Some(r) = self.list.row_at_index(i as i32) {
                self.list.select_row(Some(&r));
            }
        }
        self.show_selected();
    }

    fn reload_list(self: &Rc<Self>) {
        let lang = current_language();
        while let Some(c) = self.list.first_child() {
            self.list.remove(&c);
        }
        while let Some(c) = self.tags_box.first_child() {
            self.tags_box.remove(&c);
        }
        let store = self.store.borrow();
        let tags = store.all_tags();
        if !tags.is_empty() {
            let cur = self.tag.borrow().clone();
            let all = gtk4::ToggleButton::builder().label(tr("note_all_tags", lang, &[])).css_classes(["flat", "menew-chip"]).active(cur.is_none()).build();
            let w = Rc::downgrade(self);
            all.connect_clicked(move |_| {
                if let Some(m) = w.upgrade() {
                    *m.tag.borrow_mut() = None;
                    glib::idle_add_local_once(move || m.reload_list());
                }
            });
            self.tags_box.append(&all);
            for t in tags {
                let b = gtk4::ToggleButton::builder().label(format!("#{t}")).css_classes(["flat", "menew-chip"]).active(cur.as_deref() == Some(t.as_str())).build();
                let w = Rc::downgrade(self);
                b.connect_clicked(move |_| {
                    if let Some(m) = w.upgrade() {
                        *m.tag.borrow_mut() = Some(t.clone());
                        glib::idle_add_local_once(move || m.reload_list());
                    }
                });
                self.tags_box.append(&b);
            }
        }
        let tag = self.tag.borrow().clone();
        let q = self.search.text().to_string();
        let found = store.filtered(&q, tag.as_deref());
        let mut ids = vec![];
        for n in found {
            let row = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(2).margin_top(6).margin_bottom(6).margin_start(6).margin_end(6).build();
            let top = gtk4::Box::builder().spacing(6).build();
            if n.secure {
                top.append(&gtk4::Image::from_icon_name(if n.is_passwords() { "dialog-password-symbolic" } else { "channel-secure-symbolic" }));
            }
            top.append(&gtk4::Label::builder().label(n.title(lang)).xalign(0.0).ellipsize(gtk4::pango::EllipsizeMode::End).css_classes(["heading"]).build());
            row.append(&top);
            let sub = n.preview(lang);
            if !sub.is_empty() {
                row.append(&gtk4::Label::builder().label(sub).xalign(0.0).ellipsize(gtk4::pango::EllipsizeMode::End).css_classes(["dim-label", "caption"]).build());
            }
            self.list.append(&row);
            ids.push(n.id.clone());
        }
        drop(store);
        let sel = self.selected.borrow().clone();
        if let Some(i) = sel.and_then(|s| ids.iter().position(|x| *x == s)) {
            if let Some(r) = self.list.row_at_index(i as i32) {
                self.list.select_row(Some(&r));
            }
        }
        *self.ids.borrow_mut() = ids;
    }

    fn show_selected(self: &Rc<Self>) {
        self.loading.set(true);
        let id = self.selected.borrow().clone();
        let store = self.store.borrow();
        let note = id.as_deref().and_then(|i| store.note(i)).cloned();
        let has = note.is_some();
        self.protect_btn.set_sensitive(has);
        self.kind_btn.set_sensitive(has);
        self.delete_btn.set_sensitive(has);
        match note {
            None => self.editor_stack.set_visible_child_name("empty"),
            Some(n) => {
                self.protect_btn.set_active(n.secure);
                self.protect_btn.set_tooltip_text(Some(&tr(if n.secure { "note_unprotect" } else { "note_protect_help" }, current_language(), &[])));
                self.kind_btn.set_active(n.is_passwords());
                if store.is_locked(&n.id) {
                    self.editor_stack.set_visible_child_name("locked");
                } else if n.is_passwords() {
                    let (title, fields) = notes::parse_fields(&store.text(&n.id));
                    drop(store);
                    self.title_entry.set_text(&title);
                    while let Some(c) = self.fields_box.first_child() {
                        self.fields_box.remove(&c);
                    }
                    for f in fields {
                        self.add_field_row(&f.key, &f.value, false);
                    }
                    self.editor_stack.set_visible_child_name("passwords");
                    self.loading.set(false);
                    return;
                } else {
                    let text = store.text(&n.id);
                    self.text_view.buffer().set_text(&text);
                    self.editor_stack.set_visible_child_name("text");
                }
            }
        }
        self.loading.set(false);
    }

    fn add_field_row(self: &Rc<Self>, key: &str, value: &str, focus: bool) {
        let lang = current_language();
        let row = gtk4::Box::builder().spacing(6).build();
        let k = gtk4::Entry::builder().text(key).placeholder_text(tr("note_field_name", lang, &[])).width_chars(12).build();
        let v = gtk4::PasswordEntry::builder().text(value).placeholder_text(tr("note_field_value", lang, &[])).show_peek_icon(true).hexpand(true).build();
        let copy = gtk4::Button::builder().icon_name("edit-copy-symbolic").tooltip_text(tr("note_copy", lang, &[])).css_classes(["flat"]).build();
        let remove = gtk4::Button::builder().icon_name("list-remove-symbolic").css_classes(["flat"]).build();
        row.append(&k);
        row.append(&v);
        row.append(&copy);
        row.append(&remove);
        self.fields_box.append(&row);
        let w = Rc::downgrade(self);
        k.connect_changed(move |_| {
            if let Some(m) = w.upgrade() {
                if !m.loading.get() {
                    m.save_fields();
                }
            }
        });
        let w = Rc::downgrade(self);
        v.connect_changed(move |_| {
            if let Some(m) = w.upgrade() {
                if !m.loading.get() {
                    m.save_fields();
                }
            }
        });
        let vv = v.clone();
        let w = Rc::downgrade(self);
        copy.connect_clicked(move |_| {
            if let Some(m) = w.upgrade() {
                copy_secret(&vv.text());
                m.toast(&tr("note_secret_copied", current_language(), &[]));
            }
        });
        let w = Rc::downgrade(self);
        let r2 = row.clone();
        remove.connect_clicked(move |_| {
            if let Some(m) = w.upgrade() {
                m.fields_box.remove(&r2);
                m.save_fields();
            }
        });
        if focus {
            k.grab_focus();
        }
    }

    fn save_fields(self: &Rc<Self>) {
        let Some(id) = self.selected.borrow().clone() else { return };
        let mut fields = vec![];
        let mut c = self.fields_box.first_child();
        while let Some(row) = c {
            let k = row.first_child().and_downcast::<gtk4::Entry>();
            let v = row.first_child().and_then(|x| x.next_sibling()).and_downcast::<gtk4::PasswordEntry>();
            if let (Some(k), Some(v)) = (k, v) {
                fields.push(SecretField { key: k.text().to_string(), value: v.text().to_string() });
            }
            c = row.next_sibling();
        }
        let text = notes::format_fields(&self.title_entry.text(), &fields);
        self.store.borrow_mut().set_text(&id, &text, now());
        self.reload_list();
    }

    fn schedule_save(self: &Rc<Self>) {
        self.cancel_save();
        let w = Rc::downgrade(self);
        let src = glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || {
            if let Some(m) = w.upgrade() {
                m.save_pending.set(None);
                m.save_text();
            }
        });
        self.save_pending.set(Some(src));
    }

    fn cancel_save(&self) {
        if let Some(s) = self.save_pending.take() {
            s.remove();
        }
    }

    /// Save a pending edit now (before switching notes).
    fn flush_save(self: &Rc<Self>) {
        if let Some(s) = self.save_pending.take() {
            s.remove();
            self.save_text();
        }
    }

    fn save_text(self: &Rc<Self>) {
        let Some(id) = self.selected.borrow().clone() else { return };
        if self.editor_stack.visible_child_name().as_deref() != Some("text") {
            return;
        }
        let buf = self.text_view.buffer();
        let text = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();
        if self.store.borrow().text(&id) == text {
            return;
        }
        self.store.borrow_mut().set_text(&id, &text, now());
        self.reload_list();
        self.changed();
    }

    fn unlock_selected(self: &Rc<Self>) {
        let Some(id) = self.selected.borrow().clone() else { return };
        let me = Rc::clone(self);
        glib::spawn_future_local(async move {
            let id2 = id.clone();
            let text = gio::spawn_blocking(move || {
                if !notes::authenticate() {
                    return None;
                }
                use notes::Vault;
                notes::SecretServiceVault.read(&id2)
            })
            .await
            .ok()
            .flatten();
            match text {
                Some(t) => {
                    me.store.borrow_mut().set_unlocked(&id, t);
                    me.show_selected();
                }
                None => me.toast(&tr("note_unlock_fail", current_language(), &[])),
            }
        });
    }
}

/// Copy a secret marked as a password (KDE's hint, which Menew and other clipboard managers skip)
/// and clear it after 30 s if nothing else was copied (`SecretCopy` on the Mac).
pub fn copy_secret(value: &str) {
    let Some(display) = gdk::Display::default() else { return };
    let clipboard = display.clipboard();
    let bytes = glib::Bytes::from(value.as_bytes());
    let provider = gdk::ContentProvider::new_union(&[
        gdk::ContentProvider::for_bytes("text/plain;charset=utf-8", &bytes),
        gdk::ContentProvider::for_bytes("text/plain", &bytes),
        gdk::ContentProvider::for_bytes("UTF8_STRING", &bytes),
        gdk::ContentProvider::for_bytes("x-kde-passwordManagerHint", &glib::Bytes::from_static(b"secret")),
    ]);
    if clipboard.set_content(Some(&provider)).is_err() {
        return;
    }
    glib::timeout_add_seconds_local_once(30, move || {
        if clipboard.content().as_ref() == Some(&provider) {
            let _ = clipboard.set_content(None::<&gdk::ContentProvider>);
        }
    });
}

fn setup_tags(buf: &gtk4::TextBuffer) {
    let table = buf.tag_table();
    let add = |name: &str, f: &dyn Fn(&gtk4::TextTag)| {
        let t = gtk4::TextTag::new(Some(name));
        f(&t);
        table.add(&t);
    };
    add("h1", &|t| {
        t.set_scale(1.6);
        t.set_weight(800);
    });
    add("h2", &|t| {
        t.set_scale(1.35);
        t.set_weight(700);
    });
    add("h3", &|t| {
        t.set_scale(1.15);
        t.set_weight(700);
    });
    add("bold", &|t| t.set_weight(700));
    add("italic", &|t| t.set_style(gtk4::pango::Style::Italic));
    add("strike", &|t| t.set_strikethrough(true));
    add("code", &|t| t.set_family(Some("monospace")));
    add("quote", &|t| {
        t.set_style(gtk4::pango::Style::Italic);
        t.set_left_margin(30);
    });
    add("done", &|t| {
        t.set_strikethrough(true);
        t.set_foreground_rgba(Some(&gdk::RGBA::new(0.5, 0.5, 0.5, 1.0)));
    });
    add("mark", &|t| t.set_foreground_rgba(Some(&gdk::RGBA::new(0.5, 0.5, 0.5, 1.0))));
    add("tag", &|t| t.set_foreground_rgba(Some(&gdk::RGBA::new(0.21, 0.52, 0.89, 1.0))));
}

/// Apply the Markdown styling to the whole buffer (notes are small).
fn restyle(buf: &gtk4::TextBuffer) {
    buf.remove_all_tags(&buf.start_iter(), &buf.end_iter());
    let text = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();
    let mut in_code = false;
    for (n, line) in text.split('\n').enumerate() {
        let Some(start) = buf.iter_at_line(n as i32) else { continue };
        let mut end = start;
        if !end.ends_line() {
            end.forward_to_line_end();
        }
        let block = notes::block_of(line);
        if block == Block::Code {
            in_code = !in_code;
            buf.apply_tag_by_name("code", &start, &end);
            continue;
        }
        if in_code {
            buf.apply_tag_by_name("code", &start, &end);
            continue;
        }
        match block {
            Block::Heading(1) => buf.apply_tag_by_name("h1", &start, &end),
            Block::Heading(2) => buf.apply_tag_by_name("h2", &start, &end),
            Block::Heading(_) => buf.apply_tag_by_name("h3", &start, &end),
            Block::Quote => buf.apply_tag_by_name("quote", &start, &end),
            Block::Task(true) => buf.apply_tag_by_name("done", &start, &end),
            _ => {}
        }
        let at = |c: usize| {
            let mut i = start;
            i.forward_chars(c as i32);
            i
        };
        for (a, b, style) in notes::inline_spans(line) {
            buf.apply_tag_by_name(style, &at(a), &at(b));
        }
        // #tags
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '#' && (i == 0 || chars[i - 1].is_whitespace()) {
                let mut j = i + 1;
                while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_' || chars[j] == '-') {
                    j += 1;
                }
                if j > i + 1 {
                    buf.apply_tag_by_name("tag", &at(i), &at(j));
                }
                i = j;
            } else {
                i += 1;
            }
        }
    }
}
