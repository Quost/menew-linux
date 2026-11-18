#[cfg(target_os = "linux")]
mod alerts;
#[cfg(target_os = "linux")]
mod updater;
#[cfg(unix)]
mod updater_cli;
mod portal;
#[cfg(target_os = "linux")]
use std::cell::RefCell;
#[cfg(target_os = "linux")]
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::mpsc;
#[allow(unused_imports)]
use tracing::{error, info, warn};

#[cfg(target_os = "linux")]
use gtk4::gio;
#[cfg(target_os = "linux")]
use gtk4::glib;
#[cfg(target_os = "linux")]
use gtk4::prelude::*;
#[cfg(target_os = "linux")]
use libadwaita as adw;

use menew_clipboard::ClipboardService;
use menew_core::{set_language, ClipboardDatabase, Language};
#[cfg(target_os = "linux")]
use menew_core::PaletteMode;
use menew_shortcuts::ShortcutEvent;
use menew_tray::{TrayCommand, TrayStatus};

/// What the primary instance should do for a given command line.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Action {
    /// Start in the background (tray only), palette hidden.
    StayHidden,
    ShowPalette,
    TogglePalette,
    /// Open the palette in clipboard mode.
    ShowClipboard,
    /// Test hooks (only with `MENEW_TEST=1`): simulate Enter / Ctrl+P / Ctrl+Delete on the
    /// Nth (0-based) clipboard entry of the unfiltered list.
    SelectClipboard(usize),
    PinClipboard(usize),
    DeleteClipboard(usize),
    /// Test hooks (only with `MENEW_TEST=1`): open the palette, type this text, log the results.
    PaletteQuery(String),
    /// Test hook: press Enter on the Nth (0-based) result of the current search.
    PaletteActivate(usize),
    /// Paste the next item of the paste queue.
    PasteNext,
    /// Test hook: show the clipboard list with this filter and search text, log it.
    ClipboardView(String, String),
    /// Open the notes window.
    ShowNotes,
    /// Test hooks: create a note (`text`, `secure` or `passwords`, then the text), open one by id, or paste the
    /// password or username of a row of the Passwords filter.
    NotesCreate(String, String),
    NotesOpen(Option<String>),
    PasswordPaste(usize, bool),
    /// Test hooks (with `MENEW_TEST=1` and `MENEW_UPDATE_FEED`): "Check for Updates" with its window, and
    /// check then install at once (what Install and Relaunch does, without clicking).
    UpdatesCheck,
    UpdatesInstall,
    /// Capture an area or window (the system asks which), then edit it.
    Capture,
    /// Capture the whole screen, then edit it.
    CaptureFull,
    /// Capture an area and copy its text.
    CaptureText,
    /// Capture an area and copy the content of the QR code in it (a link is never opened).
    QrScan,
    /// Show the QR code of the copied text, or of the given text (test hook).
    QrShow(Option<String>),
    QrWifi,
    /// Test hooks: use this image as the capture (`edit`, `text` or `qr`, then the PNG), run the export of the
    /// editor on a PNG with marks (`in`, `out`, marks), save a PNG into a folder with the name of the Mac, write
    /// the image of the QR code of a text.
    CaptureFrom(String, String),
    Annotate(String, String, String),
    CaptureSave(String, String),
    CaptureCopy(String, String),
    QrImage(String, String),
    /// Open the system monitor window.
    ShowMonitor,
    /// Test hooks: quit a process (SIGTERM, or SIGKILL with `force`) through the same refusal rules as the
    /// monitor view, and log the top processes of a fresh two-sample reading.
    QuitProcess(u32, bool),
    MonitorLog,
    /// Test hook: hide the monitor window.
    HideMonitor,
    Quit,
}

/// `args` excludes the program name. `is_remote` is true when this command line
/// was forwarded from another process to the already-running primary instance.
/// `test_mode` is true when the daemon runs with `MENEW_TEST=1`, which enables the hidden test commands.
fn decide(args: &[String], is_remote: bool, test_mode: bool) -> Action {
    // the test hooks come first: their text argument may be a word like "quit"
    if let Some(action) = test_mode.then(|| test_action(args)).flatten() {
        return action;
    }
    if args.iter().any(|a| a == "quit") {
        Action::Quit
    } else if args.iter().any(|a| a == "toggle") {
        Action::TogglePalette
    } else if args.iter().any(|a| a == "paste-next") {
        Action::PasteNext
    } else if args.iter().any(|a| a == "clipboard") {
        Action::ShowClipboard
    } else if args.iter().any(|a| a == "notes") {
        Action::ShowNotes
    } else if args.iter().any(|a| a == "capture-full") {
        Action::CaptureFull
    } else if args.iter().any(|a| a == "capture-text") {
        Action::CaptureText
    } else if args.iter().any(|a| a == "capture") {
        Action::Capture
    } else if args.iter().any(|a| a == "qr-scan") {
        Action::QrScan
    } else if args.iter().any(|a| a == "qr-wifi") {
        Action::QrWifi
    } else if args.iter().any(|a| a == "qr-show") {
        Action::QrShow(None)
    } else if args.iter().any(|a| a == "monitor") {
        Action::ShowMonitor
    } else if is_remote {
        Action::ShowPalette
    } else {
        Action::StayHidden
    }
}

