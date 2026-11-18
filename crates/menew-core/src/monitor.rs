//! System monitor: parsing of `/proc` and `/sys`, the arithmetic between two samples, formatting and
//! the choice of the top processes. Port of `SystemMonitor.swift` (the Mac app is the specification,
//! see `docs/monitor-spec.md`). Everything here is pure and takes text, so the tests run on captured
//! `/proc` files; only the `sys` module and [`Sampler`] touch the machine.

use crate::format::decimal_sep;
use crate::i18n::Language;
use crate::strings::tr;
use std::collections::HashMap;

/// How many processes each top list shows (`SystemMonitor.swift`: `prefix(5)` in the card).
pub const TOP_COUNT: usize = 5;
/// Seconds between two samples (`SystemMonitor.start`: a 2 s timer).
pub const INTERVAL_SECS: u32 = 2;
/// Points of the CPU graph (`cpuHistory`: 60 values, "last 2 min").
pub const CPU_HISTORY: usize = 60;
/// Points of the network graphs (`netInHistory`: 40 values).
pub const NET_HISTORY: usize = 40;

/// What the Mac calls memory pressure. Linux has no such level, so it is derived from the share of
/// memory still available (see the spec): normal >= 25%, warning 10-25%, critical < 10%.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressure {
    Normal,
    Warning,
    Critical,
}

impl MemoryPressure {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryPressure::Normal => "normal",
            MemoryPressure::Warning => "warning",
            MemoryPressure::Critical => "critical",
        }
    }

    pub fn string_key(&self) -> &'static str {
        match self {
            MemoryPressure::Normal => "mon_pressure_normal",
            MemoryPressure::Warning => "mon_pressure_warning",
            MemoryPressure::Critical => "mon_pressure_critical",
        }
    }
}

/// Apps / System / Compressed of the memory card (`MemoryBreakdown`). The three always add up to `used`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemoryBreakdown {
    pub app: u64,
    pub wired: u64,
    pub compressed: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemInfo {
    pub total: u64,
    pub available: u64,
    /// `total - available` (what `free` and the GNOME system monitor call used).
    pub used: u64,
    /// `used / total` (`memFraction`).
    pub fraction: f64,
    pub breakdown: MemoryBreakdown,
    pub swap_total: u64,
    pub swap_used: u64,
    pub pressure: MemoryPressure,
}

/// Busy and total ticks of the whole CPU and of each core (`/proc/stat`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CpuSample {
    pub busy: u64,
    pub total: u64,
    pub cores: Vec<(u64, u64)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetSample {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiskInfo {
    pub total: u64,
    pub free: u64,
    pub used: u64,
    pub fraction: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatteryInfo {
    pub percent: u8,
    pub charging: bool,
    pub on_ac: bool,
    pub minutes_left: Option<u32>,
}

/// One process of a top list. `cpu` is in percent of one core (100 = one core busy, like `ps`).
#[derive(Debug, Clone, PartialEq)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
    pub cpu: f64,
    pub mem_kb: u64,
}

/// What `/proc/<pid>/stat` says about one process, plus its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcStat {
    pub pid: u32,
    pub name: String,
    /// utime + stime, in clock ticks.
    pub ticks: u64,
    pub rss_kb: u64,
    pub uid: u32,
}

// ---------------------------------------------------------------------------
// CPU (SystemMonitor.readCPU, readCores)
// ---------------------------------------------------------------------------

/// Parse `/proc/stat`. Mac: busy = user + system + nice, total = busy + idle. Linux adds irq, softirq
/// and steal to busy (they are work, the Mac has no such counters) and counts iowait as idle.
pub fn parse_proc_stat(content: &str) -> CpuSample {
    let mut sample = CpuSample::default();
    for line in content.lines() {
        let mut it = line.split_whitespace();
        let Some(name) = it.next() else { continue };
        let is_total = name == "cpu";
        let is_core = name.len() > 3 && name.starts_with("cpu") && name[3..].chars().all(|c| c.is_ascii_digit());
        if !is_total && !is_core {
            continue;
        }
        let v: Vec<u64> = it.map(|p| p.parse().unwrap_or(0)).collect();
        if v.len() < 4 {
            continue;
        }
        let get = |i: usize| v.get(i).copied().unwrap_or(0);
        // user nice system idle iowait irq softirq steal (guest is already inside user)
        let busy = get(0) + get(1) + get(2) + get(5) + get(6) + get(7);
        let total = busy + get(3) + get(4);
        if is_total {
            sample.busy = busy;
            sample.total = total;
        } else {
            sample.cores.push((busy, total));
        }
    }
    sample
}

