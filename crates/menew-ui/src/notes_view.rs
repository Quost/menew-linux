//! The notes window (GTK4 / libadwaita): list with search and tags, editor, Markdown reading mode, the
//! password rows, and the copy that marks a secret as confidential. A reusable component: the full Menew
//! window (Phase 11) will host the same [`NotesApp`] in its Notes page; for now it has a window of its own.
//! The data and the rules are in `menew_core::notes` and `menew_core::markdown`.

use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};
use libadwaita as adw;
use libadwaita::prelude::*;
use menew_core::markdown::{self, Block};
use menew_core::notes::{self, Auth, Authenticator, Kind, NotesStore, SecretField, VaultError};
use menew_core::strings::tr;
use menew_core::{age_between, current_language, format_age, Language};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

type AddRow = Rc<dyn Fn(&str, &str)>;

const CSS: &str = "
    .menew-quote { border-left: 3px solid @accent_bg_color; padding-left: 8px; }
    .menew-code { font-family: monospace; padding: 8px; background: alpha(currentColor, 0.06); border-radius: 8px; }
    .menew-notice { background: alpha(@warning_color, 0.18); padding: 6px 12px; }
    .menew-pw-row { background: alpha(currentColor, 0.045); border-radius: 10px; padding: 8px 12px; }
";

fn test_mode() -> bool {
    std::env::var("MENEW_TEST").map(|v| v == "1").unwrap_or(false)
}

/// What the unlock of a protected note ended in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unlocked {
    Yes,
    No,
}

struct Ui {
    window: adw::ApplicationWindow,
    search: gtk4::SearchEntry,
    tags_box: gtk4::Box,
    list: gtk4::ListBox,
    stack: gtk4::Stack,
    notice: gtk4::Label,
    title: gtk4::Label,
    edit_btn: gtk4::ToggleButton,
    read_btn: gtk4::ToggleButton,
    menu_btn: gtk4::MenuButton,
    menu_box: gtk4::Box,
    body: gtk4::Stack,
    text_view: gtk4::TextView,
    placeholder: gtk4::Label,
    read_box: gtk4::Box,
    pw_box: gtk4::Box,
    status: gtk4::Label,
    empty: gtk4::Label,
}

pub struct NotesApp {
    app: adw::Application,
    pub store: Rc<RefCell<NotesStore>>,
    auth: Arc<dyn Authenticator>,
    ui: RefCell<Option<Rc<Ui>>>,
    query: RefCell<String>,
    tag: RefCell<Option<String>>,
    /// Ids of the rows of the list, in order.
    rows: RefCell<Vec<String>>,
    current: RefCell<Option<String>>,
    reading: Cell<bool>,
    /// Set while code fills widgets, so that their handlers do not write back.
    syncing: Cell<bool>,
    /// The notice about polkit not being able to ask, once seen.
    keyring_only: RefCell<Option<String>>,
    save_gen: Cell<u32>,
}

impl NotesApp {
    pub fn new(app: &adw::Application, store: NotesStore, auth: Arc<dyn Authenticator>) -> Rc<Self> {
        Rc::new(Self {
            app: app.clone(),
            store: Rc::new(RefCell::new(store)),
            auth,
            ui: RefCell::new(None),
            query: RefCell::new(String::new()),
            tag: RefCell::new(None),
            rows: RefCell::new(vec![]),
            current: RefCell::new(None),
            reading: Cell::new(false),
            syncing: Cell::new(false),
            keyring_only: RefCell::new(None),
            save_gen: Cell::new(0),
        })
    }

    fn lang() -> Language {
        current_language()
    }

    fn notify(&self, id: &str, title: &str, body: Option<&str>) {
        let n = gio::Notification::new(title);
        if let Some(b) = body {
            n.set_body(Some(b));
        }
        self.app.send_notification(Some(id), &n);
        info!("Notification sent: {}{}", title, body.map(|b| format!(" / {b}")).unwrap_or_default());
    }

    fn vault_error(&self, e: &VaultError) {
        warn!("Notes: {}", e);
        let lang = Self::lang();
        let msg = match e {
            VaultError::Unavailable(_) => tr("vault_unavailable", lang, &[]),
            other => tr("vault_failed", lang, &[&other.to_string()]),
        };
        self.notify("menew-notes", &msg, None);
        if let Some(ui) = self.ui.borrow().as_ref() {
            ui.status.set_label(&msg);
        }
    }

    // ---- unlocking ----

