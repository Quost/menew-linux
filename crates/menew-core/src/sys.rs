//! Small helpers to run the desktop's own command line tools (`pactl`, `gsettings`, `loginctl`,
//! `udisksctl`...). Menew prefers these over extra crates: they are already on GNOME and KDE systems
//! and every call is visible in `ps`. Nothing here touches the network.

use std::process::{Command, Stdio};

/// Run a program and return its standard output when it exits with success.
pub fn output(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Run a program and report whether it exited with success.
pub fn run(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Whether a program is on `PATH`.
pub fn has(program: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&path).any(|d| d.join(program).is_file())
}

/// The desktop the session runs, from `XDG_CURRENT_DESKTOP` (`ubuntu:GNOME`, `KDE`...).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desktop {
    Gnome,
    Kde,
    Other,
}

impl Desktop {
    pub fn from_value(v: &str) -> Self {
        let v = v.to_ascii_lowercase();
        if v.split(':').any(|p| p == "kde") {
            Desktop::Kde
        } else if v.split(':').any(|p| p == "gnome" || p == "gnome-classic" || p == "unity") {
            Desktop::Gnome
        } else {
            Desktop::Other
        }
    }

    pub fn detect() -> Self {
        Self::from_value(&std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default())
    }

    pub fn name(self) -> &'static str {
        match self {
            Desktop::Gnome => "GNOME",
            Desktop::Kde => "KDE Plasma",
            Desktop::Other => "other",
        }
    }
}

/// Send a signal to a process (`SIGTERM` = 15, `SIGKILL` = 9). Only the user's own processes can
/// be signalled; anything else fails.
pub fn signal(pid: u32, sig: i32) -> bool {
    #[cfg(target_os = "linux")]
    {
        if pid <= 1 {
            return false;
        }
        // SAFETY: kill(2) has no memory effects; a wrong pid only returns an error
        unsafe { libc::kill(pid as libc::pid_t, sig) == 0 }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (pid, sig);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_detection() {
        assert_eq!(Desktop::from_value("ubuntu:GNOME"), Desktop::Gnome);
        assert_eq!(Desktop::from_value("GNOME"), Desktop::Gnome);
        assert_eq!(Desktop::from_value("KDE"), Desktop::Kde);
        assert_eq!(Desktop::from_value("XFCE"), Desktop::Other);
        assert_eq!(Desktop::from_value(""), Desktop::Other);
    }

    #[test]
    fn never_signals_init_or_pid_0() {
        assert!(!signal(0, 15));
        assert!(!signal(1, 15));
    }
}
