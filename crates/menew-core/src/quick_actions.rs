//! Quick actions (port of `Toolbox.swift`, `AppControl.swift`, `Ports.swift` and the display part of
//! `Hardware.swift`): mute the microphone or the sound, switch the audio output and input, dark mode,
//! keep awake, lock the screen, empty the trash, eject disks, brightness, quit apps and stop servers.
//!
//! The parsing is pure and tested; the calls go to the tools every GNOME and KDE system has:
//! `pactl` (PipeWire or PulseAudio), `gsettings`, `plasma-apply-colorscheme`, `systemd-inhibit`,
//! `loginctl`, `gio`, `lsblk` + `udisksctl`, `busctl` (logind `SetBrightness`) and `ddcutil`.
//!
//! The palette shows these as [`ItemAction::Command`] items with an id like `qa.mic`; the daemon
//! runs them with [`run`].

use crate::desktop_entry::DesktopApp;
use crate::i18n::Language;
use crate::palette::{Icon, ItemAction, Kind, PaletteItem};
use crate::search::fold;
use crate::strings::tr;
use crate::sys::{self, Desktop};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

// ---------------------------------------------------------------------------------------------
// Audio
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDevice {
    /// The PulseAudio name (`alsa_output.pci-0000_00_1f.3.analog-stereo`).
    pub name: String,
    /// What people read (`Built-in Audio Analog Stereo`).
    pub description: String,
}

/// Parse `pactl -f json list sinks` or `... sources`. Monitors of outputs are left out.
pub fn parse_devices(json: &str) -> Vec<AudioDevice> {
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str::<serde_json::Value>(json) else {
        return vec![];
    };
    items
        .iter()
        .filter_map(|d| {
            let name = d.get("name")?.as_str()?.to_string();
            if name.ends_with(".monitor") {
                return None;
            }
            let description = d.get("description").and_then(|v| v.as_str()).unwrap_or(&name).to_string();
            Some(AudioDevice { name, description })
        })
        .collect()
}

/// `Mute: yes` from `pactl get-sink-mute`, or `[MUTED]` from `wpctl get-volume`.
pub fn parse_muted(out: &str) -> Option<bool> {
    let t = out.trim();
    if let Some(v) = t.strip_prefix("Mute:") {
        return Some(v.trim() == "yes");
    }
    if t.starts_with("Volume:") {
        return Some(t.contains("[MUTED]"));
    }
    None
}

fn pa_target(input: bool) -> (&'static str, &'static str) {
    if input {
        ("source", "@DEFAULT_SOURCE@")
    } else {
        ("sink", "@DEFAULT_SINK@")
    }
}

pub fn is_muted(input: bool) -> Option<bool> {
    let (kind, target) = pa_target(input);
    if let Some(out) = sys::output("pactl", &[&format!("get-{kind}-mute"), target]) {
        return parse_muted(&out);
    }
    let wp = if input { "@DEFAULT_AUDIO_SOURCE@" } else { "@DEFAULT_AUDIO_SINK@" };
    sys::output("wpctl", &["get-volume", wp]).and_then(|o| parse_muted(&o))
}

/// Toggle the mute of the default output (or input) and return the new state.
pub fn toggle_mute(input: bool) -> Option<bool> {
    let (kind, target) = pa_target(input);
    let ok = sys::run("pactl", &[&format!("set-{kind}-mute"), target, "toggle"])
        || sys::run("wpctl", &["set-mute", if input { "@DEFAULT_AUDIO_SOURCE@" } else { "@DEFAULT_AUDIO_SINK@" }, "toggle"]);
    if ok {
        is_muted(input)
    } else {
        None
    }
}

pub fn devices(input: bool) -> Vec<AudioDevice> {
    let list = if input { "sources" } else { "sinks" };
    sys::output("pactl", &["-f", "json", "list", list]).map(|j| parse_devices(&j)).unwrap_or_default()
}

