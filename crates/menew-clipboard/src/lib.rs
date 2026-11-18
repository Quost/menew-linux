#[cfg(target_os = "linux")]
pub mod secrets;

use menew_core::clip_policy::{self, Capture, PauseState, MAX_BYTES};
use menew_core::{ClipKind, ClipboardDatabase, ClipboardEntry, Config};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::UnboundedSender;
#[allow(unused_imports)]
use tracing::{error, info, warn};

/// What the service tells the daemon after it kept something.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipEvent {
    /// An entry was saved (or moved to the top). `own` is true when Menew itself put it on the clipboard
    /// a moment ago (picking an item), which the paste queue must not count as a new copy.
    Saved { id: i64, own: bool },
}

#[derive(Clone)]
pub struct ClipboardService {
    db: ClipboardDatabase,
    pause: Arc<Mutex<PauseState>>,
    config: Option<Config>,
    events: Option<UnboundedSender<ClipEvent>>,
    own: Arc<Mutex<Option<(String, Instant, bool)>>>,
    ocr_running: Arc<AtomicBool>,
}

pub use menew_core::clip_policy::own_key;

fn now_secs() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// How long after Menew wrote the clipboard a copy of the same content still counts as its own.
const OWN_WINDOW: Duration = Duration::from_secs(5);