    /// Ask the user (polkit) and read the protected note from the keyring, then call `done`. A note that is not
    /// protected, or already open, answers at once. The password dialog runs off the UI thread.
    pub fn unlock(self: &Rc<Self>, id: &str, done: impl FnOnce(Unlocked) + 'static) {
        if !self.store.borrow().is_locked(id) {
            return done(Unlocked::Yes);
        }
        let this = Rc::clone(self);
        let id = id.to_string();
        let auth = Arc::clone(&self.auth);
        glib::spawn_future_local(async move {
            let lang = Self::lang();
            let answer = gio::spawn_blocking(move || auth.authenticate()).await.unwrap_or(Auth::Denied);
            info!("Notes: unlock of {} -> {:?}", id, answer);
            match answer {
                Auth::Denied => {
                    this.notify("menew-notes", &tr("auth_denied", lang, &[]), None);
                    return done(Unlocked::No);
                }
                Auth::KeyringOnly(reason) => {
                    info!("Notes: polkit could not ask ({}); the keyring unlocked at login is the protection", reason);
                    *this.keyring_only.borrow_mut() = Some(reason);
                    this.refresh_notice();
                }
                Auth::Polkit => {}
            }
            let opened = this.store.borrow_mut().unlock(&id);
            match opened {
                Ok(true) => done(Unlocked::Yes),
                Ok(false) => done(Unlocked::No),
                Err(e) => {
                    this.vault_error(&e);
                    done(Unlocked::No)
                }
            }
        });
    }

    fn refresh_notice(&self) {
        if let Some(ui) = self.ui.borrow().as_ref() {
            let on = self.keyring_only.borrow().is_some();
            ui.notice.set_label(&tr("auth_keyring_only", Self::lang(), &[]));
            ui.notice.set_visible(on);
        }
    }

    // ---- the secret on the clipboard ----

    /// Copy a secret: marked confidential (`x-kde-passwordManagerHint=secret`, so the history of Menew and other
    /// managers ignore it) and cleared after 30 s if the clipboard still holds it (`SecretCopy.copy`).
    pub fn copy_secret(self: &Rc<Self>, value: &str) {
        if test_mode() {
            // headless sessions only accept the selection from a client with a mapped window
            self.ensure_ui();
            if let Some(ui) = self.ui.borrow().as_ref() {
                if !ui.window.is_visible() {
                    ui.window.present();
                }
            }
        }
        let Some(display) = gdk::Display::default() else { return };
        let clipboard = display.clipboard();
        let text = gdk::ContentProvider::for_value(&value.to_value());
        let hint = gdk::ContentProvider::for_bytes("x-kde-passwordManagerHint", &glib::Bytes::from_static(b"secret"));
        let provider = gdk::ContentProvider::new_union(&[text, hint]);
        if clipboard.set_content(Some(&provider)).is_err() {
            warn!("Notes: could not put the secret on the clipboard");
            return;
        }
        let secs = notes::secret_clear_secs(test_mode(), std::env::var("MENEW_SECRET_CLEAR_SECS").ok().as_deref());
        info!("Notes: secret copied as confidential; cleared in {} s if still on the clipboard", secs);
        self.notify("menew-copied", &tr("pw_copied", Self::lang(), &[]), None);
        glib::timeout_add_local_once(Duration::from_secs(secs), move || {
            if clipboard.content().as_ref() == Some(&provider) {
                let _ = clipboard.set_content(None::<&gdk::ContentProvider>);
                info!("Notes: the copied secret was cleared from the clipboard");
            } else {
                info!("Notes: the clipboard changed since the secret was copied; left alone");
            }
        });
    }

    /// Paste (here: copy; the caller pastes) the password or username of a password note, asking first.
    /// `done(Some(true))` when copied, `Some(false)` when the note has no such field, `None` when not unlocked.
    pub fn copy_password_field(self: &Rc<Self>, id: &str, username: bool, done: impl FnOnce(Option<bool>) + 'static) {
        let this = Rc::clone(self);
        let id2 = id.to_string();
        let was_open = !self.store.borrow().is_locked(id);
        self.unlock(id, move |u| {
            if u == Unlocked::No {
                return done(None);
            }
            let fields = SecretField::parse(&this.store.borrow().text_of(&id2)).1;
            if !was_open {
                this.store.borrow_mut().lock(&id2);
            }
            match notes::secret_value(&fields, username).filter(|v| !v.is_empty()) {
                Some(v) => {
                    this.copy_secret(&v);
                    done(Some(true));
                }
                None => {
                    let key = if username { "pw_no_username" } else { "pw_no_password" };
                    this.notify("menew-copied", &tr(key, Self::lang(), &[]), None);
                    done(Some(false));
                }
            }
        });
    }

    // ---- the window ----

    pub fn show(self: &Rc<Self>) {
        self.ensure_ui();
        let ui = self.ui.borrow().clone().unwrap();
        self.refresh_all();
        ui.window.present();
    }

    /// Open the notes window on that note (a protected one asks first).
    pub fn open(self: &Rc<Self>, id: &str) {
        self.show();
        *self.query.borrow_mut() = String::new();
        *self.tag.borrow_mut() = None;
        if let Some(ui) = self.ui.borrow().as_ref() {
            ui.search.set_text("");
        }
        let this = Rc::clone(self);
        let id = id.to_string();
        let id2 = id.clone();
        self.unlock(&id, move |u| {
            if u == Unlocked::Yes {
                this.show_note(&id2);
            }
        });
    }

    pub fn new_note(self: &Rc<Self>) -> String {
        let tag = self.tag.borrow().clone();
        let id = self.store.borrow_mut().create(&tag.map(|t| format!("\n\n#{t}")).unwrap_or_default());
        let _ = self.store.borrow_mut().flush();
        self.notify("menew-notes", &tr("notes_created", Self::lang(), &[]), None);
        self.show();
        self.show_note(&id);
        id
    }

