use tokio::sync::mpsc::UnboundedSender;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    OpenPalette,
    Quit,
}

#[cfg(target_os = "linux")]
mod linux_impl {
    use super::*;
    use ksni::menu::{MenuItem, StandardItem};
    use ksni::TrayMethods;

    pub struct MenewTray {
        tx: UnboundedSender<TrayCommand>,
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
        let tray = MenewTray { tx };
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
