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
use menew_tray::TrayCommand;

/// What the primary instance should do for a given command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    Quit,
}

/// `args` excludes the program name. `is_remote` is true when this command line
/// was forwarded from another process to the already-running primary instance.
/// `test_mode` is true when the daemon runs with `MENEW_TEST=1`, which enables the hidden test commands.
fn decide(args: &[String], is_remote: bool, test_mode: bool) -> Action {
    if args.iter().any(|a| a == "quit") {
        Action::Quit
    } else if args.iter().any(|a| a == "toggle") {
        Action::TogglePalette
    } else if args.iter().any(|a| a == "clipboard") {
        Action::ShowClipboard
    } else if let Some(action) = test_mode.then(|| test_action(args)).flatten() {
        action
    } else if is_remote {
        Action::ShowPalette
    } else {
        Action::StayHidden
    }
}

/// Parse `__select-clipboard|__pin-clipboard|__delete-clipboard <index>`.
fn test_action(args: &[String]) -> Option<Action> {
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
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

fn handle_clipboard_push() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Read;
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    if input.trim().is_empty() {
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        // Check if clipboard contains secret markers
        if let Ok(output) = std::process::Command::new("wl-paste").arg("--list-types").output() {
            let mimes = String::from_utf8_lossy(&output.stdout);
            for line in mimes.lines() {
                let lower = line.to_lowercase();
                if lower.contains("password") || lower.contains("concealedtype") {
                    info!("KDE watcher: ignoring copy marked as secret/password hint");
                    return Ok(());
                }
            }
        }

        // Send to primary daemon over session D-Bus
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        rt.block_on(async {
            if let Ok(conn) = zbus::Connection::session().await {
                let _ = conn
                    .call_method(
                        Some("org.menew.ClipboardService"),
                        "/org/menew/Clipboard",
                        Some("org.menew.ClipboardService"),
                        "PushClipboardText",
                        &(input.as_str(), "text/plain"),
                    )
                    .await;
            }
        });
    }

    Ok(())
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

        let _ = menew_tray::start_tray(tray_tx).await;
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

    let watcher_cell: Rc<RefCell<Option<menew_clipboard::WatcherGuard>>> =
        Rc::new(RefCell::new(None));

    // connect_startup is guaranteed to execute ONLY on the primary instance
    let p_startup = Rc::clone(&palette_cell);
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
        let palette = menew_ui::PaletteWindow::create(application, db.clone());
        *p_startup.borrow_mut() = Some(Rc::clone(&palette));
        let db_tray = db.clone();
        let clipboard_service = Arc::new(ClipboardService::new(db));

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
        *w_startup.borrow_mut() = menew_clipboard::start_kde_data_control_listener();

        // 6. Start System Tray
        rt_startup.spawn(async move {
            if let Err(e) = menew_tray::start_tray(tray_tx).await {
                warn!("Could not start tray service (StatusNotifierItem): {}", e);
            }
        });

        // 7. Start Global Shortcuts portal
        rt_startup.spawn(async move {
            let status = menew_shortcuts::register_and_listen(shortcut_tx).await;
            info!("Shortcut portal status: {:?}", status);
        });

        // 8. Connect channel receivers into GLib main context
        let p_tray = Rc::clone(&palette);
        let app_handle = application.clone();
        glib::MainContext::default().spawn_local(async move {
            while let Some(cmd) = tray_rx.recv().await {
                match cmd {
                    TrayCommand::OpenPalette => p_tray.toggle(),
                    TrayCommand::OpenClipboard => p_tray.show_mode(PaletteMode::Clipboard),
                    TrayCommand::ClearHistory => match db_tray.clear_unpinned() {
                        Ok(n) => info!("Tray: cleared {} unpinned clipboard entries", n),
                        Err(e) => warn!("Tray: failed to clear clipboard history: {}", e),
                    },
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
        w_shutdown.borrow_mut().take();
    });

    // Run application passing real command line arguments
    app.run_with_args(&args);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
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