    pub fn new_passwords(self: &Rc<Self>) -> Option<String> {
        let made = self.store.borrow_mut().create_passwords(Self::lang());
        match made {
            Ok(id) => {
                self.notify("menew-notes", &tr("notes_created", Self::lang(), &[]), None);
                self.show();
                self.show_note(&id);
                Some(id)
            }
            Err(e) => {
                self.show();
                self.vault_error(&e);
                None
            }
        }
    }

    fn flush_now(&self) {
        let r = self.store.borrow_mut().flush();
        if let Err(e) = r {
            self.vault_error(&e);
        }
    }

    /// Write 600 ms after the last change (like the Mac), then refresh the list.
    fn save_soon(self: &Rc<Self>) {
        let gen = self.save_gen.get().wrapping_add(1);
        self.save_gen.set(gen);
        let this = Rc::clone(self);
        glib::timeout_add_local_once(Duration::from_millis(600), move || {
            if this.save_gen.get() == gen {
                this.flush_now();
                this.refresh_list();
                this.refresh_status();
            }
        });
    }

    fn hide(&self) {
        self.flush_now();
        self.store.borrow_mut().lock_all();
        *self.current.borrow_mut() = None;
        if let Some(ui) = self.ui.borrow().as_ref() {
            ui.window.set_visible(false);
        }
    }

    fn ensure_ui(self: &Rc<Self>) {
        if self.ui.borrow().is_some() {
            return;
        }
        let lang = Self::lang();
        if let Some(display) = gdk::Display::default() {
            let provider = gtk4::CssProvider::new();
            provider.load_from_data(CSS);
            gtk4::style_context_add_provider_for_display(&display, &provider, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }
        let window = adw::ApplicationWindow::builder().application(&self.app).title(tr("notes_title", lang, &[])).default_width(900).default_height(620).build();

        // left: search, new buttons, tags, list
        let search = gtk4::SearchEntry::builder().placeholder_text(tr("notes_search", lang, &[])).hexpand(true).build();
        let new_btn = gtk4::Button::builder().icon_name("document-new-symbolic").tooltip_text(tr("notes_new", lang, &[])).css_classes(["suggested-action"]).build();
        let new_pw_btn = gtk4::Button::builder().icon_name("dialog-password-symbolic").tooltip_text(tr("notes_new_passwords", lang, &[])).build();
        let top = gtk4::Box::builder().spacing(6).margin_top(8).margin_start(8).margin_end(8).margin_bottom(4).build();
        top.append(&search);
        top.append(&new_pw_btn);
        top.append(&new_btn);
        let tags_box = gtk4::Box::builder().spacing(4).margin_start(8).margin_end(8).margin_bottom(4).build();
        let tags_scroll = gtk4::ScrolledWindow::builder().vscrollbar_policy(gtk4::PolicyType::Never).hscrollbar_policy(gtk4::PolicyType::Automatic).child(&tags_box).build();
        let list = gtk4::ListBox::builder().selection_mode(gtk4::SelectionMode::Single).css_classes(["navigation-sidebar"]).build();
        let list_scroll = gtk4::ScrolledWindow::builder().vexpand(true).hscrollbar_policy(gtk4::PolicyType::Never).child(&list).build();
        let left = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).width_request(300).build();
        left.append(&top);
        left.append(&tags_scroll);
        left.append(&list_scroll);

        // right: the editor
        let title = gtk4::Label::builder().halign(gtk4::Align::Start).hexpand(true).ellipsize(gtk4::pango::EllipsizeMode::End).css_classes(["heading"]).build();
        let edit_btn = gtk4::ToggleButton::builder().icon_name("document-edit-symbolic").tooltip_text(tr("notes_edit", lang, &[])).active(true).build();
        let read_btn = gtk4::ToggleButton::builder().icon_name("view-reveal-symbolic").tooltip_text(tr("notes_read", lang, &[])).group(&edit_btn).build();
        let seg = gtk4::Box::builder().css_classes(["linked"]).build();
        seg.append(&edit_btn);
        seg.append(&read_btn);
        let menu_box = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(2).margin_top(4).margin_bottom(4).margin_start(4).margin_end(4).build();
        let popover = gtk4::Popover::builder().child(&menu_box).build();
        let menu_btn = gtk4::MenuButton::builder().icon_name("view-more-symbolic").popover(&popover).build();
        let head = gtk4::Box::builder().spacing(8).margin_top(8).margin_start(12).margin_end(8).margin_bottom(4).build();
        head.append(&title);
        head.append(&seg);
        head.append(&menu_btn);

        let text_view = gtk4::TextView::builder().wrap_mode(gtk4::WrapMode::WordChar).top_margin(8).bottom_margin(8).left_margin(10).right_margin(10).build();
        let placeholder = gtk4::Label::builder().label(tr("notes_placeholder", lang, &[])).halign(gtk4::Align::Start).valign(gtk4::Align::Start).margin_top(8).margin_start(12).css_classes(["dim-label"]).can_target(false).build();
        let overlay = gtk4::Overlay::builder().child(&gtk4::ScrolledWindow::builder().child(&text_view).vexpand(true).build()).build();
        overlay.add_overlay(&placeholder);
        let read_box = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(5).margin_top(8).margin_bottom(8).margin_start(12).margin_end(12).build();
        let read_scroll = gtk4::ScrolledWindow::builder().child(&read_box).vexpand(true).build();
        let pw_box = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(8).margin_top(8).margin_bottom(8).margin_start(12).margin_end(12).build();
        let pw_scroll = gtk4::ScrolledWindow::builder().child(&pw_box).vexpand(true).build();
        let body = gtk4::Stack::builder().vexpand(true).build();
        body.add_named(&overlay, Some("edit"));
        body.add_named(&read_scroll, Some("read"));
        body.add_named(&pw_scroll, Some("pw"));
        let status = gtk4::Label::builder().halign(gtk4::Align::Start).margin_start(12).margin_bottom(6).css_classes(["dim-label", "caption"]).build();
        let editor = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).hexpand(true).build();
        editor.append(&head);
        editor.append(&body);
        editor.append(&status);
        let empty = gtk4::Label::builder().label(tr("notes_none", lang, &[])).css_classes(["dim-label", "title-2"]).hexpand(true).vexpand(true).build();
        let stack = gtk4::Stack::builder().hexpand(true).build();
        stack.add_named(&empty, Some("empty"));
        stack.add_named(&editor, Some("editor"));

