//! System monitor (port of `SystemMonitor.swift`): CPU, GPU, memory, swap, disk space and speed,
//! battery, network and the top processes, read from `/proc` and `/sys` once a second. Nothing is
//! sent anywhere and nothing is kept on disk.
//!
//! The parsers are pure and tested with real samples; [`Monitor`] keeps the previous sample to turn
//! counters into rates.

use crate::i18n::Language;
use crate::strings::tr;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CpuTimes {
    pub busy: u64,
    pub total: u64,
}

/// The first `cpu` line of `/proc/stat` (user nice system idle iowait irq softirq steal).
pub fn parse_cpu(stat: &str) -> Option<CpuTimes> {
    let line = stat.lines().find(|l| l.starts_with("cpu "))?;
    let v: Vec<u64> = line.split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
    if v.len() < 4 {
        return None;
    }
    // guest and guest_nice are already counted in user and nice
    let total: u64 = v.iter().take(8).sum();
    let idle = v[3] + v.get(4).copied().unwrap_or(0);
    Some(CpuTimes { busy: total - idle, total })
}

/// Number of `cpuN` lines.
pub fn parse_cpu_count(stat: &str) -> usize {
    stat.lines().filter(|l| l.starts_with("cpu") && l.as_bytes().get(3).is_some_and(|b| b.is_ascii_digit())).count().max(1)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Memory {
    pub total: u64,
    pub available: u64,
    pub swap_total: u64,
    pub swap_free: u64,
}

impl Memory {
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.available)
    }
    pub fn used_percent(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.used() as f64 * 100.0 / self.total as f64
        }
    }
    pub fn swap_used(&self) -> u64 {
        self.swap_total.saturating_sub(self.swap_free)
    }
}

/// `/proc/meminfo` (values in kB, returned in bytes).
pub fn parse_meminfo(text: &str) -> Memory {
    let mut m = Memory::default();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let (Some(k), Some(v)) = (it.next(), it.next()) else { continue };
        let bytes = v.parse::<u64>().unwrap_or(0) * 1024;
        match k {
            "MemTotal:" => m.total = bytes,
            "MemAvailable:" => m.available = bytes,
            "SwapTotal:" => m.swap_total = bytes,
            "SwapFree:" => m.swap_free = bytes,
            _ => {}
        }
    }
    m
}

/// Bytes received and sent by every interface but loopback (`/proc/net/dev`).
pub fn parse_net_dev(text: &str) -> (u64, u64) {
    let mut rx = 0;
    let mut tx = 0;
    for line in text.lines().skip(2) {
        let Some((name, rest)) = line.split_once(':') else { continue };
        let name = name.trim();
        if name == "lo" || name.starts_with("veth") || name.starts_with("docker") || name.starts_with("br-") || name.starts_with("virbr") {
            continue;
        }
        let v: Vec<u64> = rest.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        if v.len() >= 9 {
            rx += v[0];
            tx += v[8];
        }
    }
    (rx, tx)
}

/// Whole disks (not partitions) of `/proc/diskstats`: sectors read and written, in bytes.
pub fn parse_diskstats(text: &str) -> (u64, u64) {
    let mut r = 0;
    let mut w = 0;
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 10 {
            continue;
        }
        let name = f[2];
        let whole = (name.starts_with("sd") || name.starts_with("vd") || name.starts_with("xvd")) && !name.ends_with(|c: char| c.is_ascii_digit())
            || (name.starts_with("nvme") && !name.contains('p'))
            || (name.starts_with("mmcblk") && !name.contains('p'));
        if whole {
            r += f[5].parse::<u64>().unwrap_or(0) * 512;
            w += f[9].parse::<u64>().unwrap_or(0) * 512;
        }
    }
    (r, w)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Battery {
    pub percent: u32,
    /// `Charging`, `Discharging`, `Full`, `Not charging`.
    pub status: String,
    /// Full charge now against the design, in percent.
    pub health: Option<u32>,
    pub cycles: Option<u32>,
    /// Watts drawn or charged.
    pub power: Option<f64>,
}

impl Battery {
    pub fn charging(&self) -> bool {
        self.status == "Charging" || self.status == "Full"
    }
}

