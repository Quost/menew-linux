//! Runs every vector of `docs/monitor-spec.md` through the real monitor code, and checks the real
//! `/proc` captures of `tests/fixtures/` against numbers worked out with `awk`.
//! The expected values come from the Mac code (`../menew/Sources/Menew`), not from this code's output.

use menew_core::monitor::*;
use menew_core::Language;
use std::collections::HashMap;

fn cells(line: &str) -> Vec<String> {
    let inner = line.trim_start().strip_prefix('|').unwrap().trim_end().strip_suffix('|').unwrap();
    inner.split('|').map(|c| c.trim().to_string()).collect()
}

/// The rows of the table whose first cell is `<letter><digits>`.
fn rows(letter: char, columns: usize) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/monitor-spec.md")).unwrap();
    let rows: Vec<Vec<String>> = text
        .lines()
        .filter(|l| {
            l.strip_prefix("| ").map(|r| r.starts_with(letter) && r[1..].chars().take_while(|c| *c != ' ').all(|c| c.is_ascii_digit()) && r.len() > 2).unwrap_or(false)
        })
        .map(cells)
        .collect();
    assert!(!rows.is_empty(), "no {letter} vectors in docs/monitor-spec.md");
    for r in &rows {
        assert_eq!(r.len(), columns, "bad row ({columns} columns expected): {r:?}");
    }
    rows
}

fn none(s: &str) -> Option<&str> {
    (s != "-").then_some(s)
}

fn finish(name: &str, rows: usize, executed: usize, failures: Vec<String>) {
    println!("monitor-spec {name}: executed {executed} vectors of {rows} in docs/monitor-spec.md");
    assert_eq!(executed, rows, "{name}: not every vector was executed");
    assert!(failures.is_empty(), "{} vector(s) failed:\n{}", failures.len(), failures.join("\n"));
}

/// Run `check` on each row; it returns the mismatches of that row.
fn run(name: &str, letter: char, columns: usize, check: impl Fn(&[String]) -> Vec<String>) {
    let rows = rows(letter, columns);
    let mut failures = vec![];
    let mut executed = 0;
    for r in &rows {
        for f in check(r) {
            failures.push(format!("{}: {f}", r[0]));
        }
        executed += 1;
    }
    finish(name, rows.len(), executed, failures);
}

fn cmp<T: PartialEq + std::fmt::Debug>(what: &str, expected: T, got: T) -> Vec<String> {
    if expected == got { vec![] } else { vec![format!("{what}: expected {expected:?}, got {got:?}")] }
}

fn sample(busy: &str, total: &str) -> CpuSample {
    CpuSample { busy: busy.parse().unwrap(), total: total.parse().unwrap(), cores: vec![] }
}

#[test]
fn a_cpu_load_between_samples() {
    run("A", 'A', 6, |r| {
        let u = cpu_usage(&sample(&r[1], &r[2]), &sample(&r[3], &r[4]));
        cmp("load", r[5].clone(), Fmt::pct(u))
    });
}

#[test]
fn b_proc_stat_line() {
    run("B", 'B', 4, |r| {
        let s = parse_proc_stat(&format!("cpu  {}\n", r[1]));
        let mut f = cmp("busy", r[2].parse::<u64>().unwrap(), s.busy);
        f.extend(cmp("total", r[3].parse::<u64>().unwrap(), s.total));
        f
    });
}

#[test]
fn c_memory() {
    run("C", 'C', 13, |r| {
        let n = |i: usize| r[i].parse::<u64>().unwrap();
        let text = format!(
            "MemTotal: {} kB\nMemFree: 1 kB\nMemAvailable: {} kB\nAnonPages: {} kB\nZswap: {} kB\nSwapTotal: {} kB\nSwapFree: {} kB\n",
            n(1), n(2), n(3), n(4), n(5), n(6)
        );
        let m = parse_proc_meminfo(&text);
        let kb = |b: u64| b / 1024;
        let mut f = cmp("fraction", r[7].clone(), Fmt::pct(m.fraction));
        f.extend(cmp("pressure", r[8].as_str(), m.pressure.as_str()));
        f.extend(cmp("app", n(9), kb(m.breakdown.app)));
        f.extend(cmp("system", n(10), kb(m.breakdown.wired)));
        f.extend(cmp("compressed", n(11), kb(m.breakdown.compressed)));
        f.extend(cmp("swap used", n(12), kb(m.swap_used)));
        // the three parts always add up to the used memory
        f.extend(cmp("parts add up", m.used, m.breakdown.app + m.breakdown.wired + m.breakdown.compressed));
        f
    });
}