fn load(prev_busy: u64, prev_total: u64, busy: u64, total: u64) -> f64 {
    let dt = total.saturating_sub(prev_total);
    if dt == 0 {
        0.0
    } else {
        (busy.saturating_sub(prev_busy) as f64 / dt as f64).clamp(0.0, 1.0)
    }
}

/// Load of the whole CPU between two samples, 0...1 (`readCPU`: `min(max(busy / total, 0), 1)`, 0 when no time passed).
pub fn cpu_usage(prev: &CpuSample, curr: &CpuSample) -> f64 {
    load(prev.busy, prev.total, curr.busy, curr.total)
}

/// Load of each core between two samples (`readCores`); empty when the number of cores changed.
pub fn core_usages(prev: &CpuSample, curr: &CpuSample) -> Vec<f64> {
    if prev.cores.len() != curr.cores.len() {
        return vec![];
    }
    prev.cores.iter().zip(&curr.cores).map(|(&(pb, pt), &(cb, ct))| load(pb, pt, cb, ct)).collect()
}

// ---------------------------------------------------------------------------
// Memory (SystemMonitor.readMemory)
// ---------------------------------------------------------------------------

/// Parse `/proc/meminfo`. Used = `MemTotal - MemAvailable`. The breakdown follows the Mac card:
/// Apps = `AnonPages`, Compressed = `Zswap` (the compressed pool, 0 without zswap) and System = the
/// rest of the used memory, so the three always add up to the used memory like on the Mac.
pub fn parse_proc_meminfo(content: &str) -> MemInfo {
    let mut kb: HashMap<&str, u64> = HashMap::new();
    for line in content.lines() {
        if let Some((key, rest)) = line.split_once(':') {
            let n = rest.split_whitespace().next().and_then(|s| s.parse().ok()).unwrap_or(0);
            kb.insert(key.trim(), n);
        }
    }
    let g = |k: &str| kb.get(k).copied().unwrap_or(0);
    let total = g("MemTotal");
    let available = match kb.get("MemAvailable") {
        Some(a) => *a,
        None => g("MemFree") + g("Buffers") + g("Cached"),
    }
    .min(total);
    let used = total - available;
    let app = g("AnonPages").min(used);
    let compressed = g("Zswap").min(used - app);
    let wired = used - app - compressed;
    let fraction = if total == 0 { 0.0 } else { used as f64 / total as f64 };
    let avail_fraction = if total == 0 { 1.0 } else { available as f64 / total as f64 };
    let pressure = if avail_fraction < 0.10 {
        MemoryPressure::Critical
    } else if avail_fraction < 0.25 {
        MemoryPressure::Warning
    } else {
        MemoryPressure::Normal
    };
    let swap_total = g("SwapTotal");
    let swap_used = swap_total.saturating_sub(g("SwapFree"));
    MemInfo {
        total: total * 1024,
        available: available * 1024,
        used: used * 1024,
        fraction,
        breakdown: MemoryBreakdown { app: app * 1024, wired: wired * 1024, compressed: compressed * 1024 },
        swap_total: swap_total * 1024,
        swap_used: swap_used * 1024,
        pressure,
    }
}

// ---------------------------------------------------------------------------
// Disk and network (SystemMonitor.readDisk, readNetwork)
// ---------------------------------------------------------------------------

/// Used = total - free, fraction = `1 - free / total` (`diskFraction`).
pub fn disk_info_from_bytes(total: u64, free: u64) -> DiskInfo {
    let free = free.min(total);
    let fraction = if total == 0 { 0.0 } else { 1.0 - free as f64 / total as f64 };
    DiskInfo { total, free, used: total - free, fraction }
}

/// Parse `/proc/net/dev`: the sum of every interface except loopback (`!name.hasPrefix("lo")`).
pub fn parse_proc_net_dev(content: &str) -> NetSample {
    let mut sample = NetSample::default();
    for line in content.lines() {
        let Some((iface, data)) = line.split_once(':') else { continue };
        if iface.trim().starts_with("lo") {
            continue;
        }
        let v: Vec<&str> = data.split_whitespace().collect();
        if v.len() >= 9 {
            sample.rx_bytes += v[0].parse::<u64>().unwrap_or(0);
            sample.tx_bytes += v[8].parse::<u64>().unwrap_or(0);
        }
    }
    sample
}