pub fn default_device(input: bool) -> Option<String> {
    let cmd = if input { "get-default-source" } else { "get-default-sink" };
    sys::output("pactl", &[cmd]).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

pub fn set_default_device(input: bool, name: &str) -> bool {
    let cmd = if input { "set-default-source" } else { "set-default-sink" };
    sys::run("pactl", &[cmd, name])
}

// ---------------------------------------------------------------------------------------------
// Dark mode
// ---------------------------------------------------------------------------------------------

/// `'prefer-dark'` from `gsettings get org.gnome.desktop.interface color-scheme`.
pub fn parse_gnome_scheme(out: &str) -> bool {
    out.trim().trim_matches('\'') == "prefer-dark"
}

/// The `ColorScheme` of `[General]` in `kdeglobals`.
pub fn parse_kde_scheme(kdeglobals: &str) -> Option<String> {
    let mut in_general = false;
    for line in kdeglobals.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_general = l == "[General]";
        } else if in_general {
            if let Some(v) = l.strip_prefix("ColorScheme=") {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

fn kdeglobals_path() -> PathBuf {
    crate::config::config_dir().join("kdeglobals")
}

pub fn is_dark(desktop: Desktop) -> Option<bool> {
    if desktop == Desktop::Kde {
        let text = std::fs::read_to_string(kdeglobals_path()).ok()?;
        return Some(parse_kde_scheme(&text).map(|s| s.to_lowercase().contains("dark")).unwrap_or(false));
    }
    sys::output("gsettings", &["get", "org.gnome.desktop.interface", "color-scheme"]).map(|o| parse_gnome_scheme(&o))
}

pub fn set_dark(desktop: Desktop, dark: bool) -> bool {
    if desktop == Desktop::Kde {
        return sys::run("plasma-apply-colorscheme", &[if dark { "BreezeDark" } else { "BreezeLight" }]);
    }
    // GNOME 42+; `default` is the light look (GNOME 47 also knows `prefer-light`)
    let ok = sys::run("gsettings", &["set", "org.gnome.desktop.interface", "color-scheme", if dark { "prefer-dark" } else { "default" }]);
    // GTK 3 apps follow the theme name, not the color scheme
    let _ = sys::run("gsettings", &["set", "org.gnome.desktop.interface", "gtk-theme", if dark { "Adwaita-dark" } else { "Adwaita" }]);
    ok
}

// ---------------------------------------------------------------------------------------------
// Keep awake
// ---------------------------------------------------------------------------------------------

/// Why keep awake could not start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AwakeError {
    /// `systemd-inhibit` is not installed.
    Missing,
    /// It started and exited at once: logind refused the lock.
    Refused,
}

/// Keeps the screen on with `systemd-inhibit` (logind), like the Mac's `IOPMAssertion`.
#[derive(Default)]
pub struct KeepAwake {
    child: Option<Child>,
    /// When it turns itself off (seconds since the epoch), `None` = until turned off.
    pub until: Option<f64>,
}

impl KeepAwake {
    pub fn is_on(&mut self) -> bool {
        match self.child.as_mut().map(|c| c.try_wait()) {
            Some(Ok(None)) => true,
            Some(_) => {
                self.child = None;
                self.until = None;
                false
            }
            None => false,
        }
    }

    /// Turn it on for that many minutes (or until turned off).
    pub fn start(&mut self, minutes: Option<u32>, now: f64, why: &str) -> Result<(), AwakeError> {
        self.stop();
        let mut child = Command::new("systemd-inhibit")
            .args(["--what=idle:sleep", "--who=Menew", &format!("--why={why}"), "--mode=block", "sleep", "infinity"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| AwakeError::Missing)?;
        // systemd-inhibit exits at once when logind refuses the lock (no session, a container...):
        // give it a moment so Menew doesn't say it is on when nothing holds the lock
        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_millis(25));
            if !matches!(child.try_wait(), Ok(None)) {
                let _ = child.wait();
                return Err(AwakeError::Refused);
            }
        }
        self.child = Some(child);
        self.until = minutes.map(|m| now + m as f64 * 60.0);
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        self.until = None;
    }

    /// Called once a second: true when the time ran out and it turned off.
    pub fn tick(&mut self, now: f64) -> bool {
        if let Some(u) = self.until {
            if now >= u && self.is_on() {
                self.stop();
                return true;
            }
        }
        false
    }
}

impl Drop for KeepAwake {
    fn drop(&mut self) {
        self.stop();
    }
}

/// `awake 2h`, `acordado 90m`, `wach 30`: minutes, at most 24 h.
pub fn parse_duration_minutes(s: &str) -> Option<u32> {
    let s = s.trim().to_lowercase();
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    let n: u32 = digits.parse().ok()?;
    let unit = s[digits.len()..].trim();
    let minutes = match unit {
        "" | "m" | "min" | "mins" | "minutes" | "minutos" | "minuten" | "分钟" => n,
        "h" | "hr" | "hrs" | "hour" | "hours" | "hora" | "horas" | "std" | "stunde" | "stunden" | "小时" => n.checked_mul(60)?,
        _ => return None,
    };
    (1..=24 * 60).contains(&minutes).then_some(minutes)
}

// ---------------------------------------------------------------------------------------------
// Lock, trash
// ---------------------------------------------------------------------------------------------

pub fn lock_screen() -> bool {
    sys::run("loginctl", &["lock-session"])
        || sys::run("dbus-send", &["--session", "--dest=org.freedesktop.ScreenSaver", "--type=method_call", "/org/freedesktop/ScreenSaver", "org.freedesktop.ScreenSaver.Lock"])
        || sys::run("xdg-screensaver", &["lock"])
}

/// How many items the trash has (`gio trash --list`, GLib 2.70+).
pub fn trash_count() -> Option<usize> {
    sys::output("gio", &["trash", "--list"]).map(|o| o.lines().filter(|l| !l.trim().is_empty()).count())
}

pub fn empty_trash() -> bool {
    sys::run("gio", &["trash", "--empty"])
}

// ---------------------------------------------------------------------------------------------
// Eject
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volume {
    /// `/dev/sdb1`
    pub device: String,
    /// `/dev/sdb`, powered off after unmounting.
    pub disk: Option<String>,
    pub mountpoint: String,
}

/// Parse `lsblk -J -o PATH,PKNAME,RM,HOTPLUG,MOUNTPOINT,TYPE`: mounted partitions of removable or
/// hot-plugged disks (USB sticks, SD cards, external drives). The system disk is never listed.
pub fn parse_lsblk(json: &str) -> Vec<Volume> {
    fn walk(v: &serde_json::Value, parent_removable: bool, out: &mut Vec<Volume>) {
        let flag = |k: &str| match v.get(k) {
            Some(serde_json::Value::Bool(b)) => *b,
            Some(serde_json::Value::String(s)) => s == "1" || s == "true",
            Some(serde_json::Value::Number(n)) => n.as_i64() == Some(1),
            _ => false,
        };
        let removable = parent_removable || flag("rm") || flag("hotplug");
        if let (Some(path), Some(mp)) = (v.get("path").and_then(|p| p.as_str()), v.get("mountpoint").and_then(|m| m.as_str())) {
            let system = mp == "/" || mp.starts_with("/boot") || mp == "[SWAP]" || mp.starts_with("/usr") || mp.starts_with("/var") || mp == "/home";
            if removable && !system {
                let disk = v.get("pkname").and_then(|p| p.as_str()).map(|p| format!("/dev/{p}"));
                out.push(Volume { device: path.to_string(), disk, mountpoint: mp.to_string() });
            }
        }
        if let Some(serde_json::Value::Array(children)) = v.get("children") {
            for c in children {
                walk(c, removable, out);
            }
        }
    }
    let Ok(root) = serde_json::from_str::<serde_json::Value>(json) else { return vec![] };
    let mut out = vec![];
    if let Some(serde_json::Value::Array(devs)) = root.get("blockdevices") {
        for d in devs {
            walk(d, false, &mut out);
        }
    }
    out
}

pub fn ejectable() -> Vec<Volume> {
    sys::output("lsblk", &["-J", "-o", "PATH,PKNAME,RM,HOTPLUG,MOUNTPOINT,TYPE"]).map(|j| parse_lsblk(&j)).unwrap_or_default()
}

/// Unmount every ejectable volume and power off its disk. Returns how many were unmounted.
pub fn eject_all() -> usize {
    let vols = ejectable();
    let mut done = 0;
    let mut disks: Vec<String> = vec![];
    for v in &vols {
        if sys::run("udisksctl", &["unmount", "-b", &v.device, "--no-user-interaction"]) || sys::run("gio", &["mount", "-u", &v.mountpoint]) {
            done += 1;
            if let Some(d) = &v.disk {
                if !disks.contains(d) {
                    disks.push(d.clone());
                }
            }
        }
    }
    for d in disks {
        let _ = sys::run("udisksctl", &["power-off", "-b", &d, "--no-user-interaction"]);
    }
    done
}

// ---------------------------------------------------------------------------------------------
// Brightness
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backlight {
    pub name: String,
    pub value: u32,
    pub max: u32,
}

impl Backlight {
    pub fn percent(&self) -> u32 {
        if self.max == 0 {
            0
        } else {
            ((self.value as f64 / self.max as f64) * 100.0).round() as u32
        }
    }
}

pub fn read_backlight_in(dir: &Path) -> Option<Backlight> {
    let mut entries: Vec<_> = std::fs::read_dir(dir).ok()?.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    // the firmware interfaces last, like the kernel's own preference (raw > platform > firmware)
    let rank = |p: &Path| match std::fs::read_to_string(p.join("type")).unwrap_or_default().trim() {
        "raw" => 0,
        "platform" => 1,
        _ => 2,
    };
    entries.sort_by_key(|e| rank(&e.path()));
    let e = entries.first()?;
    let read = |f: &str| std::fs::read_to_string(e.path().join(f)).ok()?.trim().parse::<u32>().ok();
    Some(Backlight { name: e.file_name().to_string_lossy().into_owned(), value: read("brightness")?, max: read("max_brightness")? })
}

pub fn backlight() -> Option<Backlight> {
    read_backlight_in(Path::new("/sys/class/backlight"))
}

/// Set the built-in screen (through logind, no root needed) and external displays (DDC/CI with
/// `ddcutil`, when installed) to `percent`. Returns whether anything changed.
pub fn set_brightness(percent: u32) -> bool {
    let p = percent.clamp(1, 100);
    let mut ok = false;
    if let Some(b) = backlight() {
        let value = ((b.max as f64) * p as f64 / 100.0).round().max(1.0) as u32;
        ok |= sys::run(
            "busctl",
            &["call", "org.freedesktop.login1", "/org/freedesktop/login1/session/auto", "org.freedesktop.login1.Session", "SetBrightness", "ssu", "backlight", &b.name, &value.to_string()],
        );
    }
    if sys::has("ddcutil") {
        ok |= sys::run("ddcutil", &["setvcp", "10", &p.to_string()]);
    }
    ok
}

// ---------------------------------------------------------------------------------------------
// Running apps (quit, force quit)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningApp {
    pub app_id: String,
    pub name: String,
    pub icon: Option<String>,
    pub pids: Vec<u32>,
}