#[test]
fn d_disk() {
    run("D", 'D', 5, |r| {
        let d = disk_info_from_bytes(r[1].parse().unwrap(), r[2].parse().unwrap());
        let mut f = cmp("used", r[3].parse::<u64>().unwrap(), d.used);
        f.extend(cmp("fraction", r[4].clone(), Fmt::pct(d.fraction)));
        f
    });
}

#[test]
fn e_network_rate() {
    run("E", 'E', 7, |r| {
        let net = |v: &str| NetSample { rx_bytes: v.parse().unwrap(), tx_bytes: 0 };
        let (rx, tx) = net_rates(&net(&r[1]), &net(&r[2]), r[3].parse().unwrap());
        let mut f = cmp("bytes per second", r[4].parse::<f64>().unwrap(), rx);
        f.extend(cmp("tx", 0.0, tx));
        f.extend(cmp("short rate", r[5].clone(), Fmt::short_rate(rx)));
        f.extend(cmp("rate", r[6].clone(), Fmt::rate(rx, Language::En)));
        f
    });
}

#[test]
fn f_battery() {
    run("F", 'F', 10, |r| {
        let b = parse_battery(&BatteryReading {
            capacity: r[1].parse().unwrap(),
            status: &r[2],
            power_w: r[3].parse().unwrap(),
            energy_now_wh: r[4].parse().unwrap(),
            energy_full_wh: r[5].parse().unwrap(),
        });
        let mut f = cmp("percent", r[6].parse::<u8>().unwrap(), b.percent);
        f.extend(cmp("charging", r[7].parse::<bool>().unwrap(), b.charging));
        f.extend(cmp("on ac", r[8].parse::<bool>().unwrap(), b.on_ac));
        f.extend(cmp("minutes", none(&r[9]).map(|m| m.parse::<u32>().unwrap()), b.minutes_left));
        f
    });
}

#[test]
fn g_pid_stat_line() {
    run("G", 'G', 6, |r| match parse_pid_stat(&r[2], 4, r[1].parse().unwrap()) {
        Some(p) => {
            let mut f = cmp("name", r[3].clone(), p.name);
            f.extend(cmp("ticks", r[4].parse::<u64>().unwrap(), p.ticks));
            f.extend(cmp("rss", r[5].parse::<u64>().unwrap(), p.rss_kb));
            f.extend(cmp("uid", r[1].parse::<u32>().unwrap(), p.uid));
            f
        }
        None => cmp("rejected", "-", r[3].as_str()),
    });
}

#[test]
fn h_which_processes_are_offered() {
    run("H", 'H', 6, |r| {
        let n = |i: usize| r[i].parse::<u32>().unwrap();
        cmp("offered", r[5].parse::<bool>().unwrap(), is_offered(n(1), n(2), n(3), n(4)))
    });
}

#[test]
fn i_top_processes() {
    run("I", 'I', 7, |r| {
        let mut prev = HashMap::new();
        let curr: Vec<ProcStat> = r[2]
            .split(';')
            .map(|p| {
                let f: Vec<&str> = p.split('/').collect();
                let pid: u32 = f[0].parse().unwrap();
                if f[3] != "-" {
                    prev.insert(pid, f[3].parse().unwrap());
                }
                ProcStat { pid, name: format!("p{pid}"), ticks: f[2].parse().unwrap(), rss_kb: f[4].parse().unwrap(), uid: f[1].parse().unwrap() }
            })
            .collect();
        let q = TopQuery { total_delta: r[3].parse().unwrap(), ncpu: r[4].parse().unwrap(), my_uid: 1000, my_pid: 99, by_mem: r[1] == "mem", limit: TOP_COUNT };
        let top = top_processes(&prev, &curr, &q);
        let pids: Vec<String> = top.iter().map(|p| p.pid.to_string()).collect();
        let mut f = cmp("order", r[5].clone(), pids.join(","));
        f.extend(cmp("first cpu", r[6].clone(), Fmt::pct(top.first().map(|p| p.cpu).unwrap_or(0.0) / 100.0)));
        f
    });
}