/// Bytes per second between two samples; a counter that went back gives 0 (`rx >= p.rx ? ... : 0`).
pub fn net_rates(prev: &NetSample, curr: &NetSample, dt_secs: f64) -> (f64, f64) {
    if dt_secs <= 0.0 {
        return (0.0, 0.0);
    }
    let rate = |p: u64, c: u64| if c >= p { (c - p) as f64 / dt_secs } else { 0.0 };
    (rate(prev.rx_bytes, curr.rx_bytes), rate(prev.tx_bytes, curr.tx_bytes))
}

// ---------------------------------------------------------------------------
// Battery (SystemMonitor.readBattery)
// ---------------------------------------------------------------------------

/// What `/sys/class/power_supply/BAT*` says, converted to watts and watt-hours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BatteryReading<'a> {
    pub capacity: u8,
    pub status: &'a str,
    pub power_w: f64,
    pub energy_now_wh: f64,
    pub energy_full_wh: f64,
}

/// Percent, charging, on AC and minutes left. Mac: `minutes = charging ? timeToFull : (onAC ? -1 : timeToEmpty)`,
/// shown only when > 0. Linux computes the times from the energy and the power drawn.
pub fn parse_battery(r: &BatteryReading) -> BatteryInfo {
    let charging = r.status.eq_ignore_ascii_case("Charging");
    let on_ac = charging || r.status.eq_ignore_ascii_case("Full") || r.status.eq_ignore_ascii_case("Not charging");
    let minutes = if r.power_w <= 0.05 {
        0.0
    } else if charging {
        (r.energy_full_wh - r.energy_now_wh).max(0.0) * 60.0 / r.power_w
    } else if on_ac {
        0.0
    } else {
        r.energy_now_wh * 60.0 / r.power_w
    };
    let minutes = minutes.round() as u32;
    BatteryInfo {
        percent: r.capacity.min(100),
        charging,
        on_ac,
        minutes_left: (minutes > 0).then_some(minutes),
    }
}

// ---------------------------------------------------------------------------
// Processes (SystemMonitor.readTopProcesses)
// ---------------------------------------------------------------------------

/// Parse one `/proc/<pid>/stat` line. The name sits between the first `(` and the last `)` and may
/// hold spaces and parentheses. `page_kb` is the size of a page in KiB. `uid` comes from elsewhere.
pub fn parse_pid_stat(line: &str, page_kb: u64, uid: u32) -> Option<ProcStat> {
    let open = line.find('(')?;
    let close = line.rfind(')')?;
    if close < open {
        return None;
    }
    let pid: u32 = line[..open].trim().parse().ok()?;
    let name = line[open + 1..close].to_string();
    // after the name: state ppid ... utime(14) stime(15) ... rss(24); this slice starts at field 3
    let f: Vec<&str> = line[close + 1..].split_whitespace().collect();
    let utime: u64 = f.get(11)?.parse().ok()?;
    let stime: u64 = f.get(12)?.parse().ok()?;
    let rss_pages: i64 = f.get(21)?.parse().ok()?;
    Some(ProcStat { pid, name, ticks: utime + stime, rss_kb: rss_pages.max(0) as u64 * page_kb, uid })
}

/// Whether a process may be listed (and quit): only processes of the current user, never PID 1,
/// never Menew itself. The root and system processes and those of other users are never offered
/// (the Mac: "system processes are never suggested").
pub fn is_offered(uid: u32, pid: u32, my_uid: u32, my_pid: u32) -> bool {
    uid == my_uid && pid > 1 && pid != my_pid
}

/// What `top_processes` needs to know besides the two samples.
#[derive(Debug, Clone, Copy)]
pub struct TopQuery {
    /// Growth of the total ticks of the whole CPU between the two samples.
    pub total_delta: u64,
    pub ncpu: usize,
    pub my_uid: u32,
    pub my_pid: u32,
    pub by_mem: bool,
    pub limit: usize,
}