        let notice = gtk4::Label::builder().wrap(true).xalign(0.0).css_classes(["menew-notice"]).visible(false).build();
        let paned = gtk4::Paned::builder().orientation(gtk4::Orientation::Horizontal).start_child(&left).end_child(&stack).position(310).shrink_start_child(false).resize_end_child(true).vexpand(true).build();
        let root = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).build();
        root.append(&adw::HeaderBar::new());
        root.append(&notice);
        root.append(&paned);
        window.set_content(Some(&root));

        let ui = Rc::new(Ui { window, search, tags_box, list, stack, notice, title, edit_btn, read_btn, menu_btn, menu_box, body, text_view, placeholder, read_box, pw_box, status, empty });
        *self.ui.borrow_mut() = Some(Rc::clone(&ui));

        // events
        {
            let this = Rc::clone(self);
            ui.search.connect_search_changed(move |e| {
                *this.query.borrow_mut() = e.text().to_string();
                this.refresh_list();
            });
            let this = Rc::clone(self);
            new_btn.connect_clicked(move |_| {
                this.new_note();
            });
            let this = Rc::clone(self);
            new_pw_btn.connect_clicked(move |_| {
                this.new_passwords();
            });
            let this = Rc::clone(self);
            ui.list.connect_row_activated(move |_, row| {
                let id = this.rows.borrow().get(row.index() as usize).cloned();
                if let Some(id) = id {
                    this.open_in_place(&id);
                }
            });
            let this = Rc::clone(self);
            ui.read_btn.connect_toggled(move |b| {
                if this.syncing.get() {
                    return;
                }
                this.reading.set(b.is_active());
                let cur = this.current.borrow().clone();
                if let Some(id) = cur {
                    this.show_note(&id);
                }
            });
            let this = Rc::clone(self);
            ui.text_view.buffer().connect_changed(move |buf| {
                if this.syncing.get() {
                    return;
                }
                let cur = this.current.borrow().clone();
                if let Some(id) = cur {
                    let text = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();
                    this.store.borrow_mut().set_text(&id, &text);
                    if let Some(ui) = this.ui.borrow().as_ref() {
                        ui.placeholder.set_visible(text.is_empty());
                        ui.title.set_label(&this.title_of(&id));
                    }
                    this.save_soon();
                }
            });
            let this = Rc::clone(self);
            ui.window.connect_close_request(move |_| {
                this.hide();
                glib::Propagation::Stop
            });
        }
    }

    fn title_of(&self, id: &str) -> String {
        self.store.borrow().note(id).map(|n| n.title(Self::lang())).unwrap_or_default()
    }

    /// A click on a row: ask for the protected ones first.
    fn open_in_place(self: &Rc<Self>, id: &str) {
        let this = Rc::clone(self);
        let id2 = id.to_string();
        self.unlock(id, move |u| {
            if u == Unlocked::Yes {
                this.show_note(&id2);
            }
        });
    }

    fn refresh_all(self: &Rc<Self>) {
        self.refresh_notice();
        self.refresh_list();
        let cur = self.current.borrow().clone();
        match cur {
            Some(id) if self.store.borrow().note(&id).is_some() && !self.store.borrow().is_locked(&id) => {}
            _ => {
                *self.current.borrow_mut() = None;
                if let Some(ui) = self.ui.borrow().as_ref() {
                    ui.stack.set_visible_child_name("empty");
                }
            }
        }
    }

    /// Rebuild the tag chips and the list for the query and tag.
    fn refresh_list(self: &Rc<Self>) {
        let Some(ui) = self.ui.borrow().clone() else { return };
        let lang = Self::lang();
        while let Some(c) = ui.tags_box.first_child() {
            ui.tags_box.remove(&c);
        }
        let tags = self.store.borrow().all_tags();
        let current_tag = self.tag.borrow().clone();
        if !tags.is_empty() {
            let mut first: Option<gtk4::ToggleButton> = None;
            let mut mk = |label: &str, tag: Option<String>| {
                let b = gtk4::ToggleButton::builder().label(label).css_classes(["flat", "pill"]).active(current_tag == tag).build();
                if let Some(f) = &first {
                    b.set_group(Some(f));
                } else {
                    first = Some(b.clone());
                }
                let this = Rc::clone(self);
                b.connect_clicked(move |_| {
                    let next = if *this.tag.borrow() == tag { None } else { tag.clone() };
                    *this.tag.borrow_mut() = next;
                    this.refresh_list();
                });
                ui.tags_box.append(&b);
            };
            mk(&tr("notes_all", lang, &[]), None);
            for t in &tags {
                mk(&format!("#{t}"), Some(t.clone()));
            }
        }
        while let Some(c) = ui.list.first_child() {
            ui.list.remove(&c);
        }
        let query = self.query.borrow().clone();
        let store = self.store.borrow();
        let found = store.filtered(&query, current_tag.as_deref());
        let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
        let mut ids = vec![];
        for n in &found {
            let icon = if n.is_passwords() { "dialog-password-symbolic" } else if n.secure { "changes-prevent-symbolic" } else { "text-x-generic-symbolic" };
            let tags_line = n.tags().iter().map(|t| format!("#{t}")).collect::<Vec<_>>().join(" ");
            let mut sub = n.preview(lang);
            if !tags_line.is_empty() {
                sub = if sub.is_empty() { tags_line } else { format!("{sub}\n{tags_line}") };
            }
            let age = format_age(age_between(now_ms, ((n.updated + notes::SWIFT_EPOCH_OFFSET) * 1000.0) as i64));
            let row = adw::ActionRow::builder().title(glib::markup_escape_text(&n.title(lang))).subtitle(glib::markup_escape_text(&sub)).title_lines(1).subtitle_lines(2).activatable(true).icon_name(icon).build();
            row.add_suffix(&gtk4::Label::builder().label(age).css_classes(["dim-label", "caption"]).build());
            ui.list.append(&row);
            ids.push(n.id.clone());
        }
        let empty = ids.is_empty();
        let none_at_all = store.notes.is_empty();
        drop(store);
        if empty {
            let text = if none_at_all { tr("notes_none", lang, &[]) } else { tr("notes_not_found", lang, &[]) };
            ui.empty.set_label(&text);
        }
        // keep the selection on the open note
        if let Some(cur) = self.current.borrow().as_ref() {
            if let Some(pos) = ids.iter().position(|i| i == cur) {
                if let Some(row) = ui.list.row_at_index(pos as i32) {
                    ui.list.select_row(Some(&row));
                }
            }
        }
        *self.rows.borrow_mut() = ids;
    }

    fn refresh_status(&self) {
        let Some(ui) = self.ui.borrow().clone() else { return };
        let Some(id) = self.current.borrow().clone() else { return };
        let (secure, chars) = {
            let s = self.store.borrow();
            (s.note(&id).map(|n| n.secure).unwrap_or(false), s.text_of(&id).chars().count())
        };
        let lang = Self::lang();
        ui.status.set_label(&if secure { tr("notes_status_protected", lang, &[]) } else { tr("notes_status_saved", lang, &[&chars.to_string()]) });
    }

    /// Show a note (already unlocked if protected) in the editor, the reading view or the password rows.
    pub fn show_note(self: &Rc<Self>, id: &str) {
        self.ensure_ui();
        let ui = self.ui.borrow().clone().unwrap();
        if self.store.borrow().note(id).is_none() || self.store.borrow().is_locked(id) {
            return;
        }
        self.flush_now();
        *self.current.borrow_mut() = Some(id.to_string());
        let (is_pw, secure, text) = {
            let s = self.store.borrow();
            let n = s.note(id).unwrap();
            (n.is_passwords(), n.secure, s.text_of(id))
        };
        self.syncing.set(true);
        ui.title.set_label(&self.title_of(id));
        ui.stack.set_visible_child_name("editor");
        let reading = self.reading.get() && !is_pw;
        ui.edit_btn.set_visible(!is_pw);
        ui.read_btn.set_visible(!is_pw);
        ui.read_btn.set_active(reading);
        ui.edit_btn.set_active(!reading);
        if is_pw {
            ui.body.set_visible_child_name("pw");
            self.build_passwords(id, &text);
        } else if reading {
            ui.body.set_visible_child_name("read");
            self.build_reading(id, &text);
        } else {
            ui.body.set_visible_child_name("edit");
            ui.text_view.buffer().set_text(&text);
            ui.placeholder.set_visible(text.is_empty());
            ui.text_view.grab_focus();
        }
        self.syncing.set(false);
        self.build_menu(id, is_pw, secure);
        self.refresh_status();
        self.refresh_list();
    }

    fn menu_item(&self, label: &str, f: impl Fn() + 'static) -> gtk4::Button {
        let b = gtk4::Button::builder().label(label).css_classes(["flat"]).halign(gtk4::Align::Fill).build();
        if let Some(l) = b.child().and_downcast::<gtk4::Label>() {
            l.set_xalign(0.0);
        }
        b.connect_clicked(move |_| f());
        b
    }

    fn build_menu(self: &Rc<Self>, id: &str, is_pw: bool, secure: bool) {
        let ui = self.ui.borrow().clone().unwrap();
        let lang = Self::lang();
        while let Some(c) = ui.menu_box.first_child() {
            ui.menu_box.remove(&c);
        }
        let pop = |ui: &Ui| {
            if let Some(p) = ui.menu_btn.popover() {
                p.popdown();
            }
        };
        let (this, id_s) = (Rc::clone(self), id.to_string());
        ui.menu_box.append(&self.menu_item(&tr("notes_copy", lang, &[]), {
            let (this, id) = (Rc::clone(&this), id_s.clone());
            move || {
                let text = this.store.borrow().text_of(&id);
                if this.store.borrow().note(&id).map(|n| n.is_passwords()).unwrap_or(false) {
                    this.copy_secret(&text);
                } else if let Some(d) = gdk::Display::default() {
                    d.clipboard().set_text(&text);
                }
                if let Some(ui) = this.ui.borrow().as_ref() {
                    if let Some(p) = ui.menu_btn.popover() {
                        p.popdown();
                    }
                }
            }
        }));
        let kind_btn = |kind: Kind, label: String| {
            let (this, id) = (Rc::clone(&this), id_s.clone());
            self.menu_item(&label, move || {
                let r = this.store.borrow_mut().set_kind(&id, kind);
                if let Err(e) = r {
                    this.vault_error(&e);
                }
                if let Some(ui) = this.ui.borrow().as_ref() {
                    if let Some(p) = ui.menu_btn.popover() {
                        p.popdown();
                    }
                }
                this.show_note(&id);
            })
        };
        ui.menu_box.append(&kind_btn(Kind::Text, format!("{}: {}", tr("notes_type", lang, &[]), tr("notes_kind_text", lang, &[]))));
        ui.menu_box.append(&kind_btn(Kind::Passwords, format!("{}: {}", tr("notes_type", lang, &[]), tr("notes_kind_passwords", lang, &[]))));
        if !is_pw {
            let (this, id) = (Rc::clone(&this), id_s.clone());
            let label = tr(if secure { "notes_remove_protection" } else { "notes_protect" }, lang, &[]);
            ui.menu_box.append(&self.menu_item(&label, move || {
                if let Some(ui) = this.ui.borrow().as_ref() {
                    if let Some(p) = ui.menu_btn.popover() {
                        p.popdown();
                    }
                }
                this.toggle_protection(&id, secure);
            }));
        }
        let (this2, id2) = (Rc::clone(&this), id_s.clone());
        let del = self.menu_item(&tr("notes_delete_note", lang, &[]), move || {
            if let Some(ui) = this2.ui.borrow().as_ref() {
                if let Some(p) = ui.menu_btn.popover() {
                    p.popdown();
                }
            }
            this2.confirm_delete(&id2);
        });
        del.add_css_class("destructive-action");
        ui.menu_box.append(&del);
        pop(&ui);
    }

    /// Protect (the content goes to the keyring) or remove the protection (asks first, the content comes back).
    fn toggle_protection(self: &Rc<Self>, id: &str, secure: bool) {
        let r = self.store.borrow_mut().set_secure(id, !secure);
        if let Err(e) = r {
            self.vault_error(&e);
        }
        self.show_note(id);
    }

    fn confirm_delete(self: &Rc<Self>, id: &str) {
        let lang = Self::lang();
        let ui = self.ui.borrow().clone().unwrap();
        let dialog = adw::MessageDialog::new(Some(&ui.window), Some(&tr("notes_delete_confirm", lang, &[])), None);
        dialog.add_response("cancel", &tr("snip_cancel", lang, &[]));
        dialog.add_response("delete", &tr("notes_delete", lang, &[]));
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_close_response("cancel");
        let (this, id) = (Rc::clone(self), id.to_string());
        dialog.connect_response(None, move |_, r| {
            if r == "delete" {
                let res = this.store.borrow_mut().delete(&id);
                if let Err(e) = res {
                    this.vault_error(&e);
                }
                *this.current.borrow_mut() = None;
                if let Some(ui) = this.ui.borrow().as_ref() {
                    ui.stack.set_visible_child_name("empty");
                }
                this.refresh_list();
            }
        });
        dialog.present();
    }

    // ---- Markdown reading mode ----

    fn build_reading(self: &Rc<Self>, id: &str, text: &str) {
        let ui = self.ui.borrow().clone().unwrap();
        while let Some(c) = ui.read_box.first_child() {
            ui.read_box.remove(&c);
        }
        let this = Rc::clone(self);
        let on_tag: Rc<dyn Fn(String)> = Rc::new(move |t| {
            *this.tag.borrow_mut() = Some(t);
            *this.current.borrow_mut() = None;
            if let Some(ui) = this.ui.borrow().as_ref() {
                ui.stack.set_visible_child_name("empty");
            }
            this.refresh_list();
        });
        let (this, id_s) = (Rc::clone(self), id.to_string());
        let on_toggle: Rc<dyn Fn(usize)> = Rc::new(move |line| {
            let text = this.store.borrow().text_of(&id_s);
            let next = markdown::toggle_task(&text, line);
            this.store.borrow_mut().set_text(&id_s, &next);
            this.save_soon();
            this.build_reading(&id_s, &next);
        });
        for b in render_markdown(text, on_toggle, on_tag) {
            ui.read_box.append(&b);
        }
    }

    // ---- password rows ----

    fn build_passwords(self: &Rc<Self>, id: &str, text: &str) {
        let ui = self.ui.borrow().clone().unwrap();
        let lang = Self::lang();
        while let Some(c) = ui.pw_box.first_child() {
            ui.pw_box.remove(&c);
        }
        let (title, fields) = SecretField::parse(text);
        let title_entry = gtk4::Entry::builder().text(&title).placeholder_text(tr("pw_title_placeholder", lang, &[])).css_classes(["title-3", "flat"]).build();
        ui.pw_box.append(&title_entry);
        let rows: Rc<RefCell<Vec<(gtk4::Entry, gtk4::Entry)>>> = Rc::new(RefCell::new(vec![]));
        let rows_box = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(8).build();
        ui.pw_box.append(&rows_box);

        // collect everything into the text of the note
        let collect: Rc<dyn Fn()> = {
            let (this, id, rows, title_entry) = (Rc::clone(self), id.to_string(), Rc::clone(&rows), title_entry.clone());
            Rc::new(move || {
                if this.syncing.get() {
                    return;
                }
                let fields: Vec<SecretField> = rows.borrow().iter().map(|(k, v)| SecretField::new(&k.text(), &v.text())).collect();
                let text = SecretField::format(&title_entry.text(), &fields);
                this.store.borrow_mut().set_text(&id, &text);
                if let Some(ui) = this.ui.borrow().as_ref() {
                    ui.title.set_label(&this.title_of(&id));
                }
                this.save_soon();
            })
        };
        {
            let c = Rc::clone(&collect);
            title_entry.connect_changed(move |_| c());
        }
        let add_row: AddRow = {
            let (this, rows, rows_box, collect) = (Rc::clone(self), Rc::clone(&rows), rows_box.clone(), Rc::clone(&collect));
            Rc::new(move |key: &str, value: &str| {
                let lang = Self::lang();
                let key_e = gtk4::Entry::builder().text(key).placeholder_text(tr("pw_name", lang, &[])).css_classes(["flat", "caption-heading"]).hexpand(true).build();
                let val_e = gtk4::Entry::builder().text(value).placeholder_text(tr("pw_value", lang, &[])).visibility(false).hexpand(true).css_classes(["flat", "monospace"]).build();
                let eye = gtk4::ToggleButton::builder().icon_name("view-reveal-symbolic").tooltip_text(tr("pw_show", lang, &[])).css_classes(["flat"]).build();
                let copy = gtk4::Button::builder().icon_name("edit-copy-symbolic").tooltip_text(tr("notes_copy", lang, &[])).css_classes(["flat"]).build();
                let remove = gtk4::Button::builder().icon_name("user-trash-symbolic").tooltip_text(tr("pw_remove_field", lang, &[])).css_classes(["flat"]).build();
                let top = gtk4::Box::builder().spacing(4).build();
                top.append(&key_e);
                top.append(&remove);
                let bottom = gtk4::Box::builder().spacing(4).build();
                bottom.append(&val_e);
                bottom.append(&eye);
                bottom.append(&copy);
                let row = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(2).css_classes(["menew-pw-row"]).build();
                row.append(&top);
                row.append(&bottom);
                rows_box.append(&row);
                rows.borrow_mut().push((key_e.clone(), val_e.clone()));
                let c = Rc::clone(&collect);
                key_e.connect_changed(move |_| c());
                let c = Rc::clone(&collect);
                val_e.connect_changed(move |_| c());
                let v = val_e.clone();
                eye.connect_toggled(move |b| {
                    v.set_visibility(b.is_active());
                    b.set_tooltip_text(Some(&tr(if b.is_active() { "pw_hide" } else { "pw_show" }, Self::lang(), &[])));
                });
                let (this2, v) = (Rc::clone(&this), val_e.clone());
                copy.connect_clicked(move |_| {
                    let t = v.text().to_string();
                    if !t.is_empty() {
                        this2.copy_secret(&t);
                    }
                });
                let (rows2, rows_box2, row2, key2, c) = (Rc::clone(&rows), rows_box.clone(), row.clone(), key_e.clone(), Rc::clone(&collect));
                remove.connect_clicked(move |_| {
                    rows2.borrow_mut().retain(|(k, _)| *k != key2);
                    rows_box2.remove(&row2);
                    c();
                });
            })
        };
        for f in &fields {
            add_row(&f.key, &f.value);
        }
        let add = gtk4::Button::builder().label(tr("pw_add_field", lang, &[])).icon_name("list-add-symbolic").css_classes(["flat"]).halign(gtk4::Align::Start).build();
        {
            let (add_row, rows) = (Rc::clone(&add_row), Rc::clone(&rows));
            add.connect_clicked(move |_| {
                add_row("", "");
                if let Some((k, _)) = rows.borrow().last() {
                    k.grab_focus();
                }
            });
        }
        ui.pw_box.append(&add);
    }

    // ---- test hooks (`MENEW_TEST=1`) ----

    /// Create a note for the desktop tests: `kind` is `text`, `secure` or `passwords`. Logs the id.
    pub fn test_create(self: &Rc<Self>, kind: &str, text: &str) -> Option<String> {
        let lang = Self::lang();
        let id = match kind {
            "passwords" => {
                let id = match self.store.borrow_mut().create_passwords(lang) {
                    Ok(id) => id,
                    Err(e) => {
                        warn!("Test hook: could not create the passwords note: {}", e);
                        return None;
                    }
                };
                self.store.borrow_mut().set_text(&id, text);
                id
            }
            _ => {
                let id = self.store.borrow_mut().create(text);
                if kind == "secure" {
                    if let Err(e) = self.store.borrow_mut().set_secure(&id, true) {
                        warn!("Test hook: could not protect the note: {}", e);
                        return None;
                    }
                }
                id
            }
        };
        match self.store.borrow_mut().flush() {
            Ok(()) => info!("Test hook: note created id={} kind={}", id, kind),
            Err(e) => warn!("Test hook: note {} created but the keyring failed: {}", id, e),
        }
        // like after a restart: a protected note is closed, so opening it goes through the unlock (polkit) path
        if self.store.borrow().note(&id).map(|n| n.secure).unwrap_or(false) {
            self.store.borrow_mut().lock(&id);
        }
        Some(id)
    }
}