/// Parse `__select-clipboard|__pin-clipboard|__delete-clipboard <index>`, `__palette-query <text>`
/// and `__palette-activate <index>`.
fn test_action(args: &[String]) -> Option<Action> {
    if args.iter().any(|a| a == "__updates-install") {
        return Some(Action::UpdatesInstall);
    }
    if args.iter().any(|a| a == "__updates-check") {
        return Some(Action::UpdatesCheck);
    }
    if let Some(p) = args.iter().position(|a| a == "__palette-query") {
        return args.get(p + 1).map(|t| Action::PaletteQuery(t.clone()));
    }
    if let Some(p) = args.iter().position(|a| a == "__capture-from") {
        return Some(Action::CaptureFrom(args.get(p + 1)?.clone(), args.get(p + 2)?.clone()));
    }
    if let Some(p) = args.iter().position(|a| a == "__annotate") {
        return Some(Action::Annotate(args.get(p + 1)?.clone(), args.get(p + 2)?.clone(), args.get(p + 3).cloned().unwrap_or_default()));
    }
    if let Some(p) = args.iter().position(|a| a == "__capture-save") {
        return Some(Action::CaptureSave(args.get(p + 1)?.clone(), args.get(p + 2)?.clone()));
    }
    if let Some(p) = args.iter().position(|a| a == "__capture-copy") {
        return Some(Action::CaptureCopy(args.get(p + 1)?.clone(), args.get(p + 2).cloned().unwrap_or_default()));
    }
    if let Some(p) = args.iter().position(|a| a == "__qr-image") {
        return Some(Action::QrImage(args.get(p + 1)?.clone(), args.get(p + 2)?.clone()));
    }
    if let Some(p) = args.iter().position(|a| a == "__qr-show") {
        return Some(Action::QrShow(args.get(p + 1).cloned()));
    }
    if let Some(p) = args.iter().position(|a| a == "__quit-process") {
        let force = args.get(p + 2).map(|f| f == "force").unwrap_or(false);
        return Some(Action::QuitProcess(args.get(p + 1)?.parse().ok()?, force));
    }
    if args.iter().any(|a| a == "__monitor-log") {
        return Some(Action::MonitorLog);
    }
    if args.iter().any(|a| a == "__monitor-hide") {
        return Some(Action::HideMonitor);
    }
    if let Some(p) = args.iter().position(|a| a == "__notes-create") {
        return Some(Action::NotesCreate(args.get(p + 1)?.clone(), args.get(p + 2).cloned().unwrap_or_default()));
    }
    if let Some(p) = args.iter().position(|a| a == "__notes-open") {
        return Some(Action::NotesOpen(args.get(p + 1).cloned()));
    }
    if let Some(p) = args.iter().position(|a| a == "__password-paste") {
        return Some(Action::PasswordPaste(args.get(p + 1)?.parse().ok()?, args.get(p + 2).map(|s| s == "username").unwrap_or(false)));
    }
    if let Some(p) = args.iter().position(|a| a == "__clipboard-view") {
        return args.get(p + 1).map(|f| Action::ClipboardView(f.clone(), args.get(p + 2).cloned().unwrap_or_default()));
    }
    if let Some(p) = args.iter().position(|a| a == "__palette-activate") {
        return args.get(p + 1)?.parse().ok().map(Action::PaletteActivate);
    }
    let (pos, make): (usize, fn(usize) -> Action) =
        if let Some(p) = args.iter().position(|a| a == "__select-clipboard") {
            (p, Action::SelectClipboard)
        } else if let Some(p) = args.iter().position(|a| a == "__pin-clipboard") {
            (p, Action::PinClipboard)
        } else {
            let p = args.iter().position(|a| a == "__delete-clipboard")?;
            (p, Action::DeleteClipboard)
        };
    args.get(pos + 1)?.parse().ok().map(make)
}