/// Choose the top processes. `prev` maps pid to the ticks of the previous sample, `total_delta` is the
/// growth of the whole CPU's total ticks between the two samples and `ncpu` the number of cores:
/// cpu% = `100 * ncpu * proc_delta / total_delta` (100 = one core busy). A process that is new has
/// no previous sample and counts 0. Order: by CPU then memory, or by memory then CPU.
pub fn top_processes(prev: &HashMap<u32, u64>, curr: &[ProcStat], q: &TopQuery) -> Vec<ProcInfo> {
    let TopQuery { total_delta, ncpu, my_uid, my_pid, by_mem, limit } = *q;
    let mut list: Vec<ProcInfo> = curr
        .iter()
        .filter(|p| is_offered(p.uid, p.pid, my_uid, my_pid))
        .map(|p| {
            let cpu = match prev.get(&p.pid) {
                Some(&before) if total_delta > 0 => {
                    100.0 * ncpu.max(1) as f64 * p.ticks.saturating_sub(before) as f64 / total_delta as f64
                }
                _ => 0.0,
            };
            ProcInfo { pid: p.pid, name: p.name.clone(), cpu, mem_kb: p.rss_kb }
        })
        .collect();
    let by_cpu = |a: &ProcInfo, b: &ProcInfo| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal);
    if by_mem {
        list.sort_by(|a, b| b.mem_kb.cmp(&a.mem_kb).then(by_cpu(a, b)).then(a.pid.cmp(&b.pid)));
    } else {
        list.sort_by(|a, b| by_cpu(a, b).then(b.mem_kb.cmp(&a.mem_kb)).then(a.pid.cmp(&b.pid)));
    }
    list.truncate(limit);
    list
}

// ---------------------------------------------------------------------------
// Formatting (SystemMonitor.swift: Fmt)
// ---------------------------------------------------------------------------

fn trim_num(val: f64, lang: Language) -> String {
    let s = if (val - val.round()).abs() < 0.05 { format!("{:.0}", val) } else { format!("{:.1}", val) };
    if decimal_sep(lang) == '.' { s } else { s.replace('.', ",") }
}

pub struct Fmt;

impl Fmt {
    /// `Fmt.bytes`: decimal units (the ByteCountFormatter file style), "48 KB", "1.2 MB".
    pub fn bytes(b: i64, lang: Language) -> String {
        let n = b.unsigned_abs() as f64;
        let sign = if b < 0 { "-" } else { "" };
        let (v, unit) = if n < 1e3 {
            return format!("{b} B");
        } else if n < 1e6 {
            (n / 1e3, "KB")
        } else if n < 1e9 {
            (n / 1e6, "MB")
        } else if n < 1e12 {
            (n / 1e9, "GB")
        } else {
            (n / 1e12, "TB")
        };
        format!("{sign}{} {unit}", trim_num(v, lang))
    }

    /// `Fmt.mem`: binary units (the ByteCountFormatter memory style).
    pub fn mem(b: u64, lang: Language) -> String {
        let n = b as f64;
        let (v, unit) = if b < 1024 {
            return format!("{b} B");
        } else if n < 1048576.0 {
            (n / 1024.0, "KB")
        } else if n < 1073741824.0 {
            (n / 1048576.0, "MB")
        } else {
            (n / 1073741824.0, "GB")
        };
        format!("{} {unit}", trim_num(v, lang))
    }

    /// `Fmt.memShort`: "12,3 GB" / "16 GB".
    pub fn mem_short(b: u64, lang: Language) -> String {
        let gb = b as f64 / 1_073_741_824.0;
        let s = if gb >= 10.0 || gb.round() == gb {
            format!("{:.0}", gb)
        } else {
            let v = format!("{:.1}", gb);
            if decimal_sep(lang) == '.' { v } else { v.replace('.', ",") }
        };
        format!("{s} GB")
    }

    /// `Fmt.rate`: bytes per second as "1.2 MB/s".
    pub fn rate(bps: f64, lang: Language) -> String {
        format!("{}/s", Self::bytes(bps as i64, lang))
    }

    /// `Fmt.shortRate` for the panel: "350B", "48K", "1.2M". Not localized, like the Mac.
    pub fn short_rate(bps: f64) -> String {
        if bps < 1_000.0 {
            format!("{:.0}B", bps)
        } else if bps < 1_000_000.0 {
            format!("{:.0}K", bps / 1_000.0)
        } else {
            format!("{:.1}M", bps / 1_000_000.0)
        }
    }

    /// `Fmt.pct`: "23%".
    pub fn pct(f: f64) -> String {
        format!("{:.0}%", f * 100.0)
    }

    /// `Fmt.duration`: "1d 1h 0min" or "2h 5min" (hours are always shown, "0h 2min").
    pub fn duration(secs: u64) -> String {
        let (d, h, m) = (secs / 86_400, (secs % 86_400) / 3_600, (secs % 3_600) / 60);
        if d > 0 { format!("{d}d {h}h {m}min") } else { format!("{h}h {m}min") }
    }