#[test]
fn j_formatting() {
    run("J", 'J', 5, |r| {
        let lang = Language::from_code(&r[1]);
        let got = match r[2].as_str() {
            "mem_short" => Fmt::mem_short(r[3].parse().unwrap(), lang),
            "short_rate" => Fmt::short_rate(r[3].parse().unwrap()),
            "pct" => Fmt::pct(r[3].parse().unwrap()),
            "duration" => Fmt::duration(r[3].parse().unwrap()),
            "bytes" => Fmt::bytes(r[3].parse().unwrap(), lang),
            "mem" => Fmt::mem(r[3].parse().unwrap(), lang),
            "battery_minutes" => Fmt::battery_minutes(r[3].parse().unwrap()),
            other => panic!("unknown function {other}"),
        };
        cmp("output", r[4].clone(), got)
    });
}

#[test]
fn k_tray_title_and_tooltip() {
    run("K", 'K', 10, |r| {
        let lang = Language::from_code(&r[1]);
        let items: Vec<String> = if r[2] == "-" { vec![] } else { r[2].split(',').map(String::from).collect() };
        let frac = |s: &str| none(s).map(|v| v.parse::<f64>().unwrap());
        let net = none(&r[5]).map(|v| {
            let (a, b) = v.split_once('/').unwrap();
            (a.parse().unwrap(), b.parse().unwrap())
        });
        let battery = none(&r[7]).map(|v| {
            let p: Vec<&str> = v.split('/').collect();
            let state = p[2];
            parse_battery(&BatteryReading {
                capacity: p[0].parse().unwrap(),
                status: match state {
                    "charging" => "Charging",
                    "plugged" => "Full",
                    _ => "Discharging",
                },
                power_w: 60.0,
                energy_now_wh: 0.0,
                energy_full_wh: 0.0,
            })
        });
        // the minutes are given in the vector: set them directly
        let battery = battery.map(|mut b| {
            let p: Vec<&str> = r[7].split('/').collect();
            b.minutes_left = none(p[1]).map(|m| m.parse().unwrap());
            b
        });
        let v = TrayValues { cpu: frac(&r[3]), cpu_history: vec![], mem_fraction: frac(&r[4]), net, disk_fraction: frac(&r[6]), battery };
        let (title, lines) = format_tray_metrics(&items, &v, lang);
        let empty = |s: &str| if s == "(empty)" { String::new() } else { s.replace("\\n", "\n") };
        let mut f = cmp("title", empty(&r[8]), title);
        f.extend(cmp("tooltip", empty(&r[9]), lines.join("\n")));
        f
    });
}

#[test]
fn l_tray_graph() {
    run("L", 'L', 3, |r| {
        let history: Vec<f64> = r[1].split(',').map(|v| v.parse().unwrap()).collect();
        let v = TrayValues { cpu_history: history, ..Default::default() };
        let (title, _) = format_tray_metrics(&["cpuGraph".to_string()], &v, Language::En);
        cmp("graph", r[2].clone(), title)
    });
}

// ---- real captures ----

#[test]
fn fixture_proc_stat() {
    let s1 = parse_proc_stat(include_str!("fixtures/proc_stat_1.txt"));
    let s2 = parse_proc_stat(include_str!("fixtures/proc_stat_2.txt"));
    // worked out with awk: busy 2040696 -> 2041331, total 44876913 -> 44878560
    assert_eq!((s1.busy, s1.total), (2040696, 44876913));
    assert_eq!((s2.busy, s2.total), (2041331, 44878560));
    assert_eq!(Fmt::pct(cpu_usage(&s1, &s2)), "39%");
    assert_eq!(s1.cores.len(), 8);
    let loads = core_usages(&s1, &s2);
    assert_eq!(loads.len(), 8);
    assert!(loads.iter().all(|l| (0.0..=1.0).contains(l)));
    // a different number of cores gives no per-core loads
    assert!(core_usages(&s1, &CpuSample::default()).is_empty());
}