/// Text printed by `menew --version`.
fn version_line() -> String {
    format!("menew {}", env!("CARGO_PKG_VERSION"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().skip(1).any(|a| a == "--version" || a == "-V") {
        println!("{}", version_line());
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "menew=info,menew_tray=info,menew_shortcuts=info,menew_clipboard=info".into()
            }),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();

    // Check for internal helper invoked by wl-paste --watch
    if args.len() > 1 && args[1] == "__clipboard-push" {
        return handle_clipboard_push();
    }

    // `menew check-updates [--install]`: a terminal command, it never starts or talks to the daemon
    #[cfg(unix)]
    if args.get(1).map(|a| a == "check-updates").unwrap_or(false) {
        set_language(Language::detect());
        std::process::exit(updater_cli::run(&args[2..]));
    }

    // `menew --snapshot <dir> [-language en|pt|de|es|zh]`: render the screens with fake data and exit.
    // It never reads the clipboard, apps, notes or config of the user, and never talks to a running daemon.
    #[cfg(target_os = "linux")]
    if let Some((dir, lang)) = snapshot_request(&args) {
        return run_snapshot(dir, lang);
    }

    info!("Starting Menew Linux");

    // Language detection
    let detected_lang = Language::detect();
    set_language(detected_lang);
    info!("Language set to: {:?}", detected_lang);

    #[cfg(target_os = "linux")]
    {
        run_gtk_app(args);
    }

    #[cfg(not(target_os = "linux"))]
    {
        info!("Running on non-Linux host in headless verification mode");
        run_stub_app();
    }

    Ok(())
}

/// Parse `--snapshot <dir>` and the optional `-language <code>` / `--language <code>` (default: English).
#[cfg(target_os = "linux")]
fn snapshot_request(args: &[String]) -> Option<(std::path::PathBuf, Language)> {
    let p = args.iter().position(|a| a == "--snapshot")?;
    let dir = args.get(p + 1).filter(|d| !d.starts_with('-'))?;
    let lang = args
        .iter()
        .position(|a| a == "-language" || a == "--language")
        .and_then(|l| args.get(l + 1))
        .map(|c| Language::from_code(c))
        .unwrap_or(Language::En);
    Some((std::path::PathBuf::from(dir), lang))
}

/// `menew --snapshot`: a separate, non-unique application, so a running daemon is never contacted.
#[cfg(target_os = "linux")]
fn run_snapshot(dir: std::path::PathBuf, lang: Language) -> Result<(), Box<dyn std::error::Error>> {
    set_language(lang);
    let app = adw::Application::builder()
        .application_id("app.menew.Menew.Snapshot")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    let failed = Rc::new(std::cell::Cell::new(false));
    let f = failed.clone();
    app.connect_activate(move |app| {
        menew_ui::snapshot::run(app, dir.clone(), lang, f.clone());
    });
    app.run_with_args::<&str>(&[]);
    if failed.get() {
        return Err("snapshot failed".into());
    }
    Ok(())
}

fn handle_clipboard_push() -> Result<(), Box<dyn std::error::Error>> {
    menew_clipboard::run_push_helper()
}

#[cfg(not(target_os = "linux"))]
fn run_stub_app() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    rt.block_on(async {
        let db = ClipboardDatabase::open_in_memory().unwrap();
        let _service = Arc::new(ClipboardService::new(db));
        let (tray_tx, mut tray_rx) = mpsc::unbounded_channel::<TrayCommand>();
        let (shortcut_tx, mut shortcut_rx) = mpsc::unbounded_channel::<ShortcutEvent>();

        let (_status_tx, status_rx) = mpsc::unbounded_channel::<TrayStatus>();
        let _ = menew_tray::start_tray(tray_tx, status_rx).await;
        let _ = menew_shortcuts::register_and_listen(shortcut_tx).await;

        tokio::select! {
            Some(cmd) = tray_rx.recv() => {
                info!("Stub received tray command: {:?}", cmd);
            }
            Some(ev) = shortcut_rx.recv() => {
                info!("Stub received shortcut event: {:?}", ev);
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(50)) => {
                info!("Spike startup sanity checks completed successfully.");
            }
        }
    });
}