/// The program a desktop entry starts: the first word of `Exec` that is not `env` or a variable,
/// without its folder (`/usr/bin/firefox %u` -> `firefox`).
pub fn exec_program(exec: &str) -> Option<String> {
    // split into words, keeping "quoted paths with spaces" together
    let mut words: Vec<String> = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    for c in exec.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    words.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    let w = words.into_iter().find(|w| !(w == "env" || w.contains('=') || w.starts_with('-')))?;
    if w.starts_with('%') {
        return None;
    }
    Some(w.rsplit('/').next().unwrap_or(&w).to_string())
}

/// Programs that are never offered for quitting: the desktop itself and Menew.
const NEVER_QUIT: [&str; 14] = [
    "menew", "gnome-shell", "plasmashell", "kwin_wayland", "kwin_x11", "Xwayland", "systemd", "dbus-daemon", "pipewire", "wireplumber",
    "gnome-session-binary", "ksmserver", "xdg-desktop-portal", "nautilus-desktop",
];

/// Match the processes of this user (`comm` or the executable's name) with the desktop entries.
/// `procs` is `(pid, uid, comm, exe name)`.
pub fn match_running(apps: &[DesktopApp], procs: &[(u32, u32, String, String)], uid: u32, never: &[String]) -> Vec<RunningApp> {
    let mut out: Vec<RunningApp> = vec![];
    for app in apps {
        if app.terminal {
            continue;
        }
        let Some(prog) = exec_program(&app.exec) else { continue };
        if NEVER_QUIT.contains(&prog.as_str()) || never.iter().any(|n| n.trim_end_matches(".desktop") == app.id.trim_end_matches(".desktop") || n == &prog) {
            continue;
        }
        // comm is cut at 15 characters by the kernel
        let short: String = prog.chars().take(15).collect();
        let pids: Vec<u32> = procs.iter().filter(|p| p.1 == uid && (p.3 == prog || p.2 == short)).map(|p| p.0).collect();
        if !pids.is_empty() && !out.iter().any(|r| r.pids == pids) {
            out.push(RunningApp { app_id: app.id.clone(), name: app.name.clone(), icon: app.icon.clone(), pids });
        }
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// Every process of `/proc`: `(pid, uid, comm, exe name)`.
pub fn processes() -> Vec<(u32, u32, String, String)> {
    let Ok(rd) = std::fs::read_dir("/proc") else { return vec![] };
    rd.filter_map(Result::ok)
        .filter_map(|e| {
            let pid: u32 = e.file_name().to_str()?.parse().ok()?;
            let dir = e.path();
            let comm = std::fs::read_to_string(dir.join("comm")).ok()?.trim().to_string();
            let status = std::fs::read_to_string(dir.join("status")).ok()?;
            let uid = status.lines().find_map(|l| l.strip_prefix("Uid:"))?.split_whitespace().next()?.parse().ok()?;
            let exe = std::fs::read_link(dir.join("exe")).ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).unwrap_or_default();
            Some((pid, uid, comm, exe))
        })
        .collect()
}

pub fn current_uid() -> u32 {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: getuid never fails
        unsafe { libc::getuid() }
    }
    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}

