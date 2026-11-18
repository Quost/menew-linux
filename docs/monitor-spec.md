# System monitor: specification and test vectors

The Mac app is the specification (`../menew/Sources/Menew`: `SystemMonitor.swift`, `MonitorViews.swift`,
`Views.swift` for the menu bar items, `Hardware.swift` for the GPU). This file lists what the Mac code does
for a given input, where in the Mac code that comes from, and where Linux has to differ because the data
comes from `/proc` and `/sys` instead of Mach and IOKit.

The tables in "Vectors" are **executed**: `crates/menew-core/tests/monitor_spec.rs` reads this file, runs every
row through the real Linux code and compares. The test also fails if it runs a different number of rows
than the file has. The expected values were worked out by hand from the Mac code (or from the Linux
fixtures with `awk`), not copied from the Linux output.

In a table cell `-` means "nothing / none". Cells never contain a pipe character.

## What the Mac does, and the Linux counterpart

| Part | Mac (file, function) | Linux (`crates/menew-core/src/monitor.rs`) |
|---|---|---|
| Sampling | `SystemMonitor.start`: a 2 s timer; GPU and per-core loads only while the menu is visible | `Sampler::tick` every `INTERVAL_SECS` = 2, created only while the window is open or the tray shows metrics |
| CPU load | `readCPU`: `busy = user + sys + nice`, `total = busy + idle`, `min(max(busy / total, 0), 1)`, 0 when `total == 0`; 60 history points | `parse_proc_stat`, `cpu_usage`; `CPU_HISTORY` = 60 |
| Per-core load | `readCores`, same formula per core | `core_usages` |
| Memory | `readMemory`: used = app + wired + compressed; `memFraction = used / total` | `parse_proc_meminfo` |
| Memory pressure | `kern.memorystatus_vm_pressure_level` (normal, warning, critical) | derived from the available share (below) |
| Swap | `vm.swapusage` used | `SwapTotal - SwapFree` |
| Disk | `readDisk`: free = available capacity, `diskFraction = 1 - free / total` | `statvfs` (`f_bavail`), `disk_info_from_bytes` |
| Network | `readNetwork`: sum of every interface not starting with `lo`, rate = delta / dt, 0 when the counter went back; 40 history points | `parse_proc_net_dev`, `net_rates`; `NET_HISTORY` = 40 |
| Battery | `readBattery`: `percent = cur * 100 / max`, `minutes = charging ? timeToFull : (onAC ? -1 : timeToEmpty)`, shown when > 0 | `parse_battery` from `/sys/class/power_supply` |
| GPU | `HardwareStats.gpuUtilization`: the largest "Device Utilization %" of the accelerators, `nil` when not reported | `read_gpu_usage`: `gpu_busy_percent` of the amdgpu driver, `None` otherwise |
| Top processes | `readTopProcesses`: `ps -Aceo pid,pcpu,rss,comm -r`, the card shows the first 5 | `top_processes`, `TOP_COUNT` = 5, by CPU and by memory |
| Quit | `quit`: SIGTERM, `force` = SIGKILL; PID <= 1 and Menew itself refused; system and other-user processes refused | `sys::quit_process`, `is_offered` |
| Formatting | `Fmt` (`bytes`, `mem`, `memShort`, `rate`, `shortRate`, `pct`, `duration`) | `Fmt` |
| Menu bar items | `MenuBarItem.segment`: `CPU 23%`, graph of the last 12 CPU points, `RAM 64%`, two short rates, `SSD 58%`, `BAT 80%` | `format_tray_metrics` (StatusNotifierItem `Title` and `ToolTip`) |
| Palette entry | `Palette.swift`: `L("Monitor do sistema")`, keywords `sistema system cpu memória memory ram bateria battery rede network monitor` | `palette.rs` `action.monitor` |

## Where Linux differs (also in `docs/parity.md`)

- **CPU ticks**: `/proc/stat` has more counters than Mach. Busy = `user + nice + system + irq + softirq + steal`,
  idle = `idle + iowait` (guest time is already inside user). The Mac formula is otherwise kept.
- **Per-process CPU**: `ps -o pcpu` on Linux is the average since the process started, which would hide a process
  that just went wild. Linux computes it like the Mac's `ps -r` shows it, over the last interval: the growth of
  `utime + stime` of `/proc/<pid>/stat` divided by the growth of the total ticks, times the number of cores
  (100% = one core busy, can pass 100%). A process with no previous sample counts 0.