pub fn read_battery_in(dir: &Path) -> Option<Battery> {
    let mut names: Vec<_> = std::fs::read_dir(dir).ok()?.filter_map(Result::ok).collect();
    names.sort_by_key(|e| e.file_name());
    let bat = names.into_iter().find(|e| {
        std::fs::read_to_string(e.path().join("type")).map(|t| t.trim() == "Battery").unwrap_or(false)
            && std::fs::read_to_string(e.path().join("scope")).map(|s| s.trim() != "Device").unwrap_or(true)
    })?;
    let p = bat.path();
    let read = |f: &str| std::fs::read_to_string(p.join(f)).ok().map(|s| s.trim().to_string());
    let num = |f: &str| read(f).and_then(|s| s.parse::<f64>().ok());
    let percent = num("capacity")? as u32;
    let full = num("energy_full").or_else(|| num("charge_full"));
    let design = num("energy_full_design").or_else(|| num("charge_full_design"));
    let health = match (full, design) {
        (Some(f), Some(d)) if d > 0.0 => Some(((f / d) * 100.0).round().min(100.0) as u32),
        _ => None,
    };
    let power = num("power_now").map(|uw| uw / 1_000_000.0).or_else(|| match (num("current_now"), num("voltage_now")) {
        (Some(c), Some(v)) => Some(c * v / 1e12),
        _ => None,
    });
    Some(Battery {
        percent: percent.min(100),
        status: read("status").unwrap_or_default(),
        health,
        cycles: num("cycle_count").map(|c| c as u32).filter(|c| *c > 0),
        power: power.filter(|w| *w > 0.0),
    })
}

/// GPU use in percent: amdgpu (and Intel Xe) expose `gpu_busy_percent`.
pub fn read_gpu_in(drm: &Path) -> Option<u32> {
    let mut best: Option<u32> = None;
    for e in std::fs::read_dir(drm).ok()?.filter_map(Result::ok) {
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }
        if let Some(v) = std::fs::read_to_string(e.path().join("device/gpu_busy_percent")).ok().and_then(|s| s.trim().parse::<u32>().ok()) {
            best = Some(best.map_or(v, |b| b.max(v)));
        }
    }
    best
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Disk {
    pub total: u64,
    pub free: u64,
}

impl Disk {
    pub fn used_percent(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.total - self.free) as f64 * 100.0 / self.total as f64
        }
    }
}

pub fn disk_space(path: &str) -> Option<Disk> {
    #[cfg(target_os = "linux")]
    {
        let c = std::ffi::CString::new(path).ok()?;
        // SAFETY: statvfs writes into the zeroed struct we own
        let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
            return None;
        }
        let bs = st.f_frsize as u64;
        Some(Disk { total: st.f_blocks as u64 * bs, free: st.f_bavail as u64 * bs })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        None
    }
}

/// One process line: `(pid, name, cpu ticks, rss bytes)` from `/proc/PID/stat`.
pub fn parse_proc_stat(text: &str, page: u64) -> Option<(String, u64, u64)> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    let name = text[open + 1..close].to_string();
    let rest: Vec<&str> = text[close + 1..].split_whitespace().collect();
    // after the name: state(0) ppid(1) ... utime(11) stime(12) ... rss(21)
    let utime: u64 = rest.get(11)?.parse().ok()?;
    let stime: u64 = rest.get(12)?.parse().ok()?;
    let rss: u64 = rest.get(21)?.parse().ok()?;
    Some((name, utime + stime, rss * page))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Process {
    pub pid: u32,
    pub name: String,
    /// Percent of one core, like `top`.
    pub cpu: f64,
    pub memory: u64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub cpu: f64,
    pub cores: usize,
    pub gpu: Option<u32>,
    pub memory: Memory,
    pub disk: Option<Disk>,
    pub disk_read: f64,
    pub disk_write: f64,
    pub net_down: f64,
    pub net_up: f64,
    pub battery: Option<Battery>,
    pub uptime: f64,
    pub load: Option<(f64, f64, f64)>,
    pub top_cpu: Vec<Process>,
    pub top_memory: Vec<Process>,
}

/// Keeps the counters of the last sample.
#[derive(Default)]
pub struct Monitor {
    cpu: Option<CpuTimes>,
    net: Option<(u64, u64, f64)>,
    disk: Option<(u64, u64, f64)>,
    procs: HashMap<u32, u64>,
    procs_at: f64,
}

fn read(p: &str) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