pub fn running_apps(apps: &[DesktopApp], never: &[String]) -> Vec<RunningApp> {
    match_running(apps, &processes(), current_uid(), never)
}

/// SIGTERM (apps can save) or SIGKILL (force quit) to every process of the app.
pub fn quit_pids(pids: &[u32], force: bool) -> bool {
    let sig = if force { 9 } else { 15 };
    let mut ok = false;
    for p in pids {
        ok |= sys::signal(*p, sig);
    }
    ok
}

// ---------------------------------------------------------------------------------------------
// Ports in use
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListeningPort {
    pub port: u16,
    pub pid: u32,
    pub command: String,
}

/// The listening TCP sockets of `/proc/net/tcp` (or `tcp6`): `(port, inode)`.
pub fn parse_proc_net_tcp(text: &str) -> Vec<(u16, u64)> {
    text.lines()
        .skip(1)
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            if f.len() < 10 || f[3] != "0A" {
                return None;
            }
            let port = u16::from_str_radix(f[1].rsplit(':').next()?, 16).ok()?;
            let inode = f[9].parse().ok()?;
            Some((port, inode))
        })
        .collect()
}

/// Servers this user runs, one row per port (like `lsof -iTCP -sTCP:LISTEN` on the Mac).
pub fn listening_ports() -> Vec<ListeningPort> {
    let mut socks: Vec<(u16, u64)> = vec![];
    for f in ["/proc/net/tcp", "/proc/net/tcp6"] {
        socks.extend(parse_proc_net_tcp(&std::fs::read_to_string(f).unwrap_or_default()));
    }
    if socks.is_empty() {
        return vec![];
    }
    let uid = current_uid();
    let mut out: Vec<ListeningPort> = vec![];
    for (pid, puid, comm, _) in processes() {
        if puid != uid {
            continue;
        }
        let Ok(fds) = std::fs::read_dir(format!("/proc/{pid}/fd")) else { continue };
        for fd in fds.filter_map(Result::ok) {
            let Ok(target) = std::fs::read_link(fd.path()) else { continue };
            let t = target.to_string_lossy();
            let Some(inode) = t.strip_prefix("socket:[").and_then(|r| r.strip_suffix(']')).and_then(|n| n.parse::<u64>().ok()) else { continue };
            for (port, _) in socks.iter().filter(|s| s.1 == inode) {
                if !out.iter().any(|p| p.port == *port) {
                    out.push(ListeningPort { port: *port, pid, command: comm.clone() });
                }
            }
        }
    }
    out.sort_by_key(|p| p.port);
    out
}