#[cfg(target_os = "linux")]
fn run_gtk_app(args: Vec<String>) {
    // Run Tokio on background threads, keeping GTK as the master of the main thread
    let rt = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to initialize background Tokio runtime"),
    );

    let app = adw::Application::builder()
        .application_id("app.menew.Menew")
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    let palette_cell: Rc<RefCell<Option<Rc<menew_ui::PaletteWindow>>>> = Rc::new(RefCell::new(None));
    let updater_cell: Rc<RefCell<Option<Rc<updater::Updater>>>> = Rc::new(RefCell::new(None));

    let watcher_cell: Rc<RefCell<Option<Rc<RefCell<menew_clipboard::WatcherControl>>>>> =
        Rc::new(RefCell::new(None));

    // connect_startup is guaranteed to execute ONLY on the primary instance
    let p_startup = Rc::clone(&palette_cell);
    let u_startup = Rc::clone(&updater_cell);
    let rt_startup = Arc::clone(&rt);
    let w_startup = Rc::clone(&watcher_cell);
    app.connect_startup(move |application| {
        info!("GApplication startup: primary instance initializing background services");

        // 1. Open DB (shared by the clipboard service and the palette)
        let db = match ClipboardDatabase::open_default() {
            Ok(database) => database,
            Err(e) => {
                warn!("Failed to open clipboard database on disk, falling back to memory: {}", e);
                ClipboardDatabase::open_in_memory().expect("In-memory SQLite failed")
            }
        };

        // 2. Create Palette Window and init ClipboardService
        let config = menew_core::Config::load();
        let state: menew_ui::SharedState = Rc::new(RefCell::new(menew_ui::PaletteState::from_config(&config)));
        // the clipboard service: pause state shared with the watcher control, events to the palette
        let pause = Arc::new(std::sync::Mutex::new(menew_core::clip_policy::PauseState::load()));
        let (clip_tx, mut clip_rx) = mpsc::unbounded_channel::<menew_clipboard::ClipEvent>();
        let clipboard_service = Arc::new(ClipboardService::new(db.clone()).with_pause(Arc::clone(&pause)).with_events(clip_tx));
        let _ = db.drop_missing_images();
        clipboard_service.prune();
        let control = Rc::new(RefCell::new(menew_clipboard::WatcherControl::new(Arc::clone(&pause))));
        let (status_tx, status_rx) = mpsc::unbounded_channel::<TrayStatus>();
        let updater = updater::Updater::new(application, Arc::clone(&rt_startup), config.auto_update_check);
        // the chosen system metrics (title and tooltip lines of the tray), refreshed by the sampler below
        let tray_metrics: Rc<RefCell<(String, Vec<String>)>> = Rc::new(RefCell::new((String::new(), Vec::new())));
        // pushes the countdown to the tray whenever it changes
        let last_status = Rc::new(RefCell::new(TrayStatus::default()));
        let push_status: Rc<dyn Fn()> = {
            let state = Rc::clone(&state);
            let last = Rc::clone(&last_status);
            let tx = status_tx.clone();
            let svc = Arc::clone(&clipboard_service);
            let upd = Rc::clone(&updater);
            let metrics = Rc::clone(&tray_metrics);
            Rc::new(move || {
                let mut status = alerts::tray_status(&state.borrow(), alerts::now(), menew_core::current_language());
                status.paused = svc.is_paused();
                status.update = upd.pending_version();
                status.auto_update = upd.auto_enabled();
                status.metrics = metrics.borrow().0.clone();
                status.metrics_lines = metrics.borrow().1.clone();
                if *last.borrow() != status {
                    info!("Tray status: {:?} {:?}", status.title(), status.lines);
                    *last.borrow_mut() = status.clone();
                    let _ = tx.send(status);
                }
            })
        };
        {
            let push = Rc::clone(&push_status);
            updater.set_on_change(Rc::new(move || push()));
        }
        updater.start_schedule();
        *u_startup.borrow_mut() = Some(Rc::clone(&updater));
        let hooks = {
            let (s1, s2, s3, s4, s5) = (
                Arc::clone(&clipboard_service),
                Arc::clone(&clipboard_service),
                Arc::clone(&clipboard_service),
                Arc::clone(&clipboard_service),
                Arc::clone(&clipboard_service),
            );
            let (c1, c2) = (Rc::clone(&control), Rc::clone(&control));
            menew_ui::ClipHooks {
                pause: Rc::new(move |m| {
                    s1.pause(m);
                    c1.borrow_mut().sync();
                }),
                resume: Rc::new(move || {
                    s2.resume();
                    c2.borrow_mut().sync();
                }),
                is_paused: Rc::new(move || s3.is_paused()),
                expect_own: Rc::new(move |k, skip| s4.expect_own_write(k, skip)),
                queue_ocr: Rc::new(move || s5.queue_ocr()),
                check_updates: {
                    let u = Rc::clone(&updater);
                    Rc::new(move || u.check_manual())
                },
            }
        };
        // the notes: content of the protected ones in the Secret Service, the user asked through polkit
        let vault: Rc<dyn menew_core::notes::Vault> = Rc::new(menew_clipboard::secrets::SecretServiceVault);
        let notes_store = menew_core::notes::NotesStore::load_default(vault);
        let notes = menew_ui::notes_view::NotesApp::new(application, notes_store, Arc::new(menew_clipboard::secrets::PolkitAuthenticator));
        // the screen: the system takes the capture (screenshot portal), the answer comes back to the GTK thread
        let rt_portal = Arc::clone(&rt_startup);
        let portal: menew_ui::screen::Portal = Rc::new(move |interactive, done| {
            let (tx, rx) = tokio::sync::oneshot::channel();
            rt_portal.spawn(async move {
                let _ = tx.send(portal::screenshot(interactive).await);
            });
            glib::spawn_future_local(async move {
                match rx.await {
                    Ok(result) => done(result),
                    Err(_) => done(Err("the portal request ended without an answer".into())),
                }
            });
        });
        let screen = menew_ui::screen::ScreenApp::new(application, portal);
        let palette = menew_ui::PaletteWindow::create(application, db.clone(), Rc::clone(&state), Rc::clone(&push_status), hooks, notes, screen);
        *p_startup.borrow_mut() = Some(Rc::clone(&palette));
        let db_tray = db.clone();

        // copies reach the palette (paste queue, open list) on the GTK thread
        {
            let p = Rc::clone(&palette);
            glib::MainContext::default().spawn_local(async move {
                while let Some(ev) = clip_rx.recv().await {
                    match ev {
                        menew_clipboard::ClipEvent::Saved { id, own } => {
                            if !own {
                                p.on_clip_saved(id);
                            }
                        }
                    }
                }
            });
        }
        // text of the images that were not read yet, a little after the start (like the Mac)
        {
            let svc = Arc::clone(&clipboard_service);
            glib::timeout_add_seconds_local_once(10, move || svc.queue_ocr());
            let svc = Arc::clone(&clipboard_service);
            glib::timeout_add_seconds_local(3600, move || {
                svc.prune();
                glib::ControlFlow::Continue
            });
        }
        // the watcher follows the pause, also when a timed pause ends
        {
            let c = Rc::clone(&control);
            glib::timeout_add_seconds_local(2, move || {
                c.borrow_mut().sync();
                glib::ControlFlow::Continue
            });
        }

        // 3. Setup communication channels
        let (tray_tx, mut tray_rx) = mpsc::unbounded_channel::<TrayCommand>();
        let (shortcut_tx, mut shortcut_rx) = mpsc::unbounded_channel::<ShortcutEvent>();

        // 4. Start D-Bus listener and hold connection
        let cb_dbus = Arc::clone(&clipboard_service);
        rt_startup.spawn(async move {
            match menew_clipboard::start_gnome_dbus_listener(cb_dbus).await {
                Ok(conn) => {
                    info!("GNOME D-Bus listener active; holding connection");
                    std::future::pending::<()>().await;
                    drop(conn);
                }
                Err(e) => {
                    warn!("GNOME D-Bus clipboard listener failed to initialize: {}", e);
                }
            }
        });

        // 5. Start KDE watcher child
        control.borrow_mut().sync();
        *w_startup.borrow_mut() = Some(Rc::clone(&control));

        // 6. Start System Tray
        rt_startup.spawn(async move {
            if let Err(e) = menew_tray::start_tray(tray_tx, status_rx).await {
                warn!("Could not start tray service (StatusNotifierItem): {}", e);
            }
        });

        // 7. Start Global Shortcuts portal
        rt_startup.spawn(async move {
            let status = menew_shortcuts::register_and_listen(shortcut_tx).await;
            info!("Shortcut portal status: {:?}", status);
        });

        // 7b. Tick the timers, alarms and focus session once a second
        {
            let state = Rc::clone(&state);
            let app = application.clone();
            let push = Rc::clone(&push_status);
            glib::timeout_add_seconds_local(1, move || {
                let lang = menew_core::current_language();
                alerts::tick(&app, &mut state.borrow_mut(), alerts::now(), lang);
                push();
                glib::ControlFlow::Continue
            });
        }

        // 7c. System metrics in the tray: sampled every 2 s, and only when the user chose some
        // (`monitor_tray_items` of config.json), so nothing is read when nobody looks
        if !config.monitor_tray_items.is_empty() {
            let items = config.monitor_tray_items.clone();
            let mut sampler = menew_core::monitor::Sampler::new(false);
            let metrics = Rc::clone(&tray_metrics);
            let push = Rc::clone(&push_status);
            let tick = move || {
                let lang = menew_core::current_language();
                let values = sampler.tick().tray_values();
                *metrics.borrow_mut() = menew_core::monitor::format_tray_metrics(&items, &values, lang);
                push();
            };
            let mut tick = tick;
            tick();
            glib::timeout_add_seconds_local(menew_core::monitor::INTERVAL_SECS, move || {
                tick();
                glib::ControlFlow::Continue
            });
        }

        // 8. Connect channel receivers into GLib main context
        let p_tray = Rc::clone(&palette);
        let tray_svc = Arc::clone(&clipboard_service);
        let tray_ctl = Rc::clone(&control);
        let app_handle = application.clone();
        let upd_tray = Rc::clone(&updater);
        glib::MainContext::default().spawn_local(async move {
            while let Some(cmd) = tray_rx.recv().await {
                match cmd {
                    TrayCommand::OpenPalette => p_tray.toggle(),
                    TrayCommand::OpenClipboard => p_tray.show_mode(PaletteMode::Clipboard),
                    TrayCommand::OpenMonitor => p_tray.open_monitor(),
                    TrayCommand::ClearHistory => match db_tray.clear_unpinned() {
                        Ok(n) => info!("Tray: cleared {} unpinned clipboard entries", n),
                        Err(e) => warn!("Tray: failed to clear clipboard history: {}", e),
                    },
                    TrayCommand::TogglePause => {
                        if tray_svc.is_paused() {
                            tray_svc.resume();
                        } else {
                            tray_svc.pause(None);
                        }
                        tray_ctl.borrow_mut().sync();
                        p_tray.queue_changed();
                    }
                    TrayCommand::PasteNext => p_tray.paste_next(),
                    TrayCommand::CheckUpdates => upd_tray.check_manual(),
                    TrayCommand::ShowUpdate => upd_tray.show_pending(),
                    TrayCommand::ToggleAutoUpdate => upd_tray.toggle_auto(),
                    TrayCommand::Capture => p_tray.screen().capture(menew_ui::screen::Purpose::Edit, true),
                    TrayCommand::CaptureText => p_tray.screen().capture(menew_ui::screen::Purpose::Text, true),
                    TrayCommand::ScanQr => p_tray.screen().capture(menew_ui::screen::Purpose::Qr, true),
                    TrayCommand::Quit => app_handle.quit(),
                }
            }
        });

        let p_short = Rc::clone(&palette);
        glib::MainContext::default().spawn_local(async move {
            while let Some(ev) = shortcut_rx.recv().await {
                match ev {
                    ShortcutEvent::TogglePalette => p_short.toggle(),
                    ShortcutEvent::ToggleClipboard => p_short.toggle_mode(PaletteMode::Clipboard),
                    ShortcutEvent::PasteNext => p_short.paste_next(),
                    ShortcutEvent::Capture => p_short.screen().capture(menew_ui::screen::Purpose::Edit, true),
                    ShortcutEvent::CaptureText => p_short.screen().capture(menew_ui::screen::Purpose::Text, true),
                }
            }
        });
    });

    // Explicit activation (e.g. launcher/desktop file) shows the palette
    let p_act = Rc::clone(&palette_cell);
    app.connect_activate(move |_| {
        if let Some(palette) = p_act.borrow().as_ref() {
            palette.show();
        }
    });

    // connect_command_line handles first launch and remote IPC such as `menew toggle`
    let p_cmd = Rc::clone(&palette_cell);
    let app_cmd = app.clone();
    let upd_cmd = Rc::clone(&updater_cell);
    app.connect_command_line(move |_application, cmd_line| {
        let cmd_args: Vec<String> = cmd_line
            .arguments()
            .iter()
            .skip(1)
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        // The hidden test commands are enabled by the daemon's own environment. GApplication does
        // not forward the client's environment, so MENEW_TEST=1 must be set when the daemon starts.
        let test_mode = std::env::var("MENEW_TEST").map(|v| v == "1").unwrap_or(false);
        let action = decide(&cmd_args, cmd_line.is_remote(), test_mode);
        info!("CLI command line {:?} -> {:?}", cmd_args, action);

        // The primary instance always stays alive without a visible window (tray only),
        // whatever its first command was (e.g. `menew clipboard` as the first launch)
        if !cmd_line.is_remote() && action != Action::Quit {
            std::mem::forget(app_cmd.hold());
        }

        match action {
            Action::Quit => app_cmd.quit(),
            Action::StayHidden => {}
            Action::ShowClipboard => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    palette.show_mode(PaletteMode::Clipboard);
                }
            }
            Action::SelectClipboard(index)
            | Action::PinClipboard(index)
            | Action::DeleteClipboard(index) => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    let op = match action {
                        Action::PinClipboard(_) => menew_ui::TestOp::Pin,
                        Action::DeleteClipboard(_) => menew_ui::TestOp::Delete,
                        _ => menew_ui::TestOp::Select,
                    };
                    let found = palette.test_clipboard_op(op, index);
                    info!("Test hook: {:?} clipboard entry {} -> found={}", op, index, found);
                }
            }
            Action::PaletteQuery(text) => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    palette.test_query(&text);
                }
            }
            Action::PasteNext => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    palette.paste_next();
                }
            }
            Action::ClipboardView(filter, query) => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    palette.test_clipboard_view(&filter, &query);
                }
            }
            Action::ShowNotes => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    palette.notes().show();
                }
            }
            Action::NotesCreate(kind, text) => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    let text = text.replace("\\n", "\n");
                    let id = palette.notes().test_create(&kind, &text);
                    info!("Test hook: notes-create {} -> {:?}", kind, id);
                }
            }
            Action::NotesOpen(id) => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    let notes = palette.notes();
                    let id = id.or_else(|| notes.store.borrow().notes.first().map(|n| n.id.clone()));
                    match id {
                        Some(id) => notes.open(&id),
                        None => notes.show(),
                    }
                }
            }
            Action::ShowMonitor => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    palette.open_monitor();
                }
            }
            Action::QuitProcess(pid, force) => {
                match menew_core::monitor::sys::quit_process(pid, force) {
                    Ok(()) => info!("Test hook: quit-process {} force={} -> sent", pid, force),
                    Err(e) => info!("Test hook: quit-process {} force={} -> refused: {}", pid, force, e),
                }
            }
            Action::MonitorLog => log_monitor(),
            Action::HideMonitor => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    palette.close_monitor();
                }
            }
            Action::PasswordPaste(index, username) => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    let found = palette.test_password_paste(index, username);
                    info!("Test hook: password-paste {} username={} -> found={}", index, username, found);
                }
            }
            Action::UpdatesCheck => {
                if let Some(u) = upd_cmd.borrow().as_ref() {
                    u.check_manual();
                }
            }
            Action::UpdatesInstall => {
                if let Some(u) = upd_cmd.borrow().as_ref() {
                    u.test_check_and_install();
                }
            }
            Action::PaletteActivate(index) => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    let found = palette.test_activate(index);
                    info!("Test hook: activate palette result {} -> found={}", index, found);
                }
            }
            Action::Capture | Action::CaptureFull | Action::CaptureText | Action::QrScan => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    use menew_ui::screen::Purpose;
                    let (purpose, interactive) = match action {
                        Action::CaptureFull => (Purpose::Edit, false),
                        Action::CaptureText => (Purpose::Text, true),
                        Action::QrScan => (Purpose::Qr, true),
                        _ => (Purpose::Edit, true),
                    };
                    palette.screen().capture(purpose, interactive);
                }
            }
            Action::QrShow(text) => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    palette.screen().show_qr(text, false);
                    if test_mode {
                        // the payload of the window, read after the clipboard had time to fill it
                        let screen = Rc::clone(palette.screen());
                        glib::timeout_add_local_once(std::time::Duration::from_millis(600), move || {
                            info!("Test hook: QR window payload={:?}", screen.qr_payload());
                        });
                    }
                }
            }
            Action::QrWifi => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    palette.screen().show_qr(None, true);
                }
            }
            Action::CaptureFrom(purpose, file) => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    use menew_ui::screen::Purpose;
                    let purpose = match purpose.as_str() {
                        "text" => Purpose::Text,
                        "qr" => Purpose::Qr,
                        _ => Purpose::Edit,
                    };
                    // the image is the test's own file: it is kept
                    palette.screen().open_file(purpose, std::path::Path::new(&file), false);
                    info!("Test hook: capture-from {:?} {}", purpose, file);
                }
            }
            Action::Annotate(input, output, marks) => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    match palette.screen().test_annotate(std::path::Path::new(&input), std::path::Path::new(&output), &marks) {
                        Ok(()) => info!("Test hook: annotate ok"),
                        Err(e) => warn!("Test hook: annotate failed: {}", e),
                    }
                }
            }
            Action::CaptureSave(input, dir) => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    match palette.screen().test_save(std::path::Path::new(&input), std::path::Path::new(&dir)) {
                        Ok(path) => info!("Test hook: capture-save ok {}", path.display()),
                        Err(e) => warn!("Test hook: capture-save failed: {}", e),
                    }
                }
            }
            Action::CaptureCopy(input, marks) => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    match palette.screen().test_copy(std::path::Path::new(&input), &marks) {
                        Ok(()) => info!("Test hook: capture-copy ok"),
                        Err(e) => warn!("Test hook: capture-copy failed: {}", e),
                    }
                }
            }
            Action::QrImage(payload, output) => match menew_ui::screen::ScreenApp::test_qr_image(&payload, std::path::Path::new(&output)) {
                Ok(()) => info!("Test hook: qr-image ok"),
                Err(e) => warn!("Test hook: qr-image failed: {}", e),
            },
            Action::ShowPalette => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    palette.show();
                }
            }
            Action::TogglePalette => {
                if let Some(palette) = p_cmd.borrow().as_ref() {
                    palette.toggle();
                }
            }
        }
        0
    });

    // Stop the wl-paste watcher when the primary instance shuts down
    let w_shutdown = Rc::clone(&watcher_cell);
    app.connect_shutdown(move |_| {
        info!("GApplication shutdown: stopping clipboard watcher");
        if let Some(c) = w_shutdown.borrow_mut().take() {
            c.borrow_mut().stop();
        }
    });

    // Run application passing real command line arguments
    app.run_with_args(&args);
}