impl ClipboardService {
    pub fn new(db: ClipboardDatabase) -> Self {
        Self {
            db,
            pause: Arc::new(Mutex::new(PauseState::load())),
            config: None,
            events: None,
            own: Arc::new(Mutex::new(None)),
            ocr_running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The pause state is shared with whoever controls the watcher.
    pub fn with_pause(mut self, pause: Arc<Mutex<PauseState>>) -> Self {
        self.pause = pause;
        self
    }

    /// Use this configuration instead of reading `config.json` on every copy (tests).
    pub fn with_config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    pub fn with_events(mut self, tx: UnboundedSender<ClipEvent>) -> Self {
        self.events = Some(tx);
        self
    }

    pub fn database(&self) -> &ClipboardDatabase {
        &self.db
    }

    pub fn pause_state(&self) -> Arc<Mutex<PauseState>> {
        Arc::clone(&self.pause)
    }

    fn config(&self) -> Config {
        self.config.clone().unwrap_or_else(Config::load)
    }

    /// Whether a copy made now by this app may be read at all (asked by the GNOME extension before it reads the
    /// clipboard, and by the KDE helper). Logs why not, so that a pause can be seen in the log.
    pub fn should_capture(&self, app_id: &str, app_name: &str) -> bool {
        let ignored = self.config().clipboard_ignored_apps;
        let pause = *self.pause.lock().unwrap();
        let now = now_secs();
        let mut verdict = clip_policy::should_capture(&pause, now, &ignored, Some(app_id));
        if verdict == Capture::Yes {
            verdict = clip_policy::should_capture(&pause, now, &ignored, Some(app_name));
        }
        match verdict {
            Capture::Yes => true,
            Capture::Paused => {
                info!("Capture paused: the clipboard is not read");
                false
            }
            Capture::Ignored => {
                info!("Capture skipped: copied from an ignored app ({})", if app_id.is_empty() { app_name } else { app_id });
                false
            }
        }
    }

    pub fn pause(&self, minutes: Option<u32>) {
        let mut p = self.pause.lock().unwrap();
        p.pause(minutes, now_secs());
        let _ = p.save();
        info!("Clipboard capture paused ({})", minutes.map(|m| format!("{m} min")).unwrap_or_else(|| "until resumed".into()));
    }

    pub fn resume(&self) {
        let mut p = self.pause.lock().unwrap();
        p.resume();
        let _ = p.save();
        info!("Clipboard capture resumed");
    }

    pub fn is_paused(&self) -> bool {
        self.pause.lock().unwrap().is_paused(now_secs())
    }

    /// Menew is about to put this content on the clipboard (`key` is `t:text`, `f:paths` or `i:file`, see
    /// [`own_key`]). When the watcher sees it come back it is not something new: with `skip` it is not kept
    /// at all (a snippet, the next item of the queue), without it the entry is only moved to the top
    /// (picking an item of the history), and the queue does not count it.
    pub fn expect_own_write(&self, key: String, skip: bool) {
        *self.own.lock().unwrap() = Some((key, Instant::now(), skip));
    }

    /// `Some(skip)` when this is the content Menew just wrote.
    fn take_own(&self, key: &str) -> Option<bool> {
        let mut own = self.own.lock().unwrap();
        match own.as_ref() {
            Some((k, at, skip)) if k == key && at.elapsed() < OWN_WINDOW => {
                let skip = *skip;
                *own = None;
                Some(skip)
            }
            _ => None,
        }
    }

    pub fn handle_new_text(&self, text: &str, mime: &str) -> Option<i64> {
        self.handle_data(mime, text.as_bytes(), "", "", &[mime.to_string()])
    }

    /// Keep what a copy holds: `mime` is the type the bytes are in, `types` everything the copy offers.
    /// Text goes into the history, a list of files too, an image becomes a file in the images folder.
    /// Nothing is kept when capture is paused, the app is ignored, a type says it is a password or
    /// the data is bigger than 20 MB. Returns the id of the entry.
    pub fn handle_data(&self, mime: &str, data: &[u8], app_id: &str, app_name: &str, types: &[String]) -> Option<i64> {
        if !self.should_capture(app_id, app_name) {
            return None;
        }
        // privacy: content marked by password managers, whatever it is
        if clip_policy::has_secret_type(types) || clip_policy::is_secret_type(mime) {
            info!("Skipping clipboard content marked as secret / password hint ({})", mime);
            return None;
        }
        if data.len() > MAX_BYTES {
            warn!("Skipping a copy of {} bytes (the limit is {} bytes)", data.len(), MAX_BYTES);
            return None;
        }
        let source = if app_name.is_empty() { app_id } else { app_name };
        let source = (!source.is_empty()).then_some(source);
        let lower = mime.to_ascii_lowercase();

        // what is it, and what would it be called (the same key Menew uses when it writes the clipboard itself)
        enum Item {
            Files(String),
            Image(String),
            Text(String),
        }
        let item = if lower == "text/uri-list" {
            let paths = clip_policy::parse_uri_list(&String::from_utf8_lossy(data));
            if paths.is_empty() {
                return None;
            }
            Item::Files(paths.join("\n"))
        } else if lower.starts_with("image/") {
            Item::Image(clip_policy::image_file_name(data, &lower))
        } else {
            let text = String::from_utf8_lossy(data).into_owned();
            if text.trim().is_empty() {
                return None;
            }
            // secondary heuristic: marked with an internal token
            if text.trim().starts_with("[PASSWORD:") || text.trim().starts_with("[CONCEALED:") {
                info!("Skipping clipboard text detected as secret / password hint");
                return None;
            }
            Item::Text(text.trim().to_string())
        };
        let key = match &item {
            Item::Files(t) => own_key(ClipKind::File, t),
            Item::Image(n) => own_key(ClipKind::Image, n),
            Item::Text(t) => own_key(ClipKind::Text, t),
        };
        let own = self.take_own(&key);
        if own == Some(true) {
            info!("Clipboard change made by Menew itself (not kept)");
            return None;
        }
        let own = own.is_some();

        let saved = match &item {
            Item::Files(text) => self.db.save_files(text, source),
            Item::Image(name) => {
                if let Err(e) = self.write_image(name, data) {
                    warn!("Could not keep the copied image: {}", e);
                    return None;
                }
                let (w, h) = clip_policy::image_size(data).unwrap_or((0, 0));
                self.db.save_image(name, w, h, source)
            }
            Item::Text(text) => self.db.save_text_from(text, source),
        };

        match saved {
            Ok(Some(id)) => {
                info!("Saved clipboard item (id: {}, type: {}, bytes: {})", id, mime, data.len());
                self.prune();
                if let Some(tx) = &self.events {
                    let _ = tx.send(ClipEvent::Saved { id, own });
                }
                if lower.starts_with("image/") {
                    self.queue_ocr();
                }
                Some(id)
            }
            Ok(None) => None,
            Err(e) => {
                error!("Failed to save clipboard item: {}", e);
                None
            }
        }
    }

    fn write_image(&self, name: &str, data: &[u8]) -> std::io::Result<()> {
        let dir = self.db.images_dir().ok_or_else(|| std::io::Error::other("no images folder"))?;
        std::fs::create_dir_all(dir)?;
        let path = dir.join(name);
        if path.exists() {
            return Ok(());
        }
        let tmp = dir.join(format!("{name}.part"));
        std::fs::write(&tmp, data)?;
        std::fs::rename(&tmp, &path)
    }

    /// Keep the history inside its limits (`maxItems`, `autoDeleteDays`).
    pub fn prune(&self) {
        let cfg = self.config();
        match self.db.prune(cfg.clipboard_max_items, cfg.clipboard_auto_delete_days) {
            Ok(0) => {}
            Ok(n) => info!("History limit: removed {} old entries", n),
            Err(e) => warn!("Could not apply the history limit: {}", e),
        }
    }

    /// Read the text of the images that were not read yet, one at a time and without hurry (a second
    /// between images), newest first, in a background thread. Without tesseract nothing is read and the
    /// images stay marked as not read, so installing it later indexes them.
    pub fn queue_ocr(&self) {
        if !self.config().clipboard_ocr {
            return;
        }
        if self.ocr_running.swap(true, Ordering::SeqCst) {
            return;
        }
        let db = self.db.clone();
        let running = Arc::clone(&self.ocr_running);
        std::thread::spawn(move || {
            while let Some(entry) = db.images_without_text().ok().and_then(|v| v.into_iter().next()) {
                let Some(path) = db.image_path(&entry) else { break };
                match menew_core::ocr::recognize(&path) {
                    Ok(text) => {
                        info!("OCR: read {} characters from image {} (entry {})", text.chars().count(), entry.image_file.as_deref().unwrap_or("?"), entry.id);
                        let _ = db.set_ocr_text(entry.id, &text);
                    }
                    Err(menew_core::ocr::OcrError::Missing) => {
                        info!("OCR skipped: {} (the images are kept, their text is not searched)", menew_core::ocr::OcrError::Missing);
                        break;
                    }
                    Err(e) => {
                        warn!("OCR failed for entry {}: {}", entry.id, e);
                        // do not try the same image forever
                        let _ = db.set_ocr_text(entry.id, "");
                    }
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            running.store(false, Ordering::SeqCst);
        });
    }

    pub fn get_recent_history(&self, limit: usize) -> Vec<ClipboardEntry> {
        self.db.get_recent(limit).unwrap_or_default()
    }

    pub fn search_history(&self, query: &str, limit: usize) -> Vec<ClipboardEntry> {
        self.db.search(query, limit).unwrap_or_default()
    }
}

/// Kept for the callers of the first alphas: true when this type or text says "password".
pub fn is_likely_secret(text: &str, mime: &str) -> bool {
    if clip_policy::is_secret_type(mime) {
        return true;
    }

    // Secondary heuristic: check if marked with internal token
    text.starts_with("[PASSWORD:") || text.starts_with("[CONCEALED:")
}

#[cfg(target_os = "linux")]
mod linux_listener {
    use super::*;
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};
    use zbus::interface;

    pub struct MenewDbusClipboard {
        service: Arc<ClipboardService>,
    }

    #[interface(name = "org.menew.ClipboardService")]
    impl MenewDbusClipboard {
        /// Called by older versions of the GNOME Shell extension and by tests: a copied text.
        async fn push_clipboard_text(&self, text: String, mime: String) -> bool {
            if is_likely_secret(&text, &mime) {
                info!("Ignoring clipboard text tagged with password / secret MIME hint ({})", mime);
                return false;
            }

            info!("Received clipboard text via D-Bus (mime: {}, bytes: {})", mime, text.len());
            self.service.handle_new_text(&text, &mime).is_some()
        }

        /// A copy of any type: `mime` is the type of `data`, `types` everything the copy offers, `app_id`
        /// the application id or window class that copied it (empty when unknown).
        async fn push_clipboard_data(&self, mime: String, data: Vec<u8>, app_id: String, app_name: String, types: Vec<String>) -> bool {
            info!("Received clipboard data via D-Bus (mime: {}, bytes: {}, app: {})", mime, data.len(), if app_id.is_empty() { "?" } else { &app_id });
            let service = Arc::clone(&self.service);
            tokio::task::spawn_blocking(move || service.handle_data(&mime, &data, &app_id, &app_name, &types).is_some()).await.unwrap_or(false)
        }

        /// Asked before the clipboard is read: false while capture is paused or the app is ignored.
        async fn should_capture(&self, app_id: String, app_name: String) -> bool {
            self.service.should_capture(&app_id, &app_name)
        }

        /// Ping to check if Menew daemon is running
        async fn ping(&self) -> String {
            "pong".to_string()
        }
    }

    /// Starts and keeps the session D-Bus connection alive
    pub async fn start_gnome_dbus_listener(service: Arc<ClipboardService>) -> Result<zbus::Connection, Box<dyn std::error::Error + Send + Sync>> {
        let dbus_obj = MenewDbusClipboard { service };

        let conn = zbus::connection::Builder::session()?.name("org.menew.ClipboardService")?.serve_at("/org/menew/Clipboard", dbus_obj)?.build().await?;

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

        // Executes: wl-paste --watch <menew> __clipboard-push
        // For every copy event wl-paste runs the helper, which asks which types the copy offers and
        // reads the right one (files, then text, then image).
        let child = Command::new("wl-paste").arg("--watch").arg(&exe).arg("__clipboard-push").spawn();

        match child {
            Ok(proc) => {
                info!("KDE wl-paste watcher process started (PID: {})", proc.id());
                Some(WatcherGuard::new(proc))
            }
            Err(e) => {
                warn!("Could not launch wl-paste watcher (expected on GNOME, or install wl-clipboard on KDE): {}", e);
                None
            }
        }
    }

    /// Starts and stops the watcher with the pause: while paused there is no `wl-paste` process, so
    /// nothing at all is read from the clipboard.
    pub struct WatcherControl {
        guard: Option<WatcherGuard>,
        started: bool,
        pause: Arc<Mutex<PauseState>>,
    }

    impl WatcherControl {
        pub fn new(pause: Arc<Mutex<PauseState>>) -> Self {
            Self { guard: None, started: false, pause }
        }

        /// Bring the watcher in line with the pause (call it when the pause changes and every few seconds,
        /// to catch the end of a timed pause).
        pub fn sync(&mut self) {
            let paused = self.pause.lock().unwrap().is_paused(now_secs());
            if paused && self.started {
                info!("Capture paused: stopping the wl-paste watcher (nothing is read while paused)");
                self.guard = None;
                self.started = false;
            } else if !paused && !self.started {
                self.guard = start_kde_data_control_listener();
                self.started = true;
            }
        }

        pub fn stop(&mut self) {
            self.guard = None;
            self.started = false;
        }

        pub fn watcher_pid(&self) -> Option<u32> {
            self.guard.as_ref().map(WatcherGuard::id)
        }
    }

    /// What runs for every copy that `wl-paste --watch` reports (`menew __clipboard-push`).
    pub fn run_push_helper() -> Result<(), Box<dyn std::error::Error>> {
        use std::io::Read;
        // wl-paste hands the clipboard to this process on stdin: take it and let it go, the right type is read below
        let mut sink = [0u8; 65536];
        let mut stdin = std::io::stdin();
        while stdin.read(&mut sink).map(|n| n > 0).unwrap_or(false) {}

        // paused: nothing is read (the watcher is normally stopped; this covers a copy in flight)
        if PauseState::load().is_paused(now_secs()) {
            info!("KDE watcher: capture is paused, the copy is not read");
            return Ok(());
        }

        let types = list_types();
        let candidates = match clip_policy::plan(&types) {
            Ok(c) => c,
            Err(clip_policy::Skip::Secret) => {
                info!("KDE watcher: ignoring copy marked as secret/password hint");
                return Ok(());
            }
        };

        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        rt.block_on(async {
            let Ok(conn) = zbus::Connection::session().await else { return };
            for c in candidates {
                let mime = match &c {
                    clip_policy::Candidate::Files(t) | clip_policy::Candidate::Text(t) | clip_policy::Candidate::Image(t) => t.clone(),
                };
                let Some(data) = fetch(&mime) else { continue };
                let reply = conn
                    .call_method(
                        Some("org.menew.ClipboardService"),
                        "/org/menew/Clipboard",
                        Some("org.menew.ClipboardService"),
                        "PushClipboardData",
                        &(mime.as_str(), data.as_slice(), "", "", types.clone()),
                    )
                    .await;
                let kept = reply.ok().and_then(|m| m.body().deserialize::<bool>().ok()).unwrap_or(false);
                if kept {
                    break;
                }
            }
        });
        Ok(())
    }

    fn list_types() -> Vec<String> {
        Command::new("wl-paste")
            .arg("--list-types")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
            .unwrap_or_default()
    }

    /// The clipboard in one type, at most 20 MB (a bigger copy is dropped and the reader stopped).
    fn fetch(mime: &str) -> Option<Vec<u8>> {
        use std::io::Read;
        let mut child = Command::new("wl-paste").arg("--no-newline").arg("--type").arg(mime).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
        let mut out = child.stdout.take()?;
        let mut data = Vec::new();
        let read = (&mut out).take(MAX_BYTES as u64 + 1).read_to_end(&mut data);
        if data.len() > MAX_BYTES {
            let _ = child.kill();
            let _ = child.wait();
            warn!("KDE watcher: a copy of {} is bigger than {} bytes, not kept", mime, MAX_BYTES);
            return None;
        }
        let status = child.wait().ok()?;
        (read.is_ok() && status.success()).then_some(data)
    }
}

#[cfg(not(target_os = "linux"))]
mod stub_listener {
    use super::*;

    pub struct DummyConn;

    pub async fn start_gnome_dbus_listener(_service: Arc<ClipboardService>) -> Result<DummyConn, Box<dyn std::error::Error + Send + Sync>> {
        info!("GNOME D-Bus listener stub initialized (non-Linux)");
        Ok(DummyConn)
    }

    pub struct WatcherGuard;

    pub fn start_kde_data_control_listener() -> Option<WatcherGuard> {
        info!("KDE data-control listener stub initialized (non-Linux)");
        None
    }

    pub struct WatcherControl;

    impl WatcherControl {
        pub fn new(_pause: Arc<Mutex<PauseState>>) -> Self {
            Self
        }
        pub fn sync(&mut self) {}
        pub fn stop(&mut self) {}
        pub fn watcher_pid(&self) -> Option<u32> {
            None
        }
    }

    pub fn run_push_helper() -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
pub use linux_listener::{run_push_helper, start_gnome_dbus_listener, start_kde_data_control_listener, WatcherControl, WatcherGuard};

#[cfg(not(target_os = "linux"))]
pub use stub_listener::{run_push_helper, start_gnome_dbus_listener, start_kde_data_control_listener, WatcherControl, WatcherGuard};

#[cfg(test)]
mod tests {
    use super::*;


    fn png(w: u32, h: u32, extra: u8) -> Vec<u8> {
        let mut v = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, b'I', b'H', b'D', b'R'];
        v.extend_from_slice(&w.to_be_bytes());
        v.extend_from_slice(&h.to_be_bytes());
        v.push(extra);
        v
    }

    fn service(dir: &std::path::Path) -> ClipboardService {
        let db = ClipboardDatabase::open_in_memory().unwrap().with_images_dir(dir.to_path_buf());
        ClipboardService::new(db).with_pause(Arc::new(Mutex::new(PauseState::default()))).with_config(Config { clipboard_ocr: false, ..Config::default() })
    }

    fn t(types: &[&str]) -> Vec<String> {
        types.iter().map(|s| s.to_string()).collect()
    }

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
        assert!(service.handle_new_text("hunter2", "x-kde-passwordManagerHint").is_none());
        assert!(service.get_recent_history(5).is_empty());
    }

    #[test]
    fn an_image_is_kept_as_a_file_and_deduplicated() {
        let dir = tempfile::tempdir().unwrap();
        let s = service(dir.path());
        let data = png(640, 480, 1);
        let id = s.handle_data("image/png", &data, "", "", &t(&["image/png"])).unwrap();
        let again = s.handle_data("image/png", &data, "", "", &t(&["image/png"])).unwrap();
        assert_eq!(id, again);
        let other = s.handle_data("image/png", &png(640, 480, 2), "", "", &t(&["image/png"])).unwrap();
        assert_ne!(id, other);
        let e = s.database().get(id).unwrap().unwrap();
        assert_eq!(e.kind, ClipKind::Image);
        assert_eq!(e.text_content, "Imagem 640\u{d7}480");
        let path = s.database().image_path(&e).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), data);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
        // clearing the history removes the files
        s.database().clear_unpinned().unwrap();
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn an_image_with_a_password_hint_is_not_saved_at_all() {
        let dir = tempfile::tempdir().unwrap();
        let s = service(dir.path());
        let data = png(10, 10, 3);
        assert!(s.handle_data("image/png", &data, "", "", &t(&["image/png", "x-kde-passwordManagerHint"])).is_none());
        assert!(s.get_recent_history(5).is_empty());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0, "no file was written either");
    }