fn clock_ticks() -> f64 {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: sysconf has no side effects
        let t = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        if t > 0 {
            return t as f64;
        }
    }
    100.0
}

fn page_size() -> u64 {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: sysconf has no side effects
        let p = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if p > 0 {
            return p as u64;
        }
    }
    4096
}

pub fn rate(prev: Option<(u64, u64, f64)>, now: (u64, u64), t: f64) -> (f64, f64) {
    match prev {
        Some((a, b, t0)) if t > t0 => (now.0.saturating_sub(a) as f64 / (t - t0), now.1.saturating_sub(b) as f64 / (t - t0)),
        _ => (0.0, 0.0),
    }
}

impl Monitor {
    /// Read everything once. `now` is in seconds (any monotonic clock).
    pub fn sample(&mut self, now: f64, with_processes: bool) -> Snapshot {
        let stat = read("/proc/stat");
        let cpu_now = parse_cpu(&stat);
        let cpu = match (self.cpu, cpu_now) {
            (Some(a), Some(b)) if b.total > a.total => (b.busy - a.busy.min(b.busy)) as f64 * 100.0 / (b.total - a.total) as f64,
            _ => 0.0,
        };
        self.cpu = cpu_now;
        let net = parse_net_dev(&read("/proc/net/dev"));
        let (down, up) = rate(self.net, net, now);
        self.net = Some((net.0, net.1, now));
        let disk = parse_diskstats(&read("/proc/diskstats"));
        let (dr, dw) = rate(self.disk, disk, now);
        self.disk = Some((disk.0, disk.1, now));
        let load = {
            let l: Vec<f64> = read("/proc/loadavg").split_whitespace().take(3).filter_map(|x| x.parse().ok()).collect();
            (l.len() == 3).then(|| (l[0], l[1], l[2]))
        };
        let (top_cpu, top_memory) = if with_processes { self.processes(now) } else { (vec![], vec![]) };
        Snapshot {
            cpu: cpu.clamp(0.0, 100.0),
            cores: parse_cpu_count(&stat),
            gpu: read_gpu_in(Path::new("/sys/class/drm")),
            memory: parse_meminfo(&read("/proc/meminfo")),
            disk: disk_space("/"),
            disk_read: dr,
            disk_write: dw,
            net_down: down,
            net_up: up,
            battery: read_battery_in(Path::new("/sys/class/power_supply")),
            uptime: read("/proc/uptime").split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0.0),
            load,
            top_cpu,
            top_memory,
        }
    }

    fn processes(&mut self, now: f64) -> (Vec<Process>, Vec<Process>) {
        let uid = crate::quick_actions::current_uid();
        let page = page_size();
        let hz = clock_ticks();
        let elapsed = now - self.procs_at;
        let mut seen: HashMap<u32, u64> = HashMap::new();
        let mut list: Vec<Process> = vec![];
        let Ok(rd) = std::fs::read_dir("/proc") else { return (vec![], vec![]) };
        for e in rd.filter_map(Result::ok) {
            let Some(pid) = e.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else { continue };
            let Some((name, ticks, rss)) = parse_proc_stat(&read(&format!("/proc/{pid}/stat")), page) else { continue };
            seen.insert(pid, ticks);
            let owner = read(&format!("/proc/{pid}/status")).lines().find_map(|l| l.strip_prefix("Uid:")).and_then(|u| u.split_whitespace().next()?.parse::<u32>().ok());
            if owner != Some(uid) || rss == 0 {
                continue;
            }
            let cpu = match self.procs.get(&pid) {
                Some(prev) if elapsed > 0.0 => ticks.saturating_sub(*prev) as f64 / hz / elapsed * 100.0,
                _ => 0.0,
            };
            list.push(Process { pid, name, cpu, memory: rss });
        }
        self.procs = seen;
        self.procs_at = now;
        let mut by_cpu = list.clone();
        by_cpu.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
        by_cpu.truncate(5);
        list.sort_by_key(|p| std::cmp::Reverse(p.memory));
        list.truncate(5);
        (by_cpu, list)
    }
}

/// "1.2 GB", "340 MB" (decimal units, like the Mac's `ByteCountFormatter`).
pub fn bytes(n: u64, lang: Language) -> String {
    let n = n as f64;
    let (v, unit) = if n >= 1e12 {
        (n / 1e12, "TB")
    } else if n >= 1e9 {
        (n / 1e9, "GB")
    } else if n >= 1e6 {
        (n / 1e6, "MB")
    } else if n >= 1e3 {
        (n / 1e3, "KB")
    } else {
        return format!("{} B", n as u64);
    };
    let digits = if v < 10.0 { 1 } else { 0 };
    format!("{} {unit}", crate::format::format_number(v, digits, lang))
}

