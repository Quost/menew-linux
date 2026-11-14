use std::sync::Arc;
use menew_core::{ClipboardDatabase, ClipboardEntry};
#[allow(unused_imports)]
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct ClipboardService {
    db: ClipboardDatabase,
}

impl ClipboardService {
    pub fn new(db: ClipboardDatabase) -> Self {
        Self { db }
    }

    pub fn handle_new_text(&self, text: &str, mime: &str) -> Option<i64> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }

        // Privacy: ignore content marked by password managers
        if is_likely_secret(trimmed, mime) {
            info!("Skipping clipboard text detected as secret / password hint");
            return None;
        }

        match self.db.save_text(trimmed) {
            Ok(id) => {
                info!("Saved clipboard item (id: {:?}, length: {})", id, trimmed.len());
                id
            }
            Err(e) => {
                error!("Failed to save clipboard item: {}", e);
                None
            }
        }
    }

    pub fn get_recent_history(&self, limit: usize) -> Vec<ClipboardEntry> {
        self.db.get_recent(limit).unwrap_or_default()
    }

    pub fn search_history(&self, query: &str, limit: usize) -> Vec<ClipboardEntry> {
        self.db.search(query, limit).unwrap_or_default()
    }
}

pub fn is_likely_secret(text: &str, mime: &str) -> bool {
    let lower_mime = mime.to_lowercase();
    if lower_mime.contains("password")
        || lower_mime.contains("secret")
        || lower_mime.contains("concealedtype")
        || lower_mime.contains("x-kde-passwordmanagerhint")
    {
        return true;
    }

    // Secondary heuristic: check if marked with internal token
    if text.starts_with("[PASSWORD:") || text.starts_with("[CONCEALED:") {
        return true;
    }

    false
}

#[cfg(target_os = "linux")]
mod linux_listener {
    use super::*;
    use std::path::PathBuf;
    use std::process::{Child, Command};
    use zbus::interface;

    pub struct MenewDbusClipboard {
        service: Arc<ClipboardService>,
    }

    #[interface(name = "org.menew.ClipboardService")]
    impl MenewDbusClipboard {
        /// Called by GNOME Shell extension or KDE watcher when clipboard changes
        async fn push_clipboard_text(&self, text: String, mime: String) -> bool {
            if is_likely_secret(&text, &mime) {
                info!("Ignoring clipboard text tagged with password / secret MIME hint ({})", mime);
                return false;
            }

            info!(
                "Received clipboard text via D-Bus (mime: {}, bytes: {})",
                mime,
                text.len()
            );
            self.service.handle_new_text(&text, &mime).is_some()
        }

        /// Ping to check if Menew daemon is running
        async fn ping(&self) -> String {
            "pong".to_string()
        }
    }

    /// Starts and keeps the session D-Bus connection alive
    pub async fn start_gnome_dbus_listener(
        service: Arc<ClipboardService>,
    ) -> Result<zbus::Connection, Box<dyn std::error::Error + Send + Sync>> {
        let dbus_obj = MenewDbusClipboard { service };

        let conn = zbus::connection::Builder::session()?
            .name("org.menew.ClipboardService")?
            .serve_at("/org/menew/Clipboard", dbus_obj)?
            .build()
            .await?;

        info!("D-Bus service 'org.menew.ClipboardService' registered successfully on session bus");
        Ok(conn)
    }

    /// Owns the `wl-paste --watch` child and kills it on drop, so no orphan
    /// watcher survives Quit or any other exit path.
    pub struct WatcherGuard(Child);

    impl WatcherGuard {
        pub fn new(child: Child) -> Self {
            Self(child)
        }

        pub fn id(&self) -> u32 {
            self.0.id()
        }
    }

    impl Drop for WatcherGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    /// Background listener for KDE Plasma / wlroots compositors supporting data-control via wl-paste
    pub fn start_kde_data_control_listener() -> Option<WatcherGuard> {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("menew"));

        info!("Spawning wl-paste watcher for KDE Plasma (binary-safe, multi-line preserving)...");

        // Executes: wl-paste --type text/plain --watch <menew> __clipboard-push
        // For every copy event, wl-paste pipes the full text to stdin of the child process.
        let child = Command::new("wl-paste")
            .arg("--type")
            .arg("text/plain")
            .arg("--watch")
            .arg(&exe)
            .arg("__clipboard-push")
            .spawn();

        match child {
            Ok(proc) => {
                info!("KDE wl-paste watcher process started (PID: {})", proc.id());
                Some(WatcherGuard::new(proc))
            }
            Err(e) => {
                warn!(
                    "Could not launch wl-paste watcher (expected on GNOME, or install wl-clipboard on KDE): {}",
                    e
                );
                None
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod stub_listener {
    use super::*;

    pub struct DummyConn;

    pub async fn start_gnome_dbus_listener(
        _service: Arc<ClipboardService>,
    ) -> Result<DummyConn, Box<dyn std::error::Error + Send + Sync>> {
        info!("GNOME D-Bus listener stub initialized (non-Linux)");
        Ok(DummyConn)
    }

    pub struct WatcherGuard;

    pub fn start_kde_data_control_listener() -> Option<WatcherGuard> {
        info!("KDE data-control listener stub initialized (non-Linux)");
        None
    }
}

#[cfg(target_os = "linux")]
pub use linux_listener::{start_gnome_dbus_listener, start_kde_data_control_listener, WatcherGuard};

#[cfg(not(target_os = "linux"))]
pub use stub_listener::{start_gnome_dbus_listener, start_kde_data_control_listener, WatcherGuard};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_service_memory() {
        let db = ClipboardDatabase::open_in_memory().unwrap();
        let service = ClipboardService::new(db);

        let id = service.handle_new_text("Sample test text", "text/plain");
        assert!(id.is_some());

        let history = service.get_recent_history(5);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].text_content, "Sample test text");
    }

    #[test]
    fn test_secret_detection() {
        assert!(is_likely_secret("my_pass", "x-kde-passwordManagerHint"));
        assert!(is_likely_secret("my_pass", "application/x-kde-passwordManagerHint"));
        assert!(!is_likely_secret("regular text", "text/plain"));
    }

    #[test]
    fn test_secret_detection_variants() {
        assert!(is_likely_secret("x", "X-KDE-PasswordManagerHint"));
        assert!(is_likely_secret("x", "org.nspasteboard.ConcealedType"));
        assert!(is_likely_secret("[PASSWORD:abc]", "text/plain"));
        assert!(is_likely_secret("[CONCEALED:abc]", "text/plain"));
        assert!(!is_likely_secret("hello", "text/html"));
    }

    #[test]
    fn test_secret_not_saved() {
        let db = ClipboardDatabase::open_in_memory().unwrap();
        let service = ClipboardService::new(db);
        assert!(service
            .handle_new_text("hunter2", "x-kde-passwordManagerHint")
            .is_none());
        assert!(service.get_recent_history(5).is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_watcher_guard_kills_child_on_drop() {
        let child = std::process::Command::new("sleep").arg("60").spawn().unwrap();
        let guard = WatcherGuard::new(child);
        let pid = guard.id();
        let alive = |pid: u32| {
            std::process::Command::new("kill")
                .args(["-0", &pid.to_string()])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        };
        assert!(alive(pid), "child should be running before drop");
        drop(guard);
        assert!(!alive(pid), "child must be gone after drop");
    }
}
