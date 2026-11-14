use tokio::sync::mpsc::UnboundedSender;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    OpenPalette,
    OpenClipboard,
    /// Delete every unpinned clipboard entry.
    ClearHistory,
    Quit,
}

#[cfg(target_os = "linux")]
mod linux_impl {
    use super::*;
    use ksni::menu::{CheckmarkItem, MenuItem, StandardItem};
    use ksni::TrayMethods;

    pub struct MenewTray {
        tx: UnboundedSender<TrayCommand>,
        /// Mirrors the presence of ~/.config/autostart/menew.desktop
        autostart: bool,
    }

    impl ksni::Tray for MenewTray {
        fn id(&self) -> String {
            "app.menew.Menew".to_string()
        }

        fn title(&self) -> String {
            "Menew".to_string()
        }

        fn icon_name(&self) -> String {
            // Prefer custom app icon if installed in hicolor, fallback to standard symbolic
            "menew-symbolic".to_string()
        }

        fn menu(&self) -> Vec<MenuItem<Self>> {
            let tx_open = self.tx.clone();
            let tx_clip = self.tx.clone();
            let tx_clear = self.tx.clone();
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
                    label: menew_core::t("tray_clear_history").to_string(),
                    activate: Box::new(move |_| {
                        info!("Tray: Clear history requested");
                        let _ = tx_clear.send(TrayCommand::ClearHistory);
                    }),
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                CheckmarkItem {
                    label: menew_core::t("tray_start_at_login").to_string(),
                    checked: self.autostart,
                    activate: Box::new(|this: &mut Self| {
                        let want = !this.autostart;
                        match menew_core::autostart::set_enabled(want) {
                            Ok(()) => {
                                this.autostart = want;
                                info!("Tray: Start at login set to {}", want);
                            }
                            Err(e) => {
                                tracing::warn!("Tray: could not change autostart: {}", e);
                                this.autostart = menew_core::autostart::is_enabled();
                            }
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

    pub async fn start_tray(
        tx: UnboundedSender<TrayCommand>,
    ) -> Result<ksni::Handle<MenewTray>, Box<dyn std::error::Error + Send + Sync>> {
        let tray = MenewTray { tx, autostart: menew_core::autostart::is_enabled() };
        let handle = tray.spawn().await?;
        Ok(handle)
    }
}

#[cfg(not(target_os = "linux"))]
mod stub_impl {
    use super::*;

    pub struct DummyHandle;

    pub async fn start_tray(
        _tx: UnboundedSender<TrayCommand>,
    ) -> Result<DummyHandle, Box<dyn std::error::Error + Send + Sync>> {
        info!("Tray stub initialized (non-Linux environment)");
        Ok(DummyHandle)
    }
}

#[cfg(target_os = "linux")]
pub use linux_impl::start_tray;

#[cfg(not(target_os = "linux"))]
pub use stub_impl::start_tray;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tray_command_channel() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        tx.send(TrayCommand::OpenPalette).unwrap();
        assert_eq!(rx.recv().await, Some(TrayCommand::OpenPalette));
    }
}