    /// Time left of the battery card: "2 h 5 min" from 60 minutes, else "45 min".
    pub fn battery_minutes(mins: u32) -> String {
        if mins >= 60 { format!("{} h {} min", mins / 60, mins % 60) } else { format!("{mins} min") }
    }
}

// ---------------------------------------------------------------------------
// Tray metrics (Views.swift: MenuBarItem)
// ---------------------------------------------------------------------------

/// The items of `MenuBarItem` that exist on Linux, in the Mac order. `temp` is not here (R2).
pub const TRAY_ITEMS: [&str; 6] = ["cpu", "cpuGraph", "ram", "net", "disk", "battery"];

/// The values the tray items are made of; `None` when the machine does not report it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrayValues {
    pub cpu: Option<f64>,
    /// The last values of the CPU history (the Mac draws the last 12).
    pub cpu_history: Vec<f64>,
    pub mem_fraction: Option<f64>,
    pub net: Option<(f64, f64)>,
    pub disk_fraction: Option<f64>,
    pub battery: Option<BatteryInfo>,
}

fn spark(values: &[f64]) -> String {
    const BARS: [char; 8] = ['\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}', '\u{2588}'];
    let start = values.len().saturating_sub(12);
    values[start..].iter().map(|v| BARS[((v.clamp(0.0, 1.0) * 7.0).round() as usize).min(7)]).collect()
}

/// Text of the chosen items for the StatusNotifierItem: the `Title` text (what KDE shows in the panel)
/// and one localized line per item for the `ToolTip`. The texts of the title are the Mac ones
/// (`MenuBarItem.segment`): "CPU 23%", "RAM 64%", "SSD 58%", "BAT 80%", and the network as the two
/// short rates with an arrow each. An item the machine does not report is left out.
pub fn format_tray_metrics(items: &[String], v: &TrayValues, lang: Language) -> (String, Vec<String>) {
    let mut title = Vec::new();
    let mut lines = Vec::new();
    for key in TRAY_ITEMS {
        if !items.iter().any(|i| i == key) {
            continue;
        }
        match key {
            "cpu" => {
                if let Some(c) = v.cpu {
                    title.push(format!("CPU {}", Fmt::pct(c)));
                    lines.push(format!("{}: {}", tr("mon_cpu", lang, &[]), Fmt::pct(c)));
                }
            }
            "cpuGraph" => {
                if !v.cpu_history.is_empty() {
                    title.push(spark(&v.cpu_history));
                    lines.push(format!("{}: {}", tr("mon_cpu_graph", lang, &[]), spark(&v.cpu_history)));
                }
            }
            "ram" => {
                if let Some(m) = v.mem_fraction {
                    title.push(format!("RAM {}", Fmt::pct(m)));
                    lines.push(format!("{}: {}", tr("mon_memory", lang, &[]), Fmt::pct(m)));
                }
            }
            "net" => {
                if let Some((rx, tx)) = v.net {
                    title.push(format!("\u{2193}{} \u{2191}{}", Fmt::short_rate(rx), Fmt::short_rate(tx)));
                    lines.push(format!(
                        "{}: \u{2193} {}  \u{2191} {}",
                        tr("mon_network", lang, &[]),
                        Fmt::rate(rx, lang),
                        Fmt::rate(tx, lang)
                    ));
                }
            }
            "disk" => {
                if let Some(d) = v.disk_fraction {
                    title.push(format!("SSD {}", Fmt::pct(d)));
                    lines.push(format!("{}: {}", tr("mon_disk", lang, &[]), Fmt::pct(d)));
                }
            }
            "battery" => {
                if let Some(b) = &v.battery {
                    title.push(format!("BAT {}%", b.percent));
                    lines.push(format!("{}: {}% ({})", tr("mon_battery", lang, &[]), b.percent, battery_state(b, lang)));
                }
            }
            _ => {}
        }
    }
    (title.join(" \u{b7} "), lines)
}

/// "Charging", "Full in 1 h 5 min", "On battery, remaining 2 h 5 min"... (`batteryText` of the Mac menu).
pub fn battery_state(b: &BatteryInfo, lang: Language) -> String {
    let t = b.minutes_left.map(Fmt::battery_minutes);
    if b.charging {
        match t {
            Some(t) => tr("mon_full_in", lang, &[&t]),
            None => tr("mon_charging", lang, &[]),
        }
    } else if b.on_ac {
        tr("mon_plugged", lang, &[])
    } else {
        match t {
            Some(t) => format!("{}, {} {}", tr("mon_on_battery", lang, &[]), tr("mon_remaining", lang, &[]).to_lowercase(), t),
            None => tr("mon_on_battery", lang, &[]),
        }
    }
}