- **Memory**: used = `MemTotal - MemAvailable`. The card keeps the Mac three parts and they add up to the used
  memory: Apps = `AnonPages`, Compressed = `Zswap` (0 without zswap), System = the rest (page cache that cannot be
  dropped, kernel, shared memory). A part never goes over what is left (an `AnonPages` larger than the used
  memory is clamped).
- **Memory pressure** has no Linux counterpart: it is derived from `MemAvailable / MemTotal`: normal from 25%,
  warning from 10% to under 25%, critical under 10%.
- **Network**: `/proc/net/dev` has the same "every interface except `lo`" sum as `getifaddrs`. Virtual
  interfaces (bridges, veth) are counted, like the Mac counts everything but loopback.
- **Battery times**: Linux has no ready-made time to empty or to full. They are `energy_now / power` when
  discharging and `(energy_full - energy_now) / power` when charging (batteries that report charge and current
  use the voltage to get energy and power). With no power reading there is no time. `Not charging` and `Full`
  count as plugged in.
- **GPU**: only `gpu_busy_percent` (amdgpu) is a real load. Intel and NVIDIA do not report one in sysfs, and a
  frequency ratio is not a load, so there the card is hidden (the Mac hides it when `nil` too).
- **Processes offered**: only the current user's own processes, never PID 1 or Menew itself. Root, system
  services and other users never appear and `quit_process` refuses them (the Mac: "system processes are never
  suggested").
- **Byte units**: `ByteCountFormatter` is approximated: decimal units for files and rates, binary units for
  memory, one decimal when the value is not whole.
- **Tray**: StatusNotifierItem has a `Title` (KDE Plasma shows it as text in the panel) and a `ToolTip`. GNOME's
  AppIndicator shows only the icon, so on GNOME the metrics are in the tooltip only (where the extension shows
  it). The Mac `temp` item needs sensors (R2) and is not offered.
- **Choice of items**: `monitor_tray_items` in `config.json` (the Mac `menuBarItems`). Empty by default: nothing
  is sampled until the user opts in, because the settings window only comes in Phase 11.

## Vectors

### A. CPU load between two samples

`Fmt::pct(cpu_usage(prev, curr))` for the aggregate counters.

| id | prev_busy | prev_total | curr_busy | curr_total | expected |
|---|---|---|---|---|---|
| A01 | 1000 | 10000 | 1200 | 11000 | 20% |
| A02 | 5000 | 20000 | 5000 | 30000 | 0% |
| A03 | 5000 | 20000 | 15000 | 30000 | 100% |
| A04 | 1000 | 10000 | 1000 | 10000 | 0% |
| A05 | 1000 | 10000 | 1230 | 11000 | 23% |
| A06 | 100000 | 500000 | 104500 | 510000 | 45% |

### B. The `cpu` line of `/proc/stat`

Columns: the eight counters of the line (user nice system idle iowait irq softirq steal), then the busy and total
ticks that `parse_proc_stat` has to give. busy = user + nice + system + irq + softirq + steal, total = busy + idle + iowait.

| id | counters | expected_busy | expected_total |
|---|---|---|---|
| B01 | 100 20 30 400 50 6 7 8 | 171 | 621 |
| B02 | 0 0 0 100 0 0 0 0 | 0 | 100 |
| B03 | 500 0 500 0 0 0 0 0 | 1000 | 1000 |
| B04 | 10 0 5 85 0 0 0 0 | 15 | 100 |

### C. Memory

Columns: values in kB of `MemTotal`, `MemAvailable`, `AnonPages`, `Zswap`, `SwapTotal`, `SwapFree`, then the
expected used fraction, pressure, Apps, System, Compressed and swap used (kB).

| id | total | avail | anon | zswap | swap_total | swap_free | pct | pressure | app | system | compressed | swap_used |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| C01 | 16000000 | 12000000 | 3000000 | 0 | 4000000 | 4000000 | 25% | normal | 3000000 | 1000000 | 0 | 0 |
| C02 | 16000000 | 8000000 | 5000000 | 500000 | 4000000 | 3000000 | 50% | normal | 5000000 | 2500000 | 500000 | 1000000 |
| C03 | 16000000 | 3200000 | 9000000 | 1000000 | 4000000 | 2000000 | 80% | warning | 9000000 | 2800000 | 1000000 | 2000000 |
| C04 | 16000000 | 800000 | 12000000 | 0 | 4000000 | 1000000 | 95% | critical | 12000000 | 3200000 | 0 | 3000000 |
| C05 | 32000000 | 32000000 | 0 | 0 | 8000000 | 8000000 | 0% | normal | 0 | 0 | 0 | 0 |
| C06 | 8000000 | 7100000 | 2000000 | 0 | 0 | 0 | 11% | normal | 900000 | 0 | 0 | 0 |
| C07 | 16000000 | 4000000 | 0 | 0 | 0 | 0 | 75% | normal | 0 | 12000000 | 0 | 0 |
| C08 | 16000000 | 1600000 | 0 | 0 | 0 | 0 | 90% | warning | 0 | 14400000 | 0 | 0 |

### D. Disk

| id | total_bytes | free_bytes | expected_used_bytes | expected_pct |
|---|---|---|---|---|
| D01 | 1000000000000 | 400000000000 | 600000000000 | 60% |
| D02 | 500000000000 | 500000000000 | 0 | 0% |
| D03 | 500000000000 | 50000000000 | 450000000000 | 90% |
| D04 | 256000000000 | 107520000000 | 148480000000 | 58% |
| D05 | 0 | 0 | 0 | 0% |

### E. Network rate between two samples

Rate of the received bytes (bytes per second), then `Fmt::short_rate` and `Fmt::rate` (English).

| id | prev_rx | curr_rx | dt_secs | expected_bps | expected_short | expected_rate |
|---|---|---|---|---|---|---|
| E01 | 1000000 | 1000000 | 2.0 | 0 | 0B | 0 B/s |
| E02 | 1000000 | 1000800 | 2.0 | 400 | 400B | 400 B/s |
| E03 | 1000000 | 1096000 | 2.0 | 48000 | 48K | 48 KB/s |
| E04 | 1000000 | 3400000 | 2.0 | 1200000 | 1.2M | 1.2 MB/s |
| E05 | 5000000 | 10000000 | 1.0 | 5000000 | 5.0M | 5 MB/s |
| E06 | 5000000 | 1000 | 2.0 | 0 | 0B | 0 B/s |

### F. Battery

Columns: the sysfs capacity (%), status, power drawn (W), energy now and full (Wh), then percent, charging,
on AC and the minutes left (`-` for none).

| id | capacity | status | power_w | now_wh | full_wh | percent | charging | on_ac | minutes |
|---|---|---|---|---|---|---|---|---|---|
| F01 | 80 | Discharging | 20.0 | 40.0 | 50.0 | 80 | false | false | 120 |
| F02 | 50 | Charging | 30.0 | 25.0 | 50.0 | 50 | true | true | 50 |
| F03 | 100 | Full | 0.0 | 50.0 | 50.0 | 100 | false | true | - |
| F04 | 15 | Discharging | 15.0 | 7.5 | 50.0 | 15 | false | false | 30 |
| F05 | 90 | Not charging | 5.0 | 45.0 | 50.0 | 90 | false | true | - |
| F06 | 60 | Discharging | 0.0 | 30.0 | 50.0 | 60 | false | false | - |
| F07 | 99 | Charging | 0.0 | 49.5 | 50.0 | 99 | true | true | - |

### G. One `/proc/<pid>/stat` line

`parse_pid_stat(line, 4, uid)`: page size 4 KiB. The name is between the first `(` and the last `)`, utime and stime
are fields 14 and 15, rss (pages) is field 24. `-` as the name means the line is rejected.

| id | uid | line | expected_name | expected_ticks | expected_rss_kb |
|---|---|---|---|---|---|
| G01 | 1000 | 2001 (cargo) S 1 2001 2001 0 -1 4194304 0 0 0 0 4500 500 0 0 20 0 1 0 100 0 25000 | cargo | 5000 | 100000 |
| G02 | 1000 | 2002 (Web Content (x)) S 1 2002 2002 0 -1 4194304 0 0 0 0 7 3 0 0 20 0 1 0 100 0 10 | Web Content (x) | 10 | 40 |
| G03 | 0 | 1 (systemd) S 0 1 1 0 -1 4194560 5039 68073 120 2020 23 33 259 103 20 0 1 0 5628086 22249472 3092 | systemd | 56 | 12368 |
| G04 | 1000 | 2003 (short) S 1 2 | - | - | - |
| G05 | 1000 | not a stat line | - | - | - |

### H. Which processes may be offered or quit

`is_offered(uid, pid, my_uid, my_pid)`.

| id | uid | pid | my_uid | my_pid | expected |
|---|---|---|---|---|---|
| H01 | 0 | 1 | 1000 | 9999 | false |
| H02 | 0 | 500 | 1000 | 9999 | false |
| H03 | 990 | 1050 | 1000 | 9999 | false |
| H04 | 1000 | 9999 | 1000 | 9999 | false |
| H05 | 1000 | 2001 | 1000 | 9999 | true |
| H06 | 1001 | 2002 | 1000 | 9999 | false |
| H07 | 1000 | 1 | 1000 | 9999 | false |
| H08 | 0 | 2 | 1000 | 9999 | false |

### I. Top processes

Columns: `by` (cpu or mem), the processes as `pid/uid/ticks_now/ticks_before/rss_kb` separated by `;` (`-` as ticks_before =
not in the previous sample), the growth of the total ticks, the number of cores, then the PIDs in order and the
CPU of the first one. Current user 1000, Menew is PID 99, the list has at most 5.

| id | by | procs | total_delta | ncpu | expected_pids | expected_first_cpu |
|---|---|---|---|---|---|---|
| I01 | cpu | 10/1000/1500/1000/100;11/1000/1200/1100/900;12/0/9000/0/50;13/1000/700/700/5000 | 1000 | 1 | 10,11,13 | 50% |
| I02 | mem | 10/1000/1500/1000/100;11/1000/1200/1100/900;12/0/9000/0/50;13/1000/700/700/5000 | 1000 | 1 | 13,11,10 | 0% |
| I03 | cpu | 20/1000/1000/0/10 | 4000 | 4 | 20 | 100% |
| I04 | cpu | 30/1000/900/-/10;31/1000/800/-/20 | 1000 | 1 | 31,30 | 0% |
| I05 | cpu | 101/1000/10/0/1;102/1000/20/0/1;103/1000/30/0/1;104/1000/40/0/1;105/1000/50/0/1;106/1000/60/0/1;107/1000/70/0/1 | 1000 | 1 | 107,106,105,104,103 | 7% |
| I06 | cpu | 99/1000/5000/0/1;40/1000/10/0/1 | 1000 | 1 | 40 | 1% |
| I07 | cpu | 50/1000/500/0/1 | 0 | 1 | 50 | 0% |

### J. Formatting

Functions: `mem_short`, `short_rate`, `pct`, `duration`, `bytes`, `mem`, `battery_minutes`.

| id | lang | function | input | expected |
|---|---|---|---|---|
| J01 | en | mem_short | 17179869184 | 16 GB |
| J02 | pt | mem_short | 17179869184 | 16 GB |
| J03 | en | mem_short | 4831838208 | 4.5 GB |
| J04 | pt | mem_short | 4831838208 | 4,5 GB |
| J05 | de | mem_short | 4831838208 | 4,5 GB |
| J06 | es | mem_short | 4831838208 | 4,5 GB |
| J07 | zh | mem_short | 4831838208 | 4.5 GB |
| J08 | en | mem_short | 12884901888 | 12 GB |
| J09 | en | short_rate | 1200000 | 1.2M |
| J10 | en | short_rate | 48000 | 48K |
| J11 | en | short_rate | 350 | 350B |
| J12 | en | pct | 0.23 | 23% |
| J13 | en | pct | 1.0 | 100% |
| J14 | en | pct | 0.0 | 0% |
| J15 | en | pct | 0.456 | 46% |
| J16 | en | duration | 120 | 0h 2min |
| J17 | en | duration | 3660 | 1h 1min |
| J18 | en | duration | 90000 | 1d 1h 0min |
| J19 | en | bytes | 1200000 | 1.2 MB |
| J20 | pt | bytes | 1200000 | 1,2 MB |
| J21 | de | bytes | 48000 | 48 KB |
| J22 | en | bytes | 350 | 350 B |
| J23 | es | bytes | 2500000000 | 2,5 GB |
| J24 | zh | bytes | 296000000000 | 296 GB |
| J25 | en | mem | 1073741824 | 1 GB |
| J26 | en | mem | 524288000 | 500 MB |
| J27 | pt | mem | 1610612736 | 1,5 GB |
| J28 | en | battery_minutes | 200 | 3 h 20 min |
| J29 | en | battery_minutes | 45 | 45 min |
| J30 | en | battery_minutes | 60 | 1 h 0 min |

### K. Tray title and tooltip

`format_tray_metrics(items, values, lang)`. Values: cpu, ram and disk as fractions, `net` as `in/out` bytes per second,
`battery` as `percent/minutes/state` (`-` when the machine has none). The title uses the Mac texts and order
(`cpu cpuGraph ram net disk battery`, whatever the order of the chosen items), the tooltip is localized,
one line per item (`\n` between lines). An item the machine does not report is left out. `temp` and unknown items
are ignored.

| id | lang | items | cpu | ram | net | disk | battery | title | tooltip |
|---|---|---|---|---|---|---|---|---|---|
| K01 | en | cpu,ram | 0.23 | 0.64 | - | - | - | CPU 23% · RAM 64% | CPU: 23%\nMemory: 64% |
| K02 | pt | cpu,ram | 0.23 | 0.64 | - | - | - | CPU 23% · RAM 64% | CPU: 23%\nMemória: 64% |
| K03 | en | ram,cpu | 0.23 | 0.64 | - | - | - | CPU 23% · RAM 64% | CPU: 23%\nMemory: 64% |
| K04 | en | cpu | 0.15 | - | - | - | - | CPU 15% | CPU: 15% |
| K05 | en | net | - | - | 1200000/48000 | - | - | ↓1.2M ↑48K | Network: ↓ 1.2 MB/s  ↑ 48 KB/s |
| K06 | de | net | - | - | 1200000/48000 | - | - | ↓1.2M ↑48K | Netzwerk: ↓ 1,2 MB/s  ↑ 48 KB/s |
| K07 | zh | cpu,ram,disk | 0.05 | 0.4 | - | 0.58 | - | CPU 5% · RAM 40% · SSD 58% | CPU: 5%\n内存: 40%\n磁盘: 58% |
| K08 | en | battery | - | - | - | - | 80/200/discharging | BAT 80% | Battery: 80% (On battery, remaining 3 h 20 min) |
| K09 | en | battery | - | - | - | - | 50/50/charging | BAT 50% | Battery: 50% (Full in 50 min) |
| K10 | es | battery | - | - | - | - | 100/-/plugged | BAT 100% | Batería: 100% (Conectada) |
| K11 | en | cpu,battery | 0.5 | - | - | - | - | CPU 50% | CPU: 50% |
| K12 | en | temp,cpu | 0.5 | - | - | - | - | CPU 50% | CPU: 50% |
| K13 | en | - | 0.5 | 0.5 | - | - | - | (empty) | (empty) |

### L. Tray graph

The `cpuGraph` item draws the last 12 values of the CPU history as eight-level bars (value x 7, rounded).

| id | history | expected |
|---|---|---|
| L01 | 0.0,0.5,1.0 | ▁▅█ |
| L02 | 1,1,0,0,0,0,0,0,0,0,0,0,0,1 | ▁▁▁▁▁▁▁▁▁▁▁█ |
| L03 | 0.25,0.75 | ▃▆ |

### Fixtures (real captures)

`crates/menew-core/tests/fixtures/` holds real files captured on Linux (WSL2, Ubuntu 24.04, 8 cores): `/proc/stat`
twice 2 s apart (`proc_stat_1.txt`, `proc_stat_2.txt`), `/proc/net/dev` twice 1 s apart, `/proc/meminfo` and
`pid_stat_2.txt` (a few `/proc/<pid>/stat` lines with their owner, a busy `yes` process of the user among them).
The expected numbers below were computed with `awk` from the files, not from the Rust code:

| Fixture | Computation | Expected |
|---|---|---|
| stat 1 to 2 | busy 2041331 - 2040696 = 635, total 44878560 - 44876913 = 1647 | 39% (0.3855) |
| net 1 to 2 (1 s) | `eth0` rx 707701872 - 707691434 = 10438, tx 3708619 - 3702608 = 6011, `lo` ignored | 10438 B/s, 6011 B/s |
| meminfo | used = 32868548 - 29876648 = 2991900 kB, Apps (AnonPages) 2034132 kB, System 957768 kB, no zswap, no swap in use | 9%, normal |