#[test]
fn fixture_proc_net_dev() {
    let n1 = parse_proc_net_dev(include_str!("fixtures/proc_net_dev_1.txt"));
    let n2 = parse_proc_net_dev(include_str!("fixtures/proc_net_dev_2.txt"));
    // only eth0 counts, lo is ignored
    assert_eq!((n1.rx_bytes, n1.tx_bytes), (707691434, 3702608));
    let (rx, tx) = net_rates(&n1, &n2, 1.0);
    assert_eq!((rx, tx), (10438.0, 6011.0));
}

#[test]
fn fixture_proc_meminfo() {
    let m = parse_proc_meminfo(include_str!("fixtures/proc_meminfo.txt"));
    assert_eq!(m.total / 1024, 32868548);
    assert_eq!(m.used / 1024, 2991900);
    assert_eq!(Fmt::pct(m.fraction), "9%");
    assert_eq!(m.pressure, MemoryPressure::Normal);
    assert_eq!(m.breakdown.app / 1024, 2034132);
    assert_eq!(m.breakdown.wired / 1024, 957768);
    assert_eq!(m.breakdown.compressed, 0);
    assert_eq!(m.swap_used, 0);
}

#[test]
fn fixture_pid_stat_finds_the_busy_process_and_hides_root() {
    // "uid<TAB>stat line" of: a busy `yes` of the user (PID 466), systemd (PID 1) and journalctl (root), bash (user)
    let procs: Vec<ProcStat> = include_str!("fixtures/pid_stat_2.txt")
        .lines()
        .map(|l| {
            let (uid, stat) = l.split_once('\t').unwrap();
            parse_pid_stat(stat, 4, uid.parse().unwrap()).unwrap()
        })
        .collect();
    assert_eq!(procs.len(), 5);
    let yes = procs.iter().find(|p| p.name == "yes").unwrap();
    assert_eq!((yes.pid, yes.ticks, yes.rss_kb), (466, 98 + 116, 384 * 4));
    // the previous sample: `yes` had just started (0 ticks) when the stat capture 2 s before was taken
    let prev: HashMap<u32, u64> = procs.iter().map(|p| (p.pid, if p.name == "yes" { 0 } else { p.ticks })).collect();
    let (s1, s2) = (
        parse_proc_stat(include_str!("fixtures/proc_stat_1.txt")),
        parse_proc_stat(include_str!("fixtures/proc_stat_2.txt")),
    );
    let total_delta = s2.total - s1.total;
    // user 1000, Menew is some other PID
    let q = TopQuery { total_delta, ncpu: s2.cores.len(), my_uid: 1000, my_pid: 4242, by_mem: false, limit: TOP_COUNT };
    let top = top_processes(&prev, &procs, &q);
    let names: Vec<&str> = top.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names.first(), Some(&"yes"), "{names:?}");
    // 214 ticks in a 1647-tick interval of 8 cores = 100 * 8 * 214 / 1647
    assert!((top[0].cpu - 100.0 * 8.0 * 214.0 / 1647.0).abs() < 0.01, "{}", top[0].cpu);
    assert!(!names.contains(&"systemd") && !names.contains(&"journalctl"), "root processes offered: {names:?}");
    assert!(top.iter().all(|p| p.pid > 1));
}

#[test]
fn the_monitor_demo_never_reads_the_machine() {
    // the screenshots use this sample; it must be complete and made up
    let d = Snapshot::demo();
    assert_eq!(d.cpu_history.len(), CPU_HISTORY);
    assert_eq!(d.net_in_history.len(), NET_HISTORY);
    assert!(d.top_cpu.len() == TOP_COUNT && d.top_mem.len() == TOP_COUNT);
    assert!(d.top_cpu.iter().all(|p| p.pid >= 1000 && !p.name.is_empty()));
    let m = d.mem.unwrap();
    assert_eq!(m.used, m.breakdown.app + m.breakdown.wired + m.breakdown.compressed);
}