// ---------------------------------------------------------------------------
// One reading of everything, and the sampler that produces it
// ---------------------------------------------------------------------------

/// Everything the monitor view and the tray show at one moment.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub cpu: f64,
    pub cpu_history: Vec<f64>,
    pub cores: Vec<f64>,
    /// `None` = the machine does not report the GPU load (never a made-up number).
    pub gpu: Option<f64>,
    pub mem: Option<MemInfo>,
    pub disk: Option<DiskInfo>,
    pub net_in: f64,
    pub net_out: f64,
    pub net_in_history: Vec<f64>,
    pub net_out_history: Vec<f64>,
    pub battery: Option<BatteryInfo>,
    pub top_cpu: Vec<ProcInfo>,
    pub top_mem: Vec<ProcInfo>,
}

impl Snapshot {
    pub fn tray_values(&self) -> TrayValues {
        TrayValues {
            cpu: Some(self.cpu),
            cpu_history: self.cpu_history.clone(),
            mem_fraction: self.mem.as_ref().map(|m| m.fraction),
            net: Some((self.net_in, self.net_out)),
            disk_fraction: self.disk.as_ref().map(|d| d.fraction),
            battery: self.battery.clone(),
        }
    }

    /// Made-up numbers for the screenshots (`menew --snapshot`); the machine is never read.
    pub fn demo() -> Snapshot {
        const GB: u64 = 1_073_741_824;
        let wave = |n: usize, base: f64, amp: f64| -> Vec<f64> {
            (0..n).map(|i| (base + amp * ((i as f64) * 0.55).sin() * ((i as f64) * 0.13).cos()).clamp(0.02, 1.0)).collect()
        };
        let p = |pid, name: &str, cpu, mb: u64| ProcInfo { pid, name: name.into(), cpu, mem_kb: mb * 1024 };
        let mut cpu_history = wave(CPU_HISTORY - 1, 0.28, 0.2);
        cpu_history.push(0.23);
        Snapshot {
            cpu: 0.23,
            cpu_history,
            cores: vec![0.41, 0.18, 0.33, 0.12, 0.27, 0.09, 0.22, 0.15],
            gpu: Some(0.12),
            mem: Some(MemInfo {
                total: 16 * GB,
                available: 6 * GB,
                used: 10 * GB,
                fraction: 0.64,
                breakdown: MemoryBreakdown { app: 6 * GB + GB / 2, wired: 3 * GB, compressed: GB / 2 },
                swap_total: 4 * GB,
                swap_used: GB / 2,
                pressure: MemoryPressure::Normal,
            }),
            disk: Some(DiskInfo { total: 512_000_000_000, free: 215_000_000_000, used: 297_000_000_000, fraction: 0.58 }),
            net_in: 1_200_000.0,
            net_out: 48_000.0,
            net_in_history: wave(NET_HISTORY, 0.4, 0.3),
            net_out_history: wave(NET_HISTORY, 0.15, 0.1),
            battery: Some(BatteryInfo { percent: 80, charging: false, on_ac: false, minutes_left: Some(200) }),
            top_cpu: vec![
                p(1420, "Web Browser", 18.2, 512),
                p(1580, "Text Editor", 8.5, 307),
                p(1890, "Terminal", 3.1, 32),
                p(2100, "Music", 1.2, 48),
                p(2310, "Files", 0.8, 24),
            ],
            top_mem: vec![
                p(1420, "Web Browser", 18.2, 512),
                p(1580, "Text Editor", 8.5, 307),
                p(2100, "Music", 1.2, 48),
                p(1890, "Terminal", 3.1, 32),
                p(2310, "Files", 0.8, 24),
            ],
        }
    }
}

fn push_capped(v: &mut Vec<f64>, x: f64, cap: usize) {
    if v.len() >= cap {
        v.remove(0);
    }
    v.push(x);
}

/// Reads the machine twice and works out the difference. Created only while someone looks (the
/// view is open or the tray shows metrics); `with_processes` adds the scan of `/proc/<pid>`.
pub struct Sampler {
    with_processes: bool,
    prev_cpu: Option<CpuSample>,
    prev_net: Option<(NetSample, std::time::Instant)>,
    prev_procs: HashMap<u32, u64>,
    snap: Snapshot,
}

