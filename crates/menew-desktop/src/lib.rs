//! Desktop integration that needs the session bus:
//! - screenshots and the color picker through `org.freedesktop.portal.Screenshot` (the desktop shows
//!   its own area / window picker, so Menew never reads the screen by itself);
//! - window snapping. Wayland apps can't move other apps' windows, so the desktop does it: KWin runs
//!   a tiny script loaded over `org.kde.kwin.Scripting`, GNOME Shell runs it in the Menew extension
//!   (`org.menew.Shell`). On X11 sessions without either, `wmctrl` is tried.
//!
//! Everything is async and runs on the daemon's Tokio runtime.

use std::path::PathBuf;

/// Where to put the front window, as fractions of the screen's work area (like `Windows.swift`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Snap {
    /// x, y, width, height in 0..1
    Rect(f64, f64, f64, f64),
    Maximize,
    /// Keep the size, center it.
    Center,
    NextDisplay,
}

impl Snap {
    /// The Mac's positions (`Windows.swift`), by the ids the palette and shortcuts use.
    pub fn from_id(id: &str) -> Option<Snap> {
        let t = 1.0 / 3.0;
        Some(match id {
            "left" => Snap::Rect(0.0, 0.0, 0.5, 1.0),
            "right" => Snap::Rect(0.5, 0.0, 0.5, 1.0),
            "top" => Snap::Rect(0.0, 0.0, 1.0, 0.5),
            "bottom" => Snap::Rect(0.0, 0.5, 1.0, 0.5),
            "topleft" => Snap::Rect(0.0, 0.0, 0.5, 0.5),
            "topright" => Snap::Rect(0.5, 0.0, 0.5, 0.5),
            "bottomleft" => Snap::Rect(0.0, 0.5, 0.5, 0.5),
            "bottomright" => Snap::Rect(0.5, 0.5, 0.5, 0.5),
            "leftthird" => Snap::Rect(0.0, 0.0, t, 1.0),
            "centerthird" => Snap::Rect(t, 0.0, t, 1.0),
            "rightthird" => Snap::Rect(2.0 * t, 0.0, t, 1.0),
            "lefttwothirds" => Snap::Rect(0.0, 0.0, 2.0 * t, 1.0),
            "righttwothirds" => Snap::Rect(t, 0.0, 2.0 * t, 1.0),
            "almost" => Snap::Rect(0.05, 0.05, 0.9, 0.9),
            "max" => Snap::Maximize,
            "center" => Snap::Center,
            "display" => Snap::NextDisplay,
            _ => return None,
        })
    }

    /// Every id, in the order the palette lists them.
    pub const IDS: [&'static str; 17] = [
        "left", "right", "top", "bottom", "max", "almost", "center", "topleft", "topright", "bottomleft", "bottomright", "leftthird",
        "centerthird", "rightthird", "lefttwothirds", "righttwothirds", "display",
    ];
}

