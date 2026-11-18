use tokio::sync::mpsc::UnboundedSender;
use tracing::{info, warn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortcutEvent {
    TogglePalette,
    ToggleClipboard,
    /// Paste the next item of the paste queue.
    PasteNext,
    /// Capture the screen (area or window) and annotate it (the Mac's Ctrl+Option+Cmd+S).
    Capture,
    /// Capture an area and copy the text in it (the Mac's Ctrl+Option+Cmd+T).
    CaptureText,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortalShortcutStatus {
    Success(Vec<String>),
    Unsupported(String),
}

#[cfg(target_os = "linux")]
mod linux_impl {
    use super::*;
    use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
    use futures_util::StreamExt;

    pub async fn register_and_listen(
        tx: UnboundedSender<ShortcutEvent>,
    ) -> PortalShortcutStatus {
        info!("Attempting to connect to org.freedesktop.portal.GlobalShortcuts...");

        let global_shortcuts = match GlobalShortcuts::new().await {
            Ok(gs) => gs,
            Err(e) => {
                let err_msg = format!("Failed to connect to GlobalShortcuts portal: {}", e);
                display_portal_fallback_message(&err_msg);
                return PortalShortcutStatus::Unsupported(err_msg);
            }
        };

        let session = match global_shortcuts.create_session().await {
            Ok(s) => s,
            Err(e) => {
                let err_msg = format!("Failed to create GlobalShortcuts session: {}", e);
                display_portal_fallback_message(&err_msg);
                return PortalShortcutStatus::Unsupported(err_msg);
            }
        };

        // Obtain activation event stream before binding
        let mut activated_stream = match global_shortcuts.receive_activated().await {
            Ok(stream) => stream,
            Err(e) => {
                let err_msg = format!("Failed to subscribe to activation stream: {}", e);
                display_portal_fallback_message(&err_msg);
                return PortalShortcutStatus::Unsupported(err_msg);
            }
        };

        // Ctrl+Alt+Space as default (avoids GNOME's Super+Space input switch)
        let shortcut = NewShortcut::new("toggle_palette", "Open Menew Command Palette")
            .preferred_trigger(Some("Control+Alt+Space"));
        let clipboard_shortcut =
            NewShortcut::new("toggle_clipboard", menew_core::t("shortcut_clipboard_desc"))
                .preferred_trigger(Some("Control+Alt+V"));
        let paste_next_shortcut =
            NewShortcut::new("paste_next", menew_core::t("shortcut_paste_next_desc"))
                .preferred_trigger(Some("Control+Alt+N"));

        let lang = menew_core::current_language();
        let capture_shortcut = NewShortcut::new("capture", menew_core::strings::tr("shortcut_capture_desc", lang, &[]))
            .preferred_trigger(Some("Control+Alt+Shift+S"));
        let capture_text_shortcut = NewShortcut::new("capture_text", menew_core::strings::tr("shortcut_capture_text_desc", lang, &[]))
            .preferred_trigger(Some("Control+Alt+Shift+T"));

        let response = match global_shortcuts
            .bind_shortcuts(
                &session,
                &[shortcut, clipboard_shortcut, paste_next_shortcut, capture_shortcut, capture_text_shortcut],
                Default::default(),
            )
            .await
        {
            Ok(r) => match r.response() {
                Ok(resp) => resp,
                Err(e) => {
                    let err_msg = format!("User or system rejected shortcut binding: {}", e);
                    display_portal_fallback_message(&err_msg);
                    return PortalShortcutStatus::Unsupported(err_msg);
                }
            },
            Err(e) => {
                let err_msg = format!("bind_shortcuts request failed: {}", e);
                display_portal_fallback_message(&err_msg);
                return PortalShortcutStatus::Unsupported(err_msg);
            }
        };

        let bound_keys: Vec<String> = response
            .shortcuts()
            .iter()
            .map(|s| s.id().to_string())
            .collect();

        info!("Global shortcut successfully bound via portal: {:?}", bound_keys);

        // Spawn event listener task moving the activation stream
        tokio::spawn(async move {
            info!("Listening for GlobalShortcuts activation events...");
            while let Some(event) = activated_stream.next().await {
                let shortcut_id = event.shortcut_id();
                info!("Portal shortcut triggered: {}", shortcut_id);
                match shortcut_id {
                    "toggle_palette" => {
                        let _ = tx.send(ShortcutEvent::TogglePalette);
                    }
                    "toggle_clipboard" => {
                        let _ = tx.send(ShortcutEvent::ToggleClipboard);
                    }
                    "paste_next" => {
                        let _ = tx.send(ShortcutEvent::PasteNext);
                    }
                    "capture" => {
                        let _ = tx.send(ShortcutEvent::Capture);
                    }
                    "capture_text" => {
                        let _ = tx.send(ShortcutEvent::CaptureText);
                    }
                    _ => {}
                }
            }
        });

        PortalShortcutStatus::Success(bound_keys)
    }

    fn display_portal_fallback_message(reason: &str) {
        warn!("{}", reason);
        eprintln!();
        eprintln!("====================================================================");
        eprintln!(" [Menew] {}", menew_core::t("portal_unavailable_title"));
        eprintln!("--------------------------------------------------------------------");
        eprintln!(" {}", menew_core::t("portal_unavailable_msg"));
        eprintln!();
        eprintln!(" Technical details: {}", reason);
        eprintln!();
        eprintln!(" Desktop environment guidance:");
        eprintln!(" - KDE Plasma 5.27+ and KDE Plasma 6 support this portal natively.");
        eprintln!(" - GNOME 48+ supports this portal natively.");
        eprintln!(" - On GNOME 45-47, configure a custom shortcut in GNOME Settings:");
        eprintln!("     Settings -> Keyboard -> View and Customize Shortcuts -> Custom");
        eprintln!("     Name:     Menew Palette");
        eprintln!("     Command:  menew toggle");
        eprintln!("     Shortcut: Ctrl+Alt+Space");
        eprintln!("   And for the clipboard history, a second custom shortcut:");
        eprintln!("     Name:     Menew Clipboard");
        eprintln!("     Command:  menew clipboard");
        eprintln!("     Shortcut: Ctrl+Alt+V");
        eprintln!("   And for the paste queue, a third one:");
        eprintln!("     Name:     Menew Paste Next");
        eprintln!("     Command:  menew paste-next");
        eprintln!("     Shortcut: Ctrl+Alt+N");
        eprintln!("   And for the screen, two more:");
        eprintln!("     Name:     Menew Capture");
        eprintln!("     Command:  menew capture");
        eprintln!("     Shortcut: Ctrl+Alt+Shift+S");
        eprintln!("     Name:     Menew Text from Screen");
        eprintln!("     Command:  menew capture-text");
        eprintln!("     Shortcut: Ctrl+Alt+Shift+T");
        eprintln!("====================================================================");
        eprintln!();
    }
}

#[cfg(not(target_os = "linux"))]
mod stub_impl {
    use super::*;

    pub async fn register_and_listen(
        _tx: UnboundedSender<ShortcutEvent>,
    ) -> PortalShortcutStatus {
        info!("GlobalShortcuts stub initialized (non-Linux environment)");
        PortalShortcutStatus::Unsupported("GlobalShortcuts portal only available on Linux".to_string())
    }
}

#[cfg(target_os = "linux")]
pub use linux_impl::register_and_listen;

#[cfg(not(target_os = "linux"))]
pub use stub_impl::register_and_listen;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shortcuts_channel() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        tx.send(ShortcutEvent::TogglePalette).unwrap();
        tx.send(ShortcutEvent::ToggleClipboard).unwrap();
        tx.send(ShortcutEvent::PasteNext).unwrap();
        assert_eq!(rx.recv().await, Some(ShortcutEvent::TogglePalette));
        assert_eq!(rx.recv().await, Some(ShortcutEvent::ToggleClipboard));
        assert_eq!(rx.recv().await, Some(ShortcutEvent::PasteNext));
    }
}