// ---------------------------------------------------------------------------------------------
// State and palette items
// ---------------------------------------------------------------------------------------------

/// What the quick actions show right now (read when the palette opens).
#[derive(Debug, Clone, Default)]
pub struct ToolState {
    pub mic_muted: Option<bool>,
    pub sound_muted: Option<bool>,
    pub dark: Option<bool>,
    pub awake: bool,
    pub outputs: Vec<AudioDevice>,
    pub inputs: Vec<AudioDevice>,
    pub output: Option<String>,
    pub input: Option<String>,
    pub ejectable: usize,
    pub has_backlight: bool,
}

impl ToolState {
    /// Read everything (a few short process calls; run it off the GTK thread).
    pub fn read(desktop: Desktop) -> Self {
        Self {
            mic_muted: is_muted(true),
            sound_muted: is_muted(false),
            dark: is_dark(desktop),
            awake: false,
            outputs: devices(false),
            inputs: devices(true),
            output: default_device(false),
            input: default_device(true),
            ejectable: ejectable().len(),
            has_backlight: backlight().is_some() || sys::has("ddcutil"),
        }
    }
}

fn cmd(id: &str, title: &str, icon: &'static str, kw: &str, command: &str) -> PaletteItem {
    PaletteItem::new(&format!("action.{id}"), Kind::Action, title).kw(kw).icon(Icon::Symbol(icon)).act(ItemAction::Command(command.to_string()))
}