/// The KWin script for a snap (works on KWin 5.27 and 6: `activeWindow` / `activeClient`,
/// `frameGeometry`, `sendClientToScreen`).
pub fn kwin_script(snap: Snap) -> String {
    let body = match snap {
        Snap::Rect(x, y, w, h) => format!(
            "if (w.setMaximize) w.setMaximize(false, false);\n\
             var a = workspace.clientArea(KWin.MaximizeArea, w);\n\
             w.frameGeometry = {{x: Math.round(a.x + a.width * {x}), y: Math.round(a.y + a.height * {y}), width: Math.round(a.width * {w}), height: Math.round(a.height * {h})}};"
        ),
        Snap::Maximize => "if (w.setMaximize) w.setMaximize(true, true);".to_string(),
        Snap::Center => "var a = workspace.clientArea(KWin.MaximizeArea, w);\n\
             var g = w.frameGeometry;\n\
             w.frameGeometry = {x: Math.round(a.x + (a.width - g.width) / 2), y: Math.round(a.y + (a.height - g.height) / 2), width: g.width, height: g.height};"
            .to_string(),
        Snap::NextDisplay => "if (workspace.screens) {\n\
               var i = workspace.screens.indexOf(w.output);\n\
               workspace.sendClientToScreen(w, workspace.screens[(i + 1) % workspace.screens.length]);\n\
             } else {\n\
               workspace.sendClientToScreen(w, (w.screen + 1) % workspace.numScreens);\n\
             }"
            .to_string(),
    };
    format!("var w = workspace.activeWindow || workspace.activeClient;\nif (w && w.moveable !== false && !w.specialWindow) {{\n{body}\n}}\n")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapError {
    /// Neither KWin nor the GNOME extension answered.
    Unsupported,
    Failed(String),
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use ashpd::desktop::{screenshot::Screenshot, Color};
    use tracing::{info, warn};

    /// Ask the desktop for a screenshot. `interactive` shows its picker (area, window, screen);
    /// otherwise the whole screen is taken. Returns the file the portal wrote.
    pub async fn screenshot(interactive: bool) -> Result<PathBuf, String> {
        let req = Screenshot::request().interactive(interactive).modal(true).send().await.map_err(|e| e.to_string())?;
        let shot = req.response().map_err(|e| e.to_string())?;
        shot.uri().to_file_path().map_err(|_| format!("not a local file: {}", shot.uri()))
    }

    /// The color under the pointer, as `#RRGGBB`.
    pub async fn pick_color() -> Result<String, String> {
        let req = Color::pick().send().await.map_err(|e| e.to_string())?;
        let c = req.response().map_err(|e| e.to_string())?;
        Ok(super::hex(c.red(), c.green(), c.blue()))
    }

    async fn kwin(conn: &zbus::Connection, snap: Snap) -> Result<(), String> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let path = dir.join(format!("menew-snap-{}.js", std::process::id()));
        std::fs::write(&path, kwin_script(snap)).map_err(|e| e.to_string())?;
        let name = "menew-snap";
        let scripting = zbus::Proxy::new(conn, "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting").await.map_err(|e| e.to_string())?;
        // a previous run that failed halfway would block the name
        let _: Result<bool, _> = scripting.call("unloadScript", &(name,)).await;
        let id: i32 = scripting.call("loadScript", &(path.to_string_lossy().as_ref(), name)).await.map_err(|e| e.to_string())?;
        // Plasma 6 puts the script at /Scripting/ScriptN, Plasma 5 at /N
        let mut ran = false;
        for p in [format!("/Scripting/Script{id}"), format!("/{id}")] {
            if let Ok(s) = zbus::Proxy::new(conn, "org.kde.KWin", p.as_str(), "org.kde.kwin.Script").await {
                if s.call::<_, _, ()>("run", &()).await.is_ok() {
                    ran = true;
                    break;
                }
            }
        }
        let _: Result<bool, _> = scripting.call("unloadScript", &(name,)).await;
        let _ = std::fs::remove_file(&path);
        if ran {
            Ok(())
        } else {
            Err("KWin did not run the script".into())
        }
    }

    async fn gnome(conn: &zbus::Connection, snap: Snap) -> Result<bool, String> {
        let p = zbus::Proxy::new(conn, "org.menew.Shell", "/org/menew/Shell", "org.menew.Shell").await.map_err(|e| e.to_string())?;
        let r: Result<bool, zbus::Error> = match snap {
            Snap::Rect(x, y, w, h) => p.call("SnapFocused", &(x, y, w, h)).await,
            Snap::Maximize => p.call("MaximizeFocused", &()).await,
            Snap::Center => p.call("CenterFocused", &()).await,
            Snap::NextDisplay => p.call("MoveFocusedToNextMonitor", &()).await,
        };
        r.map_err(|e| e.to_string())
    }

    async fn has_name(conn: &zbus::Connection, name: &str) -> bool {
        let Ok(dbus) = zbus::fdo::DBusProxy::new(conn).await else { return false };
        match zbus::names::BusName::try_from(name) {
            Ok(n) => dbus.name_has_owner(n).await.unwrap_or(false),
            Err(_) => false,
        }
    }

    /// Move the front window. The caller hides the palette first, so the front window is the one
    /// people were using.
    pub async fn snap(snap: Snap) -> Result<(), SnapError> {
        let conn = zbus::Connection::session().await.map_err(|e| SnapError::Failed(e.to_string()))?;
        if has_name(&conn, "org.kde.KWin").await {
            info!("Snap {:?} through KWin scripting", snap);
            return kwin(&conn, snap).await.map_err(SnapError::Failed);
        }
        if has_name(&conn, "org.menew.Shell").await {
            info!("Snap {:?} through the GNOME extension", snap);
            return match gnome(&conn, snap).await {
                Ok(true) => Ok(()),
                Ok(false) => Err(SnapError::Failed("no front window".into())),
                Err(e) => Err(SnapError::Failed(e)),
            };
        }
        warn!("Snap: neither KWin nor the Menew GNOME extension is on the bus");
        Err(SnapError::Unsupported)
    }

    /// Whether the Menew GNOME extension (version with the window methods) is running.
    pub async fn gnome_extension_running() -> bool {
        match zbus::Connection::session().await {
            Ok(c) => has_name(&c, "org.menew.Shell").await,
            Err(_) => false,
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::{gnome_extension_running, pick_color, screenshot, snap};

#[cfg(not(target_os = "linux"))]
pub async fn screenshot(_interactive: bool) -> Result<PathBuf, String> {
    Err("screenshots need Linux".into())
}

#[cfg(not(target_os = "linux"))]
pub async fn pick_color() -> Result<String, String> {
    Err("the color picker needs Linux".into())
}

#[cfg(not(target_os = "linux"))]
pub async fn snap(_snap: Snap) -> Result<(), SnapError> {
    Err(SnapError::Unsupported)
}

#[cfg(not(target_os = "linux"))]
pub async fn gnome_extension_running() -> bool {
    false
}

/// `#RRGGBB` from 0..1 components.
pub fn hex(r: f64, g: f64, b: f64) -> String {
    let c = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", c(r), c(g), c(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_id_is_a_snap() {
        for id in Snap::IDS {
            assert!(Snap::from_id(id).is_some(), "{id}");
        }
        assert_eq!(Snap::from_id("nope"), None);
        assert_eq!(Snap::from_id("right"), Some(Snap::Rect(0.5, 0.0, 0.5, 1.0)));
    }

    #[test]
    fn kwin_script_uses_the_work_area() {
        let s = kwin_script(Snap::Rect(0.5, 0.0, 0.5, 1.0));
        assert!(s.contains("workspace.activeWindow || workspace.activeClient"));
        assert!(s.contains("a.width * 0.5"));
        assert!(s.contains("KWin.MaximizeArea"));
        assert!(kwin_script(Snap::Maximize).contains("setMaximize(true, true)"));
        assert!(kwin_script(Snap::NextDisplay).contains("sendClientToScreen"));
    }

    #[test]
    fn hex_colors() {
        assert_eq!(hex(1.0, 0.5, 0.0), "#FF8000");
        assert_eq!(hex(2.0, -1.0, 0.0), "#FF0000");
    }
}