/// "1.2 MB/s"
pub fn speed(bps: f64, lang: Language) -> String {
    format!("{}/s", bytes(bps.max(0.0) as u64, lang))
}

/// "3 d 4 h", "5 h 12 min"
pub fn uptime_text(secs: f64) -> String {
    let m = (secs / 60.0) as u64;
    let (d, h, min) = (m / 1440, (m / 60) % 24, m % 60);
    if d > 0 {
        format!("{d} d {h} h")
    } else if h > 0 {
        format!("{h} h {min} min")
    } else {
        format!("{min} min")
    }
}

/// The metrics people can put in the tray.
pub const TRAY_METRICS: [&str; 6] = ["cpu", "memory", "gpu", "disk", "network", "battery"];

/// The tray text for the chosen metrics: "CPU 12% · RAM 45%".
pub fn tray_text(s: &Snapshot, metrics: &[String], lang: Language) -> String {
    let mut parts = vec![];
    for m in metrics {
        match m.as_str() {
            "cpu" => parts.push(format!("CPU {:.0}%", s.cpu)),
            "memory" => parts.push(format!("{} {:.0}%", tr("mon_ram", lang, &[]), s.memory.used_percent())),
            "gpu" => {
                if let Some(g) = s.gpu {
                    parts.push(format!("GPU {g}%"))
                }
            }
            "disk" => {
                if let Some(d) = &s.disk {
                    parts.push(format!("{} {}", tr("mon_disk", lang, &[]), bytes(d.free, lang)))
                }
            }
            "network" => parts.push(format!("↓{} ↑{}", speed(s.net_down, lang), speed(s.net_up, lang))),
            "battery" => {
                if let Some(b) = &s.battery {
                    parts.push(format!("{}{}%", if b.charging() { "⚡" } else { "" }, b.percent))
                }
            }
            _ => {}
        }
    }
    parts.join(" · ")
}

