use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    OpenPalette,
    OpenClipboard,
    /// Delete every unpinned clipboard entry.
    ClearHistory,
    /// Pause the clipboard capture until resumed, or resume it.
    TogglePause,
    /// Paste the next item of the paste queue.
    PasteNext,
    /// "Check for Updates…" (or show the update already found).
    CheckUpdates,
    /// Show the window of the update that was found.
    ShowUpdate,
    /// Turn the automatic update check on or off.
    ToggleAutoUpdate,
    /// Capture the screen (area or window) and annotate it.
    Capture,
    /// Capture an area and copy the text in it.
    CaptureText,
    /// Capture an area and copy the QR code in it.
    ScanQr,
    Quit,
}

/// What the tray shows besides the menu: the countdown of the nearest timer or focus session, in
/// the StatusNotifierItem `Title` and `ToolTip` properties. Empty means nothing is running.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrayStatus {
    /// Short text, e.g. "FOCUS 24m · TIMER 9m". Empty when idle.
    pub summary: String,
    /// One line per running timer or focus session (the tooltip body).
    pub lines: Vec<String>,
    /// The clipboard capture is paused.
    pub paused: bool,
    /// Items waiting in the paste queue (0 also when the queue is off).
    pub queue: usize,
    /// Version of an update found and not installed yet: the icon gets a dot and the menu an item.
    pub update: Option<String>,
    /// The automatic update check is on.
    pub auto_update: bool,
}

impl TrayStatus {
    /// The `Title` property: "Menew" when idle, "Menew · FOCUS 24m" while something runs.
    pub fn title(&self) -> String {
        if self.summary.is_empty() {
            "Menew".to_string()
        } else {
            format!("Menew \u{b7} {}", self.summary)
        }
    }
}

#[cfg(target_os = "linux")]
mod linux_impl {
    use super::*;
    use ksni::menu::{CheckmarkItem, MenuItem, StandardItem};
    use ksni::TrayMethods;

    pub struct MenewTray {
        tx: UnboundedSender<TrayCommand>,
        status: TrayStatus,
    }

    impl ksni::Tray for MenewTray {
        fn id(&self) -> String {
            "app.menew.Menew".to_string()
        }

        fn title(&self) -> String {
            self.status.title()
        }

        fn tool_tip(&self) -> ksni::ToolTip {
            ksni::ToolTip {
                icon_name: "menew-symbolic".to_string(),
                icon_pixmap: vec![],
                title: self.status.title(),
                description: self.status.lines.join("\n"),
            }
        }

        fn status(&self) -> ksni::Status {
            // the "gentle reminder": a dot on the icon, no window
            if self.status.update.is_some() {
                ksni::Status::NeedsAttention
            } else {
                ksni::Status::Active
            }
        }

        fn overlay_icon_name(&self) -> String {
            if self.status.update.is_some() { "software-update-available-symbolic".to_string() } else { String::new() }
        }

        fn attention_icon_name(&self) -> String {
            "menew-symbolic".to_string()
        }

        fn icon_name(&self) -> String {
            // Prefer custom app icon if installed in hicolor, fallback to standard symbolic
            "menew-symbolic".to_string()
        }