impl Sampler {
    pub fn new(with_processes: bool) -> Self {
        Sampler {
            with_processes,
            prev_cpu: None,
            prev_net: None,
            prev_procs: HashMap::new(),
            snap: Snapshot {
                cpu_history: vec![0.0; CPU_HISTORY],
                net_in_history: vec![0.0; NET_HISTORY],
                net_out_history: vec![0.0; NET_HISTORY],
                ..Default::default()
            },
        }
    }

    /// Take a sample and return the state. The first call has nothing to compare with: rates are 0.
    pub fn tick(&mut self) -> &Snapshot {
        let mut total_delta = 0;
        let mut ncpu = 1;
        if let Ok(cur) = sys::read_cpu_sample() {
            if let Some(prev) = &self.prev_cpu {
                self.snap.cpu = cpu_usage(prev, &cur);
                self.snap.cores = core_usages(prev, &cur);
                push_capped(&mut self.snap.cpu_history, self.snap.cpu, CPU_HISTORY);
                total_delta = cur.total.saturating_sub(prev.total);
            }
            ncpu = cur.cores.len().max(1);
            self.prev_cpu = Some(cur);
        }
        let now = std::time::Instant::now();
        if let Ok(cur) = sys::read_net_sample() {
            if let Some((prev, at)) = &self.prev_net {
                let (rx, tx) = net_rates(prev, &cur, now.duration_since(*at).as_secs_f64());
                self.snap.net_in = rx;
                self.snap.net_out = tx;
                push_capped(&mut self.snap.net_in_history, rx, NET_HISTORY);
                push_capped(&mut self.snap.net_out_history, tx, NET_HISTORY);
            }
            self.prev_net = Some((cur, now));
        }
        self.snap.mem = sys::read_mem_info().ok();
        self.snap.disk = sys::read_disk_info("/").ok();
        self.snap.battery = sys::read_battery_info();
        self.snap.gpu = sys::read_gpu_usage();
        if self.with_processes {
            let (my_uid, my_pid) = (sys::current_uid(), std::process::id());
            let procs = sys::read_processes();
            let q = TopQuery { total_delta, ncpu, my_uid, my_pid, by_mem: false, limit: TOP_COUNT };
            self.snap.top_cpu = top_processes(&self.prev_procs, &procs, &q);
            self.snap.top_mem = top_processes(&self.prev_procs, &procs, &TopQuery { by_mem: true, ..q });
            self.prev_procs = procs.iter().map(|p| (p.pid, p.ticks)).collect();
        }
        &self.snap
    }
}

// ---------------------------------------------------------------------------
// Linux readers
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
pub mod sys {
    use super::*;
    use std::fs;
    use std::path::Path;

    pub fn current_uid() -> u32 {
        unsafe { libc::getuid() }
    }

    pub fn read_cpu_sample() -> std::io::Result<CpuSample> {
        Ok(parse_proc_stat(&fs::read_to_string("/proc/stat")?))
    }

    pub fn read_mem_info() -> std::io::Result<MemInfo> {
        Ok(parse_proc_meminfo(&fs::read_to_string("/proc/meminfo")?))
    }

    pub fn read_net_sample() -> std::io::Result<NetSample> {
        Ok(parse_proc_net_dev(&fs::read_to_string("/proc/net/dev")?))
    }

    /// `statvfs`: the space an ordinary user can use (`f_bavail`) is the free space.
    pub fn read_disk_info(mount_point: &str) -> std::io::Result<DiskInfo> {
        let path = std::ffi::CString::new(mount_point).map_err(std::io::Error::other)?;
        let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(path.as_ptr(), &mut st) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        let frsize = st.f_frsize as u64;
        Ok(disk_info_from_bytes(st.f_blocks as u64 * frsize, st.f_bavail as u64 * frsize))
    }

    fn num(path: &Path) -> Option<f64> {
        fs::read_to_string(path).ok()?.trim().parse().ok()
    }

