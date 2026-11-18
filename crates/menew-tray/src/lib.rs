use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    OpenPalette,
    OpenClipboard,
    OpenMonitor,
    /// Delete every unpinned clipboard entry.
    ClearHistory,
    /// Pause the clipboard capture until resumed, or resume it.
    TogglePause,
    /// Paste the next item of the paste queue.
    PasteNext,
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
    /// The chosen system metrics for the panel, e.g. "CPU 23% · RAM 64%". Empty when none is chosen.
    /// KDE shows the `Title` as text in the panel; GNOME AppIndicator shows only the icon.
    pub metrics: String,
    /// One line per chosen metric (the tooltip body, after the timers).
    pub metrics_lines: Vec<String>,
}

impl TrayStatus {
    /// The `Title` property: "Menew" when idle, "Menew · FOCUS 24m" while something runs.
    pub fn title(&self) -> String {
        let mut parts = vec!["Menew"];
        parts.extend([self.summary.as_str(), self.metrics.as_str()].into_iter().filter(|p| !p.is_empty()));
        parts.join(" \u{b7} ")
    }

    /// The `ToolTip` body: the running timers, then the metrics.
    pub fn tooltip_lines(&self) -> Vec<String> {
        self.lines.iter().chain(&self.metrics_lines).cloned().collect()
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
                description: self.status.tooltip_lines().join("\n"),
            }
        }

        fn icon_name(&self) -> String {
            // Prefer custom app icon if installed in hicolor, fallback to standard symbolic
            "menew-symbolic".to_string()
        }

        fn menu(&self) -> Vec<MenuItem<Self>> {
            let tx_open = self.tx.clone();
            let tx_clip = self.tx.clone();
            let tx_mon = self.tx.clone();
            let tx_clear = self.tx.clone();
            let tx_pause = self.tx.clone();
            let tx_next = self.tx.clone();
            let tx_quit = self.tx.clone();

            vec![
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
                    label: menew_core::t("tray_monitor").to_string(),
                    activate: Box::new(move |_| {
                        info!("Tray: System Monitor requested");
                        let _ = tx_mon.send(TrayCommand::OpenMonitor);
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
            ]
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

    #[test]
    fn title_and_tooltip_carry_the_metrics() {
        let s = TrayStatus {
            metrics: "CPU 23% \u{b7} RAM 64%".into(),
            metrics_lines: vec!["CPU: 23%".into()],
            lines: vec!["FOCUS 24m".into()],
            ..Default::default()
        };
        assert_eq!(s.title(), "Menew \u{b7} CPU 23% \u{b7} RAM 64%");
        assert_eq!(s.tooltip_lines(), vec!["FOCUS 24m".to_string(), "CPU: 23%".to_string()]);
        let both = TrayStatus { summary: "FOCUS 24m".into(), ..s };
        assert_eq!(both.title(), "Menew \u{b7} FOCUS 24m \u{b7} CPU 23% \u{b7} RAM 64%");
    }

    #[tokio::test]
    async fn test_tray_command_channel() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        tx.send(TrayCommand::OpenPalette).unwrap();
        assert_eq!(rx.recv().await, Some(TrayCommand::OpenPalette));
    }
}