        fn menu(&self) -> Vec<MenuItem<Self>> {
            let tx_open = self.tx.clone();
            let tx_clip = self.tx.clone();
            let tx_clear = self.tx.clone();
            let tx_pause = self.tx.clone();
            let tx_next = self.tx.clone();
            let tx_quit = self.tx.clone();
            let tx_check = self.tx.clone();
            let tx_show = self.tx.clone();
            let tx_auto = self.tx.clone();
            let tx_cap = self.tx.clone();
            let tx_text = self.tx.clone();
            let tx_qr = self.tx.clone();
            let lang = menew_core::current_language();

            let mut items: Vec<MenuItem<Self>> = vec![];
            if let Some(version) = &self.status.update {
                items.push(
                    StandardItem {
                        label: menew_core::strings::tr("upd_available_tray", lang, &[version]),
                        activate: Box::new(move |_| {
                            info!("Tray: update window requested");
                            let _ = tx_show.send(TrayCommand::ShowUpdate);
                        }),
                        ..Default::default()
                    }
                    .into(),
                );
                items.push(MenuItem::Separator);
            }
            items.extend(vec![
                StandardItem {
                    label: menew_core::t("tray_open_palette").to_string(),
                    activate: Box::new(move |_| {
                        info!("Tray: Open Palette requested");
                        let _ = tx_open.send(TrayCommand::OpenPalette);
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: menew_core::t("tray_clipboard_history").to_string(),
                    activate: Box::new(move |_| {
                        info!("Tray: Clipboard history requested");
                        let _ = tx_clip.send(TrayCommand::OpenClipboard);
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: menew_core::t("tray_clear_history").to_string(),
                    activate: Box::new(move |_| {
                        info!("Tray: Clear history requested");
                        let _ = tx_clear.send(TrayCommand::ClearHistory);
                    }),
                    ..Default::default()
                }
                .into(),
                CheckmarkItem {
                    label: menew_core::t("tray_pause_clip").to_string(),
                    checked: self.status.paused,
                    activate: Box::new(move |_this: &mut Self| {
                        info!("Tray: pause/resume clipboard capture requested");
                        let _ = tx_pause.send(TrayCommand::TogglePause);
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: menew_core::t("act_paste_next").to_string(),
                    enabled: self.status.queue > 0,
                    activate: Box::new(move |_| {
                        info!("Tray: paste next requested");
                        let _ = tx_next.send(TrayCommand::PasteNext);
                    }),
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                // the menu bar items of the Mac: capture the screen, copy text from the screen, read a QR code
                StandardItem {
                    label: menew_core::strings::tr("act_capture", lang, &[]),
                    activate: Box::new(move |_| {
                        info!("Tray: capture requested");
                        let _ = tx_cap.send(TrayCommand::Capture);
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: menew_core::strings::tr("act_ocr", lang, &[]),
                    activate: Box::new(move |_| {
                        info!("Tray: text from the screen requested");
                        let _ = tx_text.send(TrayCommand::CaptureText);
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: menew_core::strings::tr("act_qr_scan", lang, &[]),
                    activate: Box::new(move |_| {
                        info!("Tray: QR code from the screen requested");
                        let _ = tx_qr.send(TrayCommand::ScanQr);
                    }),
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                CheckmarkItem {
                    label: menew_core::t("tray_start_at_login").to_string(),
                    // read the real state: the palette can change it too
                    checked: menew_core::autostart::is_enabled(),
                    activate: Box::new(|_this: &mut Self| {
                        let want = !menew_core::autostart::is_enabled();
                        match menew_core::autostart::set_enabled(want) {
                            Ok(()) => info!("Tray: Start at login set to {}", want),
                            Err(e) => tracing::warn!("Tray: could not change autostart: {}", e),
                        }
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: menew_core::strings::tr("upd_check", lang, &[]),
                    activate: Box::new(move |_| {
                        info!("Tray: check for updates requested");
                        let _ = tx_check.send(TrayCommand::CheckUpdates);
                    }),
                    ..Default::default()
                }
                .into(),
                CheckmarkItem {
                    label: menew_core::strings::tr("upd_auto", lang, &[]),
                    checked: self.status.auto_update,
                    activate: Box::new(move |_this: &mut Self| {
                        info!("Tray: automatic update check toggled");
                        let _ = tx_auto.send(TrayCommand::ToggleAutoUpdate);
                    }),
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                StandardItem {
                    label: menew_core::t("tray_quit").to_string(),
                    activate: Box::new(move |_| {
                        info!("Tray: Quit requested");
                        let _ = tx_quit.send(TrayCommand::Quit);
                    }),
                    ..Default::default()
                }
                .into(),
            ]);
            items
        }
    }

    /// Register the tray, then keep applying the status updates until the channel closes.
    pub async fn start_tray(
        tx: UnboundedSender<TrayCommand>,
        mut status_rx: UnboundedReceiver<TrayStatus>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tray = MenewTray { tx, status: TrayStatus::default() };
        let handle = tray.spawn().await?;
        while let Some(status) = status_rx.recv().await {
            handle.update(|t| t.status = status.clone()).await;
        }
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
mod stub_impl {
    use super::*;

    pub async fn start_tray(
        _tx: UnboundedSender<TrayCommand>,
        _status_rx: UnboundedReceiver<TrayStatus>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Tray stub initialized (non-Linux environment)");
        Ok(())
    }
}

#[cfg(target_os = "linux")]
pub use linux_impl::start_tray;

#[cfg(not(target_os = "linux"))]
pub use stub_impl::start_tray;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_shows_the_countdown() {
        assert_eq!(TrayStatus::default().title(), "Menew");
        let s = TrayStatus { summary: "FOCUS 24m".into(), ..Default::default() };
        assert_eq!(s.title(), "Menew \u{b7} FOCUS 24m");
    }

    #[tokio::test]
    async fn test_tray_command_channel() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        tx.send(TrayCommand::OpenPalette).unwrap();
        assert_eq!(rx.recv().await, Some(TrayCommand::OpenPalette));
    }
}