    /// The first battery of `/sys/class/power_supply`, `None` on a desktop. Batteries report either
    /// energy (uWh, uW) or charge (uAh, uA) with the voltage (uV); both become Wh and W.
    pub fn read_battery_info() -> Option<BatteryInfo> {
        let mut dirs: Vec<_> = fs::read_dir("/sys/class/power_supply").ok()?.flatten().map(|e| e.path()).collect();
        dirs.sort();
        for dir in dirs {
            let is_battery = fs::read_to_string(dir.join("type")).map(|t| t.trim() == "Battery").unwrap_or(false);
            if !is_battery {
                continue;
            }
            let status = fs::read_to_string(dir.join("status")).unwrap_or_default();
            let volts = num(&dir.join("voltage_now")).unwrap_or(0.0) / 1e6;
            let (now, full, power) = if let Some(e) = num(&dir.join("energy_now")) {
                (e / 1e6, num(&dir.join("energy_full")).unwrap_or(0.0) / 1e6, num(&dir.join("power_now")).unwrap_or(0.0) / 1e6)
            } else {
                let c = |n: &str| num(&dir.join(n)).unwrap_or(0.0) / 1e6;
                (c("charge_now") * volts, c("charge_full") * volts, c("current_now").abs() * volts)
            };
            let capacity = num(&dir.join("capacity")).map(|c| c as u8).or_else(|| (full > 0.0).then(|| (now * 100.0 / full) as u8))?;
            return Some(parse_battery(&BatteryReading {
                capacity,
                status: status.trim(),
                power_w: power,
                energy_now_wh: now,
                energy_full_wh: full,
            }));
        }
        None
    }

    /// GPU load from the amdgpu driver (`gpu_busy_percent`), the largest of the cards like the Mac.
    /// Other drivers do not report a load in sysfs: `None`, never an estimate.
    pub fn read_gpu_usage() -> Option<f64> {
        let mut best: Option<f64> = None;
        for card in fs::read_dir("/sys/class/drm").ok()?.flatten() {
            let name = card.file_name().to_string_lossy().into_owned();
            if !name.starts_with("card") || name[4..].chars().any(|c| !c.is_ascii_digit()) {
                continue;
            }
            if let Some(p) = num(&card.path().join("device/gpu_busy_percent")) {
                best = Some(best.unwrap_or(0.0).max((p / 100.0).clamp(0.0, 1.0)));
            }
        }
        best
    }

    /// Every process: `/proc/<pid>/stat` and the owner (the owner of the `/proc/<pid>` directory).
    pub fn read_processes() -> Vec<ProcStat> {
        use std::os::unix::fs::MetadataExt;
        let page_kb = (unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(1024) as u64) / 1024;
        let mut out = Vec::new();
        let Ok(dir) = fs::read_dir("/proc") else { return out };
        for e in dir.flatten() {
            let name = e.file_name();
            let Some(pid) = name.to_str().filter(|n| n.bytes().all(|b| b.is_ascii_digit())) else { continue };
            let Ok(meta) = e.metadata() else { continue };
            let Ok(stat) = fs::read_to_string(e.path().join("stat")) else { continue };
            if let Some(p) = parse_pid_stat(&stat, page_kb, meta.uid()) {
                debug_assert_eq!(p.pid.to_string(), pid);
                out.push(p);
            }
        }
        out
    }

    /// Quit a process: SIGTERM, or SIGKILL when `force` (the explicit second step). Refuses PID 1,
    /// Menew itself and every process that is not the user's own.
    pub fn quit_process(pid: u32, force: bool) -> Result<(), String> {
        use std::os::unix::fs::MetadataExt;
        let meta = fs::metadata(format!("/proc/{pid}")).map_err(|_| format!("process {pid} is not running"))?;
        if !is_offered(meta.uid(), pid, current_uid(), std::process::id()) {
            return Err(format!("process {pid} is not one of your own processes: refused"));
        }
        let sig = if force { libc::SIGKILL } else { libc::SIGTERM };
        if unsafe { libc::kill(pid as libc::pid_t, sig) } == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error().to_string())
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub mod sys {
    use super::*;

    fn unsupported<T>() -> std::io::Result<T> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Linux only"))
    }
    pub fn current_uid() -> u32 {
        0
    }
    pub fn read_cpu_sample() -> std::io::Result<CpuSample> {
        unsupported()
    }
    pub fn read_mem_info() -> std::io::Result<MemInfo> {
        unsupported()
    }
    pub fn read_net_sample() -> std::io::Result<NetSample> {
        unsupported()
    }
    pub fn read_disk_info(_mount_point: &str) -> std::io::Result<DiskInfo> {
        unsupported()
    }
    pub fn read_battery_info() -> Option<BatteryInfo> {
        None
    }
    pub fn read_gpu_usage() -> Option<f64> {
        None
    }
    pub fn read_processes() -> Vec<ProcStat> {
        vec![]
    }
    pub fn quit_process(_pid: u32, _force: bool) -> Result<(), String> {
        Err("Linux only".into())
    }
}