/// The quick actions of the palette (the Mac's `Palette.commands`, `Module.actions` part).
pub fn items(st: &ToolState, lang: Language) -> Vec<PaletteItem> {
    let mut out = vec![];
    let muted = st.mic_muted == Some(true);
    out.push(
        cmd("mic", &tr(if muted { "qa_mic_on" } else { "qa_mic_off" }, lang, &[]), if muted { "microphone-sensitivity-muted-symbolic" } else { "audio-input-microphone-symbolic" },
            "mic microphone microfone mute mudo silenciar reunião meeting mikrofon micrófono 麦克风 静音", "qa.mic")
        .shortcut("Ctrl+Alt+M"),
    );
    let smuted = st.sound_muted == Some(true);
    out.push(cmd("sound", &tr(if smuted { "qa_sound_on" } else { "qa_sound_off" }, lang, &[]), if smuted { "audio-volume-muted-symbolic" } else { "audio-volume-high-symbolic" },
        "sound som volume audio áudio mute mudo ton sonido 声音 静音", "qa.sound"));
    let dark = st.dark == Some(true);
    out.push(cmd("dark", &tr(if dark { "qa_light" } else { "qa_dark" }, lang, &[]), if dark { "weather-clear-symbolic" } else { "weather-clear-night-symbolic" },
        "dark mode escuro claro light tema theme aparência appearance dunkel oscuro 深色 浅色", "qa.dark"));
    for d in st.outputs.iter().filter(|d| Some(&d.name) != st.output.as_ref()) {
        out.push(cmd(&format!("out.{}", d.name), &tr("qa_output", lang, &[&d.description]), "audio-speakers-symbolic",
            &format!("som saída output audio áudio speaker fone headphone ausgabe salida 输出 {}", d.description), &format!("qa.out:{}", d.name)));
    }
    for d in st.inputs.iter().filter(|d| Some(&d.name) != st.input.as_ref()) {
        out.push(cmd(&format!("in.{}", d.name), &tr("qa_input", lang, &[&d.description]), "audio-input-microphone-symbolic",
            &format!("microfone mic input entrada audio áudio mikrofon micrófono 麦克风 {}", d.description), &format!("qa.in:{}", d.name)));
    }
    let mut awake = cmd("awake", &tr(if st.awake { "qa_awake_off" } else { "qa_awake" }, lang, &[]), "preferences-desktop-screensaver-symbolic",
        "awake acordado caffeine cafe café sleep dormir tela amphetamine wach despierto 唤醒 保持", "qa.awake");
    if !st.awake {
        awake = awake.subtitle(&tr("qa_awake_sub", lang, &[]));
    }
    out.push(awake);
    out.push(cmd("lock", &tr("qa_lock", lang, &[]), "system-lock-screen-symbolic", "lock bloquear tela screen sperren bloquear pantalla 锁定 锁屏", "qa.lock"));
    out.push(cmd("trash", &tr("qa_trash", lang, &[]), "user-trash-full-symbolic", "trash lixeira lixo esvaziar empty papierkorb papelera 废纸篓 清空", "qa.trash"));
    out.push(cmd("color", &tr("qa_color", lang, &[]), "color-select-symbolic", "color cor picker conta-gotas eyedropper hex farbe pipette 颜色 取色", "qa.color"));
    out.push(cmd("plain", &tr("qa_plain", lang, &[]), "edit-clear-symbolic", "plain text texto puro sem formatação formatting clipboard reiner text texto plano 纯文本", "qa.plain"));
    if st.ejectable > 0 {
        out.push(cmd("eject", &tr("qa_eject", lang, &[]), "media-eject-symbolic", "eject ejetar disco disk usb pendrive auswerfen expulsar 弹出", "qa.eject")
            .subtitle(&tr("qa_eject_sub", lang, &[&st.ejectable.to_string()])));
    }
    if st.has_backlight {
        out.push(cmd("brightness", &tr("qa_brightness", lang, &[]), "display-brightness-symbolic", "brightness brilho tela screen helligkeit brillo 亮度", "")
            .subtitle(&tr("qa_brightness_sub", lang, &[])).forget().keep_open().act(ItemAction::FillQuery(tr("qa_brightness_word", lang, &[]) + " ")));
    }
    out.push(cmd("quitall", &tr("qa_quit_all", lang, &[]), "window-close-symbolic", "fechar todos quit all close apps encerrar sair alle beenden cerrar todas 退出 全部", "qa.quitall"));
    out.push(cmd("ports", &tr("qa_ports", lang, &[]), "network-server-symbolic", "port porta ports portas server servidor localhost listen dev stop kill", "")
        .forget().keep_open().act(ItemAction::FillQuery(tr("qa_ports_word", lang, &[]) + " ")));
    out
}

