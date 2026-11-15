//! Builds the Menew window with its sections and connects them to the palette commands
//! (`note.*`, `open.*`).

use crate::commands::Commands;
use libadwaita as adw;
use menew_core::notes::{self, NotesStore};
use menew_core::strings::tr;
use menew_core::current_language;
use menew_ui::notes::NotesView;
use menew_ui::window::{MenewWindow, Section};
use std::cell::RefCell;
use std::rc::Rc;

pub struct AppWindow {
    pub window: Rc<MenewWindow>,
    pub notes: Rc<NotesView>,
}

/// Extra sections from the other modules (performance, settings, overview), built before the window.
pub type SectionFactory = Box<dyn FnOnce() -> Section>;

pub fn setup(app: &adw::Application, cmds: &Rc<Commands>, palette: &Rc<menew_ui::PaletteWindow>, before: Vec<SectionFactory>, after: Vec<SectionFactory>) -> Rc<AppWindow> {
    let lang = current_language();
    let store = Rc::new(RefCell::new(NotesStore::open_default()));
    let notes_view = NotesView::new(Rc::clone(&store));
    let mut sections: Vec<Section> = before.into_iter().map(|f| f()).collect();
    let nv = Rc::clone(&notes_view);
    sections.push(Section {
        id: "notes",
        title: tr("note_open_all", lang, &[]),
        icon: "accessories-text-editor-symbolic",
        page: notes_view.root.clone(),
        header: notes_view.header.clone(),
        on_show: Some(Rc::new(move || nv.refresh())),
    });
    sections.extend(after.into_iter().map(|f| f()));
    let window = MenewWindow::new(app, sections);
    let me = Rc::new(AppWindow { window, notes: notes_view });

    // palette items for the notes
    let s = Rc::clone(&store);
    cmds.add_provider("notes", Rc::new(move |raw, lang| notes::palette_items(&s.borrow(), raw, lang)));

    let m = Rc::clone(&me);
    let p = Rc::clone(palette);
    cmds.add_runner(Rc::new(move |cmd| {
        let (name, arg) = cmd.split_once(':').map(|(a, b)| (a, Some(b))).unwrap_or((cmd, None));
        match name {
            "note.new" | "note.newpasswords" => {
                p.hide();
                m.window.show_section("notes");
                m.notes.new_note(name == "note.newpasswords");
                true
            }
            "note.open" => {
                p.hide();
                m.window.show_section("notes");
                if let Some(id) = arg {
                    m.notes.select(id);
                }
                true
            }
            _ if name.starts_with("open.") => {
                p.hide();
                m.window.show_section(&name[5..]);
                true
            }
            _ => false,
        }
    }));
    me
}