/// Test hook: two samples a moment apart, then one log line per process of each top list.
#[cfg(target_os = "linux")]
fn log_monitor() {
    use menew_core::monitor::Sampler;
    let mut sampler = Sampler::new(true);
    sampler.tick();
    glib::timeout_add_local_once(std::time::Duration::from_millis(1500), move || {
        let snap = sampler.tick().clone();
        for (list, procs) in [("cpu", &snap.top_cpu), ("mem", &snap.top_mem)] {
            for p in procs {
                info!("Monitor top {}: pid={} name={} cpu={:.1} mem_kb={}", list, p.pid, p.name, p.cpu, p.mem_kb);
            }
        }
        info!("Monitor top done: cpu={} mem={}", snap.top_cpu.len(), snap.top_mem.len());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn version_line_has_the_alpha_version() {
        assert_eq!(version_line(), "menew 0.1.0-alpha.2");
    }

    #[test]
    fn first_launch_without_args_stays_hidden() {
        assert_eq!(decide(&v(&[]), false, false), Action::StayHidden);
    }

    #[test]
    fn remote_without_args_shows_palette() {
        assert_eq!(decide(&v(&[]), true, false), Action::ShowPalette);
    }

    #[test]
    fn toggle_and_quit() {
        assert_eq!(decide(&v(&["toggle"]), true, false), Action::TogglePalette);
        assert_eq!(decide(&v(&["toggle"]), false, false), Action::TogglePalette);
        assert_eq!(decide(&v(&["quit"]), true, false), Action::Quit);
    }

    #[test]
    fn clipboard_opens_clipboard_mode() {
        assert_eq!(decide(&v(&["clipboard"]), true, false), Action::ShowClipboard);
        assert_eq!(decide(&v(&["clipboard"]), false, false), Action::ShowClipboard);
    }

    #[test]
    fn paste_next_and_the_clipboard_view_hook() {
        assert_eq!(decide(&v(&["paste-next"]), true, false), Action::PasteNext);
        assert_eq!(decide(&v(&["__clipboard-view", "image", "x"]), true, true), Action::ClipboardView("image".into(), "x".into()));
        assert_eq!(decide(&v(&["__clipboard-view", "image"]), true, false), Action::ShowPalette);
    }

    #[test]
    fn monitor_command_and_hooks() {
        assert_eq!(decide(&v(&["monitor"]), true, false), Action::ShowMonitor);
        assert_eq!(decide(&v(&["__quit-process", "4242"]), true, true), Action::QuitProcess(4242, false));
        assert_eq!(decide(&v(&["__quit-process", "4242", "force"]), true, true), Action::QuitProcess(4242, true));
        assert_eq!(decide(&v(&["__monitor-log"]), true, true), Action::MonitorLog);
        // the hooks are plain unknown arguments without MENEW_TEST=1
        assert_eq!(decide(&v(&["__quit-process", "4242"]), true, false), Action::ShowPalette);
        assert_eq!(decide(&v(&["__quit-process", "x"]), true, true), Action::ShowPalette);
    }

    #[test]
    fn quit_wins_over_other_words() {
        assert_eq!(decide(&v(&["clipboard", "quit"]), true, false), Action::Quit);
    }

    #[test]
    fn select_clipboard_only_in_test_mode() {
        let args = v(&["__select-clipboard", "1"]);
        assert_eq!(decide(&args, true, true), Action::SelectClipboard(1));
        // without MENEW_TEST=1 the hidden command is just an unknown argument
        assert_eq!(decide(&args, true, false), Action::ShowPalette);
        assert_eq!(decide(&args, false, false), Action::StayHidden);
    }

    #[test]
    fn select_clipboard_needs_a_valid_index() {
        assert_eq!(decide(&v(&["__select-clipboard"]), true, true), Action::ShowPalette);
        assert_eq!(decide(&v(&["__select-clipboard", "x"]), true, true), Action::ShowPalette);
        assert_eq!(
            test_action(&v(&["__select-clipboard", "0"])),
            Some(Action::SelectClipboard(0))
        );
    }

    #[test]
    fn notes_hooks_only_in_test_mode() {
        let a = v(&["__notes-create", "text", "Shopping list phase6\\n#phase6tag\\nmilk and bread"]);
        assert_eq!(decide(&a, true, true), Action::NotesCreate("text".into(), "Shopping list phase6\\n#phase6tag\\nmilk and bread".into()));
        assert_eq!(decide(&a, true, false), Action::ShowPalette);
        assert_eq!(decide(&v(&["__notes-open", "ID"]), true, true), Action::NotesOpen(Some("ID".into())));
        assert_eq!(decide(&v(&["__password-paste", "0", "username"]), true, true), Action::PasswordPaste(0, true));
    }

    #[test]
    fn palette_hooks_only_in_test_mode_and_their_text_is_not_a_command() {
        let q = v(&["__palette-query", "5 km to mi"]);
        assert_eq!(decide(&q, true, true), Action::PaletteQuery("5 km to mi".into()));
        assert_eq!(decide(&q, true, false), Action::ShowPalette);
        // a query that is the word of another command stays a query
        assert_eq!(decide(&v(&["__palette-query", "quit"]), true, true), Action::PaletteQuery("quit".into()));
        assert_eq!(decide(&v(&["__palette-activate", "0"]), true, true), Action::PaletteActivate(0));
        assert_eq!(decide(&v(&["__palette-activate", "x"]), true, true), Action::ShowPalette);
        assert_eq!(decide(&v(&["__palette-query"]), true, true), Action::ShowPalette);
    }

    #[test]
    fn the_screen_commands_and_their_hooks() {
        assert_eq!(decide(&v(&["capture"]), true, false), Action::Capture);
        assert_eq!(decide(&v(&["capture-full"]), true, false), Action::CaptureFull);
        assert_eq!(decide(&v(&["capture-text"]), true, false), Action::CaptureText);
        assert_eq!(decide(&v(&["qr-scan"]), true, false), Action::QrScan);
        assert_eq!(decide(&v(&["qr-show"]), true, false), Action::QrShow(None));
        assert_eq!(decide(&v(&["qr-wifi"]), true, false), Action::QrWifi);
        // the hooks only exist with MENEW_TEST=1
        let hook = v(&["__capture-from", "qr", "/tmp/a.png"]);
        assert_eq!(decide(&hook, true, true), Action::CaptureFrom("qr".into(), "/tmp/a.png".into()));
        assert_eq!(decide(&hook, true, false), Action::ShowPalette);
        assert_eq!(decide(&v(&["__annotate", "a.png", "b.png", "blur:1,1,5,5"]), true, true), Action::Annotate("a.png".into(), "b.png".into(), "blur:1,1,5,5".into()));
        assert_eq!(decide(&v(&["__capture-save", "a.png", "/tmp"]), true, true), Action::CaptureSave("a.png".into(), "/tmp".into()));
        assert_eq!(decide(&v(&["__capture-copy", "a.png", "blur:1,1,5,5"]), true, true), Action::CaptureCopy("a.png".into(), "blur:1,1,5,5".into()));
        assert_eq!(decide(&v(&["__qr-image", "text", "o.png"]), true, true), Action::QrImage("text".into(), "o.png".into()));
        assert_eq!(decide(&v(&["__qr-show"]), true, true), Action::QrShow(None));
        assert_eq!(decide(&v(&["__qr-show", "abc"]), true, true), Action::QrShow(Some("abc".into())));
        // a word that is the name of a command stays text for a hook
        assert_eq!(decide(&v(&["__qr-show", "capture"]), true, true), Action::QrShow(Some("capture".into())));
        assert_eq!(decide(&v(&["__qr-show", "capture"]), true, false), Action::Capture);
    }

    #[test]
    fn pin_and_delete_hooks() {
        assert_eq!(
            decide(&v(&["__pin-clipboard", "3"]), true, true),
            Action::PinClipboard(3)
        );
        assert_eq!(
            decide(&v(&["__delete-clipboard", "2"]), true, true),
            Action::DeleteClipboard(2)
        );
        assert_eq!(decide(&v(&["__pin-clipboard", "3"]), true, false), Action::ShowPalette);
        assert_eq!(decide(&v(&["__delete-clipboard", "2"]), false, false), Action::StayHidden);
    }
}