/// Typed items: `awake 2h`, `brightness 50`, `quit firefox`, `port 3000`.
pub fn typed_items(q: &str, running: &dyn Fn() -> Vec<RunningApp>, ports: &dyn Fn() -> Vec<ListeningPort>, lang: Language) -> Vec<PaletteItem> {
    let q = fold(q.trim());
    let (word, rest) = match q.split_once(' ') {
        Some((w, r)) => (w, r.trim()),
        None => (q.as_str(), ""),
    };
    let mut out = vec![];
    let is = |words: &[&str]| words.iter().any(|w| fold(w) == word);
    if is(&["awake", "acordado", "wach", "despierto", "唤醒", "caffeine"]) {
        if let Some(m) = parse_duration_minutes(rest) {
            out.push(PaletteItem::new("qa.awake.timed", Kind::Action, &tr("qa_awake_for", lang, &[&crate::timers::span_text(m as f64 * 60.0)]))
                .kw(&q).icon(Icon::Symbol("preferences-desktop-screensaver-symbolic")).forget().act(ItemAction::Command(format!("qa.awake:{m}"))));
        }
    }
    if is(&["brightness", "brilho", "helligkeit", "brillo", "亮度"]) {
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(p) = digits.parse::<u32>() {
            if (1..=100).contains(&p) {
                out.push(PaletteItem::new("qa.brightness.set", Kind::Action, &tr("qa_brightness_to", lang, &[&p.to_string()]))
                    .kw(&q).icon(Icon::Symbol("display-brightness-symbolic")).forget().act(ItemAction::Command(format!("qa.bright:{p}"))));
            }
        }
    }
    if is(&["quit", "fechar", "close", "beenden", "cerrar", "退出", "kill", "force", "forcar"]) {
        let force = is(&["kill", "force", "forcar"]);
        let needle = fold(rest);
        for app in running() {
            if needle.is_empty() || fold(&app.name).contains(&needle) {
                let pids = app.pids.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",");
                let key = if force { "qa_force_quit_app" } else { "qa_quit_app" };
                out.push(PaletteItem::new(&format!("qa.quit.{}", app.app_id), Kind::Action, &tr(key, lang, &[&app.name]))
                    .kw(&q).icon(Icon::App(app.icon.clone())).forget().act(ItemAction::Command(format!("qa.{}:{pids}", if force { "force" } else { "quit" }))));
            }
        }
    }
    if is(&["port", "porta", "ports", "portas", "puerto", "端口"]) {
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        for p in ports() {
            if digits.is_empty() || p.port.to_string().starts_with(&digits) {
                out.push(PaletteItem::new(&format!("qa.port.{}", p.port), Kind::Action, &tr("qa_port_stop", lang, &[&p.port.to_string(), &p.command]))
                    .subtitle(&format!("PID {}", p.pid)).kw(&q).icon(Icon::Symbol("network-server-symbolic")).forget()
                    .act(ItemAction::Command(format!("qa.port:{}", p.pid))));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_devices_from_pactl_json() {
        let json = r#"[{"index":1,"name":"alsa_output.pci.analog-stereo","description":"Built-in Audio"},
                      {"index":2,"name":"alsa_output.pci.analog-stereo.monitor","description":"Monitor of Built-in"},
                      {"index":3,"name":"bluez_output.AA","description":"Headphones"}]"#;
        let d = parse_devices(json);
        assert_eq!(d.len(), 2);
        assert_eq!(d[1].description, "Headphones");
        assert!(parse_devices("not json").is_empty());
    }

    #[test]
    fn mute_state_from_pactl_and_wpctl() {
        assert_eq!(parse_muted("Mute: yes\n"), Some(true));
        assert_eq!(parse_muted("Mute: no"), Some(false));
        assert_eq!(parse_muted("Volume: 0.40 [MUTED]"), Some(true));
        assert_eq!(parse_muted("Volume: 0.40"), Some(false));
        assert_eq!(parse_muted("??"), None);
    }

    #[test]
    fn dark_mode_state() {
        assert!(parse_gnome_scheme("'prefer-dark'\n"));
        assert!(!parse_gnome_scheme("'default'"));
        let kde = "[Colors:View]\nColorScheme=Nope\n[General]\nColorScheme=BreezeDark\nfont=x\n";
        assert_eq!(parse_kde_scheme(kde).as_deref(), Some("BreezeDark"));
        assert_eq!(parse_kde_scheme("[General]\nfoo=1\n"), None);
    }

    #[test]
    fn durations_like_the_mac() {
        assert_eq!(parse_duration_minutes("2h"), Some(120));
        assert_eq!(parse_duration_minutes("90m"), Some(90));
        assert_eq!(parse_duration_minutes("45"), Some(45));
        assert_eq!(parse_duration_minutes("1 hora"), Some(60));
        assert_eq!(parse_duration_minutes("0"), None);
        assert_eq!(parse_duration_minutes("25h"), None);
        assert_eq!(parse_duration_minutes("2x"), None);
    }

    #[test]
    fn only_removable_mounted_volumes_are_ejectable() {
        let json = r#"{"blockdevices":[
          {"path":"/dev/nvme0n1","pkname":null,"rm":false,"hotplug":false,"mountpoint":null,"type":"disk","children":[
             {"path":"/dev/nvme0n1p1","pkname":"nvme0n1","rm":false,"hotplug":false,"mountpoint":"/boot/efi","type":"part"},
             {"path":"/dev/nvme0n1p2","pkname":"nvme0n1","rm":false,"hotplug":false,"mountpoint":"/","type":"part"}]},
          {"path":"/dev/sdb","pkname":null,"rm":true,"hotplug":true,"mountpoint":null,"type":"disk","children":[
             {"path":"/dev/sdb1","pkname":"sdb","rm":true,"hotplug":true,"mountpoint":"/media/u/STICK","type":"part"},
             {"path":"/dev/sdb2","pkname":"sdb","rm":true,"hotplug":true,"mountpoint":null,"type":"part"}]},
          {"path":"/dev/sdc","pkname":null,"rm":"0","hotplug":"1","mountpoint":"/run/media/u/EXT","type":"disk"}
        ]}"#;
        let v = parse_lsblk(json);
        assert_eq!(v.len(), 2, "{v:?}");
        assert_eq!(v[0], Volume { device: "/dev/sdb1".into(), disk: Some("/dev/sdb".into()), mountpoint: "/media/u/STICK".into() });
        assert_eq!(v[1].device, "/dev/sdc");
    }

    #[test]
    fn backlight_from_sysfs() {
        let dir = tempfile::tempdir().unwrap();
        let b = dir.path().join("intel_backlight");
        std::fs::create_dir(&b).unwrap();
        std::fs::write(b.join("brightness"), "480\n").unwrap();
        std::fs::write(b.join("max_brightness"), "960\n").unwrap();
        std::fs::write(b.join("type"), "raw\n").unwrap();
        let a = dir.path().join("acpi_video0");
        std::fs::create_dir(&a).unwrap();
        std::fs::write(a.join("brightness"), "1\n").unwrap();
        std::fs::write(a.join("max_brightness"), "10\n").unwrap();
        std::fs::write(a.join("type"), "firmware\n").unwrap();
        let bl = read_backlight_in(dir.path()).unwrap();
        assert_eq!(bl.name, "intel_backlight");
        assert_eq!(bl.percent(), 50);
    }

    #[test]
    fn exec_program_of_desktop_entries() {
        assert_eq!(exec_program("/usr/bin/firefox %u").as_deref(), Some("firefox"));
        assert_eq!(exec_program("env GDK_BACKEND=x11 code --new-window %F").as_deref(), Some("code"));
        assert_eq!(exec_program("\"/opt/My App/app\" --x").as_deref(), Some("app"));
        assert_eq!(exec_program("%u"), None);
    }

    fn app(id: &str, name: &str, exec: &str) -> DesktopApp {
        DesktopApp::parse_content(&format!("[Desktop Entry]\nType=Application\nName={name}\nExec={exec}\n"), Path::new(&format!("/x/{id}"))).unwrap()
    }

    #[test]
    fn running_apps_are_matched_by_program_and_owner() {
        let apps = vec![app("firefox.desktop", "Firefox", "firefox %u"), app("gedit.desktop", "Text Editor", "gedit"), app("menew.desktop", "Menew", "menew")];
        let procs = vec![
            (100, 1000, "firefox".to_string(), "firefox".to_string()),
            (101, 1000, "Isolated Web Co".to_string(), "firefox".to_string()),
            (200, 0, "gedit".to_string(), "gedit".to_string()),
            (300, 1000, "menew".to_string(), "menew".to_string()),
        ];
        let r = match_running(&apps, &procs, 1000, &[]);
        assert_eq!(r.len(), 1, "{r:?}");
        assert_eq!(r[0].pids, vec![100, 101]);
        assert!(match_running(&apps, &procs, 1000, &["firefox.desktop".to_string()]).is_empty());
    }

    #[test]
    fn listening_sockets_from_proc_net_tcp() {
        let text = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n   0: 0100007F:0BB8 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 12345 1 0000000000000000 100 0 0 10 0\n   1: 0100007F:A1B2 0100007F:0BB8 01 00000000:00000000 00:00000000 00000000  1000        0 999 1\n";
        assert_eq!(parse_proc_net_tcp(text), vec![(3000, 12345)]);
    }

    #[test]
    fn typed_quick_actions() {
        let lang = Language::En;
        let none_apps = || vec![];
        let none_ports = || vec![];
        let a = typed_items("awake 2h", &none_apps, &none_ports, lang);
        assert_eq!(a[0].action, ItemAction::Command("qa.awake:120".into()));
        let b = typed_items("brilho 40", &none_apps, &none_ports, Language::Pt);
        assert_eq!(b[0].action, ItemAction::Command("qa.bright:40".into()));
        assert!(typed_items("brightness 400", &none_apps, &none_ports, lang).is_empty());
        let apps = || vec![RunningApp { app_id: "f".into(), name: "Firefox".into(), icon: None, pids: vec![5, 6] }];
        let q = typed_items("quit fire", &apps, &none_ports, lang);
        assert_eq!(q[0].action, ItemAction::Command("qa.quit:5,6".into()));
        let k = typed_items("kill fire", &apps, &none_ports, lang);
        assert_eq!(k[0].action, ItemAction::Command("qa.force:5,6".into()));
        let ports = || vec![ListeningPort { port: 3000, pid: 42, command: "node".into() }, ListeningPort { port: 8080, pid: 43, command: "python3".into() }];
        let p = typed_items("port 30", &none_apps, &ports, lang);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].action, ItemAction::Command("qa.port:42".into()));
        assert_eq!(typed_items("port", &none_apps, &ports, lang).len(), 2);
    }

    #[test]
    fn items_follow_the_state() {
        let st = ToolState {
            mic_muted: Some(true),
            outputs: vec![AudioDevice { name: "a".into(), description: "Speakers".into() }, AudioDevice { name: "b".into(), description: "Headphones".into() }],
            output: Some("a".into()),
            ..ToolState::default()
        };
        let items = items(&st, Language::En);
        let mic = items.iter().find(|i| i.id == "action.mic").unwrap();
        assert_eq!(mic.title, tr("qa_mic_on", Language::En, &[]));
        assert!(items.iter().any(|i| i.action == ItemAction::Command("qa.out:b".into())));
        assert!(!items.iter().any(|i| i.action == ItemAction::Command("qa.out:a".into())));
        assert!(!items.iter().any(|i| i.id == "action.eject"));
    }
}