/// The widgets of the blocks of a note (`MarkdownView`).
pub fn render_markdown(text: &str, on_toggle: Rc<dyn Fn(usize)>, on_tag: Rc<dyn Fn(String)>) -> Vec<gtk4::Widget> {
    let link = {
        let on_tag = Rc::clone(&on_tag);
        move |_: &gtk4::Label, uri: &str| -> glib::Propagation {
            if let Some(t) = markdown::tag_of_uri(uri) {
                on_tag(t.to_string());
            } else {
                let _ = gio::AppInfo::launch_default_for_uri(uri, gio::AppLaunchContext::NONE);
            }
            glib::Propagation::Stop
        }
    };
    let label = |markup: &str| {
        let l = gtk4::Label::builder().use_markup(true).label(markup).wrap(true).wrap_mode(gtk4::pango::WrapMode::WordChar).xalign(0.0).hexpand(true).selectable(true).build();
        l.connect_activate_link(link.clone());
        l
    };
    let mut out: Vec<gtk4::Widget> = vec![];
    for b in markdown::blocks(text) {
        match b {
            Block::Heading(level, s) => {
                let size = match level {
                    1 => "x-large",
                    2 => "large",
                    _ => "medium",
                };
                out.push(label(&format!("<span size=\"{size}\" weight=\"bold\">{}</span>", markdown::inline_pango(&s))).upcast());
            }
            Block::Task { done, text, line } => {
                let row = gtk4::Box::builder().spacing(6).build();
                let check = gtk4::CheckButton::builder().active(done).valign(gtk4::Align::Start).build();
                let t = markdown::inline_pango(&text);
                let l = label(&if done { format!("<s>{t}</s>") } else { t });
                if done {
                    l.add_css_class("dim-label");
                }
                let cb = Rc::clone(&on_toggle);
                check.connect_toggled(move |_| cb(line));
                row.append(&check);
                row.append(&l);
                out.push(row.upcast());
            }
            Block::Bullet { text, indent } => {
                let row = gtk4::Box::builder().spacing(6).margin_start(14 * indent as i32).build();
                row.append(&gtk4::Label::builder().label(if indent > 0 { "\u{25e6}" } else { "\u{2022}" }).valign(gtk4::Align::Start).css_classes(["dim-label"]).build());
                row.append(&label(&markdown::inline_pango(&text)));
                out.push(row.upcast());
            }
            Block::Numbered(n, text) => {
                let row = gtk4::Box::builder().spacing(6).build();
                row.append(&gtk4::Label::builder().label(format!("{n}.")).valign(gtk4::Align::Start).css_classes(["dim-label", "numeric"]).build());
                row.append(&label(&markdown::inline_pango(&text)));
                out.push(row.upcast());
            }
            Block::Quote(text) => {
                let l = label(&markdown::inline_pango(&text));
                l.add_css_class("dim-label");
                l.add_css_class("menew-quote");
                out.push(l.upcast());
            }
            Block::Code(code) => {
                let l = gtk4::Label::builder().label(code).selectable(true).xalign(0.0).hexpand(true).wrap(true).css_classes(["menew-code"]).build();
                out.push(l.upcast());
            }
            Block::Rule => out.push(gtk4::Separator::builder().margin_top(4).margin_bottom(4).build().upcast()),
            Block::Paragraph(s) => out.push(label(&markdown::inline_pango(&s)).upcast()),
            Block::Space => out.push(gtk4::Box::builder().height_request(4).build().upcast()),
        }
    }
    out
}