    #[test]
    fn files_and_text_and_limits() {
        let dir = tempfile::tempdir().unwrap();
        let s = service(dir.path());
        let id = s.handle_data("text/uri-list", b"file:///home/u/a%20b.txt\r\nfile:///tmp/c.png\r\n", "", "", &t(&["text/uri-list"])).unwrap();
        let e = s.database().get(id).unwrap().unwrap();
        assert_eq!(e.kind, ClipKind::File);
        assert_eq!(e.text_content, "/home/u/a b.txt\n/tmp/c.png");
        assert!(s.handle_data("text/uri-list", b"https://example.com\n", "", "", &[]).is_none());
        assert!(s.handle_data("text/plain", b"   \n", "", "", &[]).is_none());
        let big = vec![b'a'; MAX_BYTES + 1];
        assert!(s.handle_data("text/plain", &big, "", "", &[]).is_none());
        assert!(s.handle_data("image/png", &big, "", "", &[]).is_none());
    }

    #[test]
    fn pause_and_ignored_apps_keep_everything_out() {
        let dir = tempfile::tempdir().unwrap();
        let db = ClipboardDatabase::open_in_memory().unwrap().with_images_dir(dir.path().to_path_buf());
        let cfg = Config { clipboard_ocr: false, clipboard_ignored_apps: vec!["org.keepassxc.KeePassXC".into()], ..Config::default() };
        let pause = Arc::new(Mutex::new(PauseState::default()));
        let s = ClipboardService::new(db).with_pause(Arc::clone(&pause)).with_config(cfg);
        assert!(s.handle_data("text/plain", b"from keepass", "org.keepassxc.KeePassXC", "KeePassXC", &[]).is_none());
        assert!(s.handle_data("text/plain", b"from firefox", "firefox", "Firefox", &[]).is_some());
        pause.lock().unwrap().pause(None, now_secs());
        assert!(!s.should_capture("firefox", "Firefox"));
        assert!(s.handle_data("image/png", &png(5, 5, 4), "", "", &[]).is_none());
        assert_eq!(s.get_recent_history(10).len(), 1);
        pause.lock().unwrap().resume();
        assert!(s.should_capture("firefox", "Firefox"));
        assert_eq!(s.database().get_recent(1).unwrap()[0].source_app.as_deref(), Some("Firefox"));
    }

