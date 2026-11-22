//! Small dialogs shared by the windows (confirmations).

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

/// Ask before doing something that can't be undone (`NSAlert` on the Mac). `on_ok` runs only on
/// the confirm button.
pub fn confirm(parent: Option<&gtk4::Window>, title: &str, body: &str, ok: &str, cancel: &str, destructive: bool, on_ok: impl Fn() + 'static) {
    let d = adw::MessageDialog::builder().heading(title).body(body).modal(true).build();
    if let Some(p) = parent {
        d.set_transient_for(Some(p));
    }
    d.add_response("cancel", cancel);
    d.add_response("ok", ok);
    d.set_response_appearance("ok", if destructive { adw::ResponseAppearance::Destructive } else { adw::ResponseAppearance::Suggested });
    d.set_default_response(Some("cancel"));
    d.set_close_response("cancel");
    d.connect_response(None, move |d, r| {
        if r == "ok" {
            on_ok();
        }
        d.close();
    });
    d.present();
}

/// Ask for one line of text (`{input:Name}` style).
pub fn ask_text(parent: Option<&gtk4::Window>, title: &str, placeholder: &str, ok: &str, cancel: &str, on_ok: impl Fn(String) + 'static) {
    let d = adw::MessageDialog::builder().heading(title).modal(true).build();
    if let Some(p) = parent {
        d.set_transient_for(Some(p));
    }
    let entry = gtk4::Entry::builder().placeholder_text(placeholder).activates_default(true).build();
    d.set_extra_child(Some(&entry));
    d.add_response("cancel", cancel);
    d.add_response("ok", ok);
    d.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
    d.set_default_response(Some("ok"));
    d.set_close_response("cancel");
    d.connect_response(None, move |d, r| {
        if r == "ok" {
            on_ok(entry.text().to_string());
        }
        d.close();
    });
    d.present();
}
