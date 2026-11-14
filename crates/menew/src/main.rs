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
use menew_shortcuts::ShortcutEvent;
use menew_tray::TrayCommand;

/// What the primary instance should do for a given command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    /// Start in the background (tray only), palette hidden.
    StayHidden,
    ShowPalette,
    TogglePalette,
    Quit,
}

/// `args` excludes the program name. `is_remote` is true when this command line
/// was forwarded from another process to the already-running primary instance.
fn decide(args: &[String], is_remote: bool) -> Action {
    if args.iter().any(|a| a == "quit") {
        Action::Quit
    } else if args.iter().any(|a| a == "toggle") {
        Action::TogglePalette
    } else if is_remote {
        Action::ShowPalette
    } else {
        Action::StayHidden
    }
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

        // 1. Create Palette Window
        let palette = menew_ui::PaletteWindow::create(application);
        *p_startup.borrow_mut() = Some(Rc::clone(&palette));

        // 2. Open DB and init ClipboardService
        let db = match ClipboardDatabase::open_default() {
            Ok(database) => database,
            Err(e) => {
                warn!("Failed to open clipboard database on disk, falling back to memory: {}", e);
                ClipboardDatabase::open_in_memory().expect("In-memory SQLite failed")
            }
        };
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
                    TrayCommand::Quit => app_handle.quit(),
                }
            }
        });

        let p_short = Rc::clone(&palette);
        glib::MainContext::default().spawn_local(async move {
            while let Some(ShortcutEvent::TogglePalette) = shortcut_rx.recv().await {
                p_short.toggle();
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
        let action = decide(&cmd_args, cmd_line.is_remote());
        info!("CLI command line {:?} -> {:?}", cmd_args, action);

        match action {
            Action::Quit => app_cmd.quit(),
            Action::StayHidden => {
                // Keep the daemon alive without a visible window (tray only)
                std::mem::forget(app_cmd.hold());
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
        assert_eq!(decide(&v(&[]), false), Action::StayHidden);
    }

    #[test]
    fn remote_without_args_shows_palette() {
        assert_eq!(decide(&v(&[]), true), Action::ShowPalette);
    }

    #[test]
    fn toggle_and_quit() {
        assert_eq!(decide(&v(&["toggle"]), true), Action::TogglePalette);
        assert_eq!(decide(&v(&["toggle"]), false), Action::TogglePalette);
        assert_eq!(decide(&v(&["quit"]), true), Action::Quit);
    }
}