    #[test]
    fn a_copy_menew_made_itself_is_flagged_as_its_own() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let s = service(dir.path()).with_events(tx);
        s.handle_new_text("first", "text/plain");
        assert_eq!(rx.try_recv().unwrap(), ClipEvent::Saved { id: 1, own: false });
        s.expect_own_write(own_key(ClipKind::Text, "first"), false);
        s.handle_new_text("first", "text/plain");
        assert_eq!(rx.try_recv().unwrap(), ClipEvent::Saved { id: 1, own: true });
        // only once
        s.handle_new_text("first", "text/plain");
        assert_eq!(rx.try_recv().unwrap(), ClipEvent::Saved { id: 1, own: false });
    }

    #[test]
    fn a_skipped_own_write_is_not_kept_at_all() {
        let dir = tempfile::tempdir().unwrap();
        let s = service(dir.path());
        s.expect_own_write(own_key(ClipKind::Text, "Nov 14, 2026"), true);
        assert!(s.handle_new_text("Nov 14, 2026", "text/plain").is_none());
        assert!(s.get_recent_history(5).is_empty());
        // and a second copy of the same text is a normal copy again
        assert!(s.handle_new_text("Nov 14, 2026", "text/plain").is_some());
    }

    #[test]
    fn the_history_limit_is_applied_on_every_copy() {
        let dir = tempfile::tempdir().unwrap();
        let db = ClipboardDatabase::open_in_memory().unwrap().with_images_dir(dir.path().to_path_buf());
        let s = ClipboardService::new(db).with_pause(Arc::new(Mutex::new(PauseState::default()))).with_config(Config { clipboard_max_items: 20, clipboard_ocr: false, ..Config::default() });
        for i in 0..25 {
            s.handle_new_text(&format!("item {i}"), "text/plain");
        }
        assert_eq!(s.get_recent_history(100).len(), 20);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_watcher_guard_kills_child_on_drop() {
        let child = std::process::Command::new("sleep").arg("60").spawn().unwrap();
        let guard = WatcherGuard::new(child);
        let pid = guard.id();
        let alive = |pid: u32| std::process::Command::new("kill").args(["-0", &pid.to_string()]).status().map(|s| s.success()).unwrap_or(false);
        assert!(alive(pid), "child should be running before drop");
        drop(guard);
        assert!(!alive(pid), "child must be gone after drop");
    }
}