/// "All good", or what needs attention (the Mac's status line on the Now tab).
pub fn attention(s: &Snapshot, lang: Language) -> Vec<String> {
    let mut out = vec![];
    if s.memory.used_percent() > 90.0 {
        out.push(tr("mon_warn_memory", lang, &[&format!("{:.0}", s.memory.used_percent())]));
    }
    if let Some(d) = &s.disk {
        if d.free < 5_000_000_000 || d.used_percent() > 95.0 {
            out.push(tr("mon_warn_disk", lang, &[&bytes(d.free, lang)]));
        }
    }
    if let Some(b) = &s.battery {
        if !b.charging() && b.percent <= 10 {
            out.push(tr("mon_warn_battery", lang, &[&b.percent.to_string()]));
        }
    }
    if s.cpu > 90.0 {
        out.push(tr("mon_warn_cpu", lang, &[&format!("{:.0}", s.cpu)]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_from_proc_stat() {
        let a = parse_cpu("cpu  100 0 50 800 50 0 0 0 0 0\ncpu0 1 2 3 4\ncpu1 1 2 3 4\n").unwrap();
        assert_eq!(a, CpuTimes { busy: 150, total: 1000 });
        assert_eq!(parse_cpu_count("cpu  1 2 3 4\ncpu0 1 2 3 4\ncpu1 1 2 3 4\nintr 5\n"), 2);
        assert_eq!(parse_cpu("nothing"), None);
    }

    #[test]
    fn memory_from_meminfo() {
        let m = parse_meminfo("MemTotal:       16000000 kB\nMemFree:  100 kB\nMemAvailable:    4000000 kB\nSwapTotal: 2000 kB\nSwapFree: 500 kB\n");
        assert_eq!(m.total, 16_000_000 * 1024);
        assert_eq!(m.used_percent().round(), 75.0);
        assert_eq!(m.swap_used(), 1500 * 1024);
    }

    #[test]
    fn network_skips_loopback_and_bridges() {
        let t = "Inter-|   Receive    |  Transmit\n face |bytes packets errs drop fifo frame compressed multicast|bytes packets\n    lo: 999 1 0 0 0 0 0 0 999 1 0 0 0 0 0 0\n  eth0: 1000 10 0 0 0 0 0 0 500 5 0 0 0 0 0 0\ndocker0: 77 1 0 0 0 0 0 0 77 1 0 0 0 0 0 0\n wlan0: 24 1 0 0 0 0 0 0 6 1 0 0 0 0 0 0\n";
        assert_eq!(parse_net_dev(t), (1024, 506));
    }

    #[test]
    fn disk_io_counts_whole_disks_only() {
        let t = "   8       0 sda 100 0 2 0 50 0 4 0 0 0 0\n   8       1 sda1 100 0 2000 0 50 0 4000 0 0 0 0\n 259 0 nvme0n1 1 0 10 0 1 0 20 0 0 0 0\n 259 1 nvme0n1p1 1 0 10 0 1 0 20 0 0 0 0\n";
        assert_eq!(parse_diskstats(t), (12 * 512, 24 * 512));
    }

    #[test]
    fn process_line_with_spaces_and_parens_in_the_name() {
        let line = "1234 (Web Content (x)) S 1 2 3 4 5 6 7 8 9 10 300 200 0 0 20 0 1 0 100 1000 250 18446744073709551615";
        let (name, ticks, rss) = parse_proc_stat(line, 4096).unwrap();
        assert_eq!(name, "Web Content (x)");
        assert_eq!(ticks, 500);
        assert_eq!(rss, 250 * 4096);
    }

    #[test]
    fn battery_from_sysfs() {
        let dir = tempfile::tempdir().unwrap();
        let ac = dir.path().join("AC");
        std::fs::create_dir(&ac).unwrap();
        std::fs::write(ac.join("type"), "Mains\n").unwrap();
        let mouse = dir.path().join("hidpp_battery_0");
        std::fs::create_dir(&mouse).unwrap();
        std::fs::write(mouse.join("type"), "Battery\n").unwrap();
        std::fs::write(mouse.join("scope"), "Device\n").unwrap();
        std::fs::write(mouse.join("capacity"), "5\n").unwrap();
        let b = dir.path().join("BAT0");
        std::fs::create_dir(&b).unwrap();
        for (f, v) in [("type", "Battery"), ("capacity", "81"), ("status", "Discharging"), ("energy_full", "45000000"), ("energy_full_design", "50000000"), ("cycle_count", "312"), ("power_now", "7500000")] {
            std::fs::write(b.join(f), format!("{v}\n")).unwrap();
        }
        let bat = read_battery_in(dir.path()).unwrap();
        assert_eq!(bat.percent, 81);
        assert_eq!(bat.health, Some(90));
        assert_eq!(bat.cycles, Some(312));
        assert_eq!(bat.power, Some(7.5));
        assert!(!bat.charging());
    }

    #[test]
    fn rates_need_two_samples() {
        assert_eq!(rate(None, (10, 10), 1.0), (0.0, 0.0));
        assert_eq!(rate(Some((0, 0, 0.0)), (2000, 1000), 2.0), (1000.0, 500.0));
    }

    #[test]
    fn texts() {
        assert_eq!(bytes(1_234_000_000, Language::En), "1.2 GB");
        assert_eq!(bytes(1_234_000_000, Language::Pt), "1,2 GB");
        assert_eq!(bytes(340_000_000, Language::En), "340 MB");
        assert_eq!(bytes(12, Language::En), "12 B");
        assert_eq!(uptime_text(3.0 * 86400.0 + 4.0 * 3600.0), "3 d 4 h");
        assert_eq!(uptime_text(5.0 * 3600.0 + 12.0 * 60.0), "5 h 12 min");
        let s = Snapshot { cpu: 12.4, memory: Memory { total: 100, available: 55, ..Memory::default() }, battery: Some(Battery { percent: 80, status: "Charging".into(), ..Battery::default() }), ..Snapshot::default() };
        assert_eq!(tray_text(&s, &["cpu".into(), "memory".into(), "battery".into(), "gpu".into()], Language::En), "CPU 12% · RAM 45% · ⚡80%");
        assert!(attention(&s, Language::En).is_empty());
        let full = Snapshot { memory: Memory { total: 100, available: 5, ..Memory::default() }, ..Snapshot::default() };
        assert_eq!(attention(&full, Language::En).len(), 1);
    }
}
