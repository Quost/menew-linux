//! System monitor view and window (GTK4 / libadwaita). Port of the "Mac details" cards of
//! `MonitorViews.swift` and `Views.swift`: processor, memory, disk, network, battery and the top
//! processes. [`MonitorWidget`] only draws a [`Snapshot`]; it never reads the machine, so Phase 11 can
//! put it inside the Menew window and `--snapshot` can feed it fake numbers. [`MonitorWindow`] owns the
//! sampling, which runs only while the window is visible.

use gtk4::cairo;
use gtk4::prelude::*;
use gtk4::{gdk, glib};
use libadwaita as adw;
use libadwaita::prelude::*;
use menew_core::monitor::{self, battery_state, Fmt, MemoryPressure, ProcInfo, Sampler, Snapshot, INTERVAL_SECS};
use menew_core::strings::tr;
use menew_core::Language;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::time::Duration;
use tracing::{info, warn};

type Rgb = (f64, f64, f64);
const CPU_COLOR: Rgb = (0.21, 0.52, 0.89);
const MEM_COLOR: Rgb = (0.56, 0.35, 0.80);
const SYS_COLOR: Rgb = (0.96, 0.55, 0.12);
const COMP_COLOR: Rgb = (0.90, 0.75, 0.10);
const DOWN_COLOR: Rgb = (0.18, 0.65, 0.35);
const UP_COLOR: Rgb = (0.90, 0.45, 0.15);

/// The parts (value, color) of the memory bar and their total.
type BarParts = (Vec<(f64, Rgb)>, f64);

fn foreground() -> Rgb {
    if adw::StyleManager::default().is_dark() { (1.0, 1.0, 1.0) } else { (0.0, 0.0, 0.0) }
}

/// A line graph of `values` (0...1 unless `scale_to_max`), filled underneath.
fn sparkline(color: Rgb, scale_to_max: bool, height: i32) -> (gtk4::DrawingArea, Rc<RefCell<Vec<f64>>>) {
    let data: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let area = gtk4::DrawingArea::new();
    area.set_content_height(height);
    area.set_hexpand(true);
    let d = Rc::clone(&data);
    area.set_draw_func(move |_, cr, w, h| {
        let (w, h) = (w as f64, h as f64);
        let (fr, fg, fb) = foreground();
        cr.set_source_rgba(fr, fg, fb, 0.06);
        rounded(cr, 0.0, 0.0, w, h, 6.0);
        let _ = cr.fill();
        let v = d.borrow();
        if v.len() < 2 {
            return;
        }
        let max = if scale_to_max { v.iter().cloned().fold(1.0, f64::max) } else { 1.0 };
        let step = w / (v.len() - 1) as f64;
        let y = |x: f64| h - 3.0 - (x / max).clamp(0.0, 1.0) * (h - 6.0);
        cr.set_source_rgba(color.0, color.1, color.2, 0.20);
        cr.move_to(0.0, h);
        for (i, x) in v.iter().enumerate() {
            cr.line_to(i as f64 * step, y(*x));
        }
        cr.line_to(w, h);
        cr.close_path();
        let _ = cr.fill();
        cr.set_source_rgba(color.0, color.1, color.2, 1.0);
        cr.set_line_width(1.6);
        for (i, x) in v.iter().enumerate() {
            if i == 0 { cr.move_to(0.0, y(*x)) } else { cr.line_to(i as f64 * step, y(*x)) }
        }
        let _ = cr.stroke();
    });
    (area, data)
}

fn rounded(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    cr.arc(x + r, y + h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
    cr.arc(x + r, y + r, r, std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
    cr.close_path();
}

/// A bar split in colored parts of a total (the memory bar: Apps, System, Compressed).
fn segmented_bar(height: i32) -> (gtk4::DrawingArea, Rc<RefCell<BarParts>>) {
    let data: Rc<RefCell<BarParts>> = Rc::new(RefCell::new((Vec::new(), 1.0)));
    let area = gtk4::DrawingArea::new();
    area.set_content_height(height);
    area.set_hexpand(true);
    let d = Rc::clone(&data);
    area.set_draw_func(move |_, cr, w, h| {
        let (w, h) = (w as f64, h as f64);
        let (fr, fg, fb) = foreground();
        cr.set_source_rgba(fr, fg, fb, 0.08);
        rounded(cr, 0.0, 0.0, w, h, h / 2.0);
        let _ = cr.fill();
        let (parts, total) = &*d.borrow();
        cr.save().ok();
        rounded(cr, 0.0, 0.0, w, h, h / 2.0);
        cr.clip();
        let mut x = 0.0;
        for (v, c) in parts {
            let pw = (w * v / total.max(1.0) - 1.5).max(0.0);
            cr.set_source_rgb(c.0, c.1, c.2);
            cr.rectangle(x, 0.0, pw, h);
            let _ = cr.fill();
            x += pw + 1.5;
        }
        cr.restore().ok();
    });
    (area, data)
}

/// One small vertical bar per core (`CoreBars`).
fn core_bars() -> (gtk4::DrawingArea, Rc<RefCell<Vec<f64>>>) {
    let data: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let area = gtk4::DrawingArea::new();
    area.set_content_height(22);
    area.set_hexpand(true);
    let d = Rc::clone(&data);
    area.set_draw_func(move |_, cr, w, h| {
        let v = d.borrow();
        if v.is_empty() {
            return;
        }
        let (w, h) = (w as f64, h as f64);
        let gap = 3.0;
        let bw = ((w - gap * (v.len() - 1) as f64) / v.len() as f64).max(1.0);
        let (fr, fg, fb) = foreground();
        for (i, x) in v.iter().enumerate() {
            let left = i as f64 * (bw + gap);
            cr.set_source_rgba(fr, fg, fb, 0.08);
            rounded(cr, left, 0.0, bw, h, 2.0);
            let _ = cr.fill();
            cr.set_source_rgb(CPU_COLOR.0, CPU_COLOR.1, CPU_COLOR.2);
            let bh = (h * x.clamp(0.0, 1.0)).max(2.0);
            rounded(cr, left, h - bh, bw, bh, 2.0);
            let _ = cr.fill();
        }
    });
    (area, data)
}

fn card(title: &str) -> (gtk4::Box, gtk4::Box) {
    let outer = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    outer.add_css_class("card");
    outer.set_margin_start(2);
    outer.set_margin_end(2);
    let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    inner.set_margin_top(12);
    inner.set_margin_bottom(12);
    inner.set_margin_start(14);
    inner.set_margin_end(14);
    let head = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let t = gtk4::Label::new(Some(title));
    t.add_css_class("heading");
    t.set_xalign(0.0);
    t.set_hexpand(true);
    head.append(&t);
    inner.append(&head);
    outer.append(&inner);
    (outer, inner)
}

fn dim(text: &str) -> gtk4::Label {
    let l = gtk4::Label::new(Some(text));
    l.add_css_class("dim-label");
    l.add_css_class("caption");
    l.set_xalign(0.0);
    l
}

fn big(text: &str, class: &str) -> gtk4::Label {
    let l = gtk4::Label::new(Some(text));
    l.add_css_class(class);
    l.add_css_class("numeric");
    l.set_xalign(0.0);
    l
}

/// A small value with its title above, in a column (`LegendValue`, `BatteryValue`).
fn stat(title: &str, color: Option<Rgb>) -> (gtk4::Box, gtk4::Label) {
    let b = gtk4::Box::new(gtk4::Orientation::Vertical, 1);
    b.set_hexpand(true);
    b.set_homogeneous(false);
    let t = dim(title);
    if let Some(c) = color {
        t.set_markup(&format!(
            "<span foreground=\"#{:02x}{:02x}{:02x}\">\u{25cf}</span> {}",
            (c.0 * 255.0) as u8,
            (c.1 * 255.0) as u8,
            (c.2 * 255.0) as u8,
            glib::markup_escape_text(title)
        ));
    }
    let v = gtk4::Label::new(Some("-"));
    v.add_css_class("heading");
    v.add_css_class("numeric");
    v.set_xalign(0.0);
    b.append(&t);
    b.append(&v);
    (b, v)
}

/// The list of processes of one top list, rebuilt at every sample.
struct ProcList {
    group: adw::PreferencesGroup,
    list: gtk4::ListBox,
}

impl ProcList {
    fn new(title: &str) -> Self {
        let group = adw::PreferencesGroup::new();
        group.set_title(title);
        let list = gtk4::ListBox::new();
        list.add_css_class("boxed-list");
        list.set_selection_mode(gtk4::SelectionMode::None);
        group.add(&list);
        ProcList { group, list }
    }
}

/// Reusable monitor widget: draws what [`MonitorWidget::update`] is given.
pub struct MonitorWidget {
    pub root: gtk4::ScrolledWindow,
    lang: Language,
    cpu_pct: gtk4::Label,
    cpu_graph: (gtk4::DrawingArea, Rc<RefCell<Vec<f64>>>),
    gpu_box: gtk4::Box,
    gpu_pct: gtk4::Label,
    cores_box: gtk4::Box,
    cores_title: gtk4::Label,
    cores_avg: gtk4::Label,
    cores: (gtk4::DrawingArea, Rc<RefCell<Vec<f64>>>),
    mem_card: gtk4::Box,
    mem_badge: gtk4::Label,
    mem_bar: (gtk4::DrawingArea, Rc<RefCell<BarParts>>),
    mem_vals: [gtk4::Label; 4],
    disk_free: gtk4::Label,
    disk_bar: gtk4::LevelBar,
    disk_text: gtk4::Label,
    net_down: gtk4::Label,
    net_up: gtk4::Label,
    net_down_graph: (gtk4::DrawingArea, Rc<RefCell<Vec<f64>>>),
    net_up_graph: (gtk4::DrawingArea, Rc<RefCell<Vec<f64>>>),
    bat_card: gtk4::Box,
    bat_pct: gtk4::Label,
    bat_state: gtk4::Label,
    bat_bar: gtk4::LevelBar,
    top_cpu: ProcList,
    top_mem: ProcList,
    /// Processes asked to quit with SIGTERM: while they are still running, "Force Quit" is offered.
    termed: Rc<RefCell<HashSet<u32>>>,
}

impl MonitorWidget {
    pub fn new(lang: Language) -> Rc<Self> {
        let t = |k: &str| tr(k, lang, &[]);
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        content.set_margin_top(14);
        content.set_margin_bottom(14);
        content.set_margin_start(14);
        content.set_margin_end(14);
        let root = gtk4::ScrolledWindow::new();
        root.set_hscrollbar_policy(gtk4::PolicyType::Never);
        root.set_vexpand(true);
        root.set_child(Some(&content));

        // processor
        let (cpu_card, cpu_in) = card(&t("mon_processor"));
        if let Some(head) = cpu_in.first_child().and_then(|c| c.downcast::<gtk4::Box>().ok()) {
            head.append(&dim(&t("mon_last_2min")));
        }
        let cpu_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        let cpu_left = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        cpu_left.set_size_request(86, -1);
        let cpu_pct = big("0%", "title-1");
        cpu_left.append(&cpu_pct);
        cpu_left.append(&dim(&t("mon_in_use")));
        let cpu_graph = sparkline(CPU_COLOR, false, 46);
        cpu_row.append(&cpu_left);
        cpu_row.append(&cpu_graph.0);
        cpu_in.append(&cpu_row);
        let gpu_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        let gpu_left = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        gpu_left.set_size_request(86, -1);
        let gpu_title = dim("GPU");
        gpu_title.add_css_class("heading");
        let gpu_pct = big("0%", "heading");
        gpu_left.append(&gpu_title);
        gpu_left.append(&gpu_pct);
        gpu_box.append(&gpu_left);
        gpu_box.set_visible(false);
        cpu_in.append(&gpu_box);
        let cores_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        let cores_head = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        let cores_title = gtk4::Label::new(Some(&t("mon_cores")));
        cores_title.add_css_class("heading");
        let cores_count = dim("");
        cores_count.set_hexpand(true);
        let cores_avg = big("0%", "heading");
        cores_head.append(&cores_title);
        cores_head.append(&cores_count);
        cores_head.append(&cores_avg);
        let cores = core_bars();
        cores_box.append(&cores_head);
        cores_box.append(&cores.0);
        cores_box.set_visible(false);
        cpu_in.append(&cores_box);
        content.append(&cpu_card);

        // memory
        let (mem_card, mem_in) = card(&t("mon_memory"));
        let mem_badge = gtk4::Label::new(None);
        mem_badge.add_css_class("caption-heading");
        if let Some(head) = mem_in.first_child().and_then(|c| c.downcast::<gtk4::Box>().ok()) {
            head.append(&mem_badge);
        }
        let mem_bar = segmented_bar(8);
        mem_in.append(&mem_bar.0);
        let legend = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        legend.set_homogeneous(true);
        let mut vals: Vec<gtk4::Label> = Vec::new();
        for (key, color) in [("mon_apps", Some(MEM_COLOR)), ("mon_system", Some(SYS_COLOR)), ("mon_compressed", Some(COMP_COLOR)), ("mon_swap", None)] {
            let (b, v) = stat(&t(key), color);
            legend.append(&b);
            vals.push(v);
        }
        mem_in.append(&legend);
        let mem_vals: [gtk4::Label; 4] = vals.try_into().expect("four legend values");
        content.append(&mem_card);

        // disk
        let (disk_card, disk_in) = card(&t("mon_disk"));
        let disk_free = dim("");
        if let Some(head) = disk_in.first_child().and_then(|c| c.downcast::<gtk4::Box>().ok()) {
            head.append(&disk_free);
        }
        let disk_bar = gtk4::LevelBar::new();
        disk_bar.set_min_value(0.0);
        disk_bar.set_max_value(1.0);
        let disk_text = gtk4::Label::new(None);
        disk_text.add_css_class("numeric");
        disk_text.set_xalign(0.0);
        disk_in.append(&disk_bar);
        disk_in.append(&disk_text);
        content.append(&disk_card);

        // network
        let (net_card, net_in) = card(&t("mon_network"));
        let net_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
        let mut net_widgets = Vec::new();
        for (key, color) in [("mon_download", DOWN_COLOR), ("mon_upload", UP_COLOR)] {
            let col = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
            col.set_hexpand(true);
            col.append(&dim(&t(key)));
            let v = big("0 B/s", "heading");
            col.append(&v);
            let g = sparkline(color, true, 30);
            col.append(&g.0);
            net_row.append(&col);
            net_widgets.push((v, g));
        }
        net_in.append(&net_row);
        let (net_up, net_up_graph) = net_widgets.pop().unwrap();
        let (net_down, net_down_graph) = net_widgets.pop().unwrap();
        content.append(&net_card);

        // battery (only on a machine that has one)
        let (bat_card, bat_in) = card(&t("mon_battery"));
        let bat_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        let bat_pct = big("0%", "title-1");
        bat_pct.set_size_request(86, -1);
        let bat_state = gtk4::Label::new(None);
        bat_state.set_xalign(0.0);
        bat_state.set_hexpand(true);
        bat_state.set_wrap(true);
        bat_row.append(&bat_pct);
        bat_row.append(&bat_state);
        let bat_bar = gtk4::LevelBar::new();
        bat_bar.set_min_value(0.0);
        bat_bar.set_max_value(1.0);
        bat_in.append(&bat_row);
        bat_in.append(&bat_bar);
        bat_card.set_visible(false);
        content.append(&bat_card);

        // top processes
        let top_cpu = ProcList::new(&t("mon_top_cpu"));
        let top_mem = ProcList::new(&t("mon_top_mem"));
        content.append(&top_cpu.group);
        content.append(&top_mem.group);

        Rc::new(MonitorWidget {
            root,
            lang,
            cpu_pct,
            cpu_graph,
            gpu_box,
            gpu_pct,
            cores_box,
            cores_title,
            cores_avg,
            cores,
            mem_card,
            mem_badge,
            mem_bar,
            mem_vals,
            disk_free,
            disk_bar,
            disk_text,
            net_down,
            net_up,
            net_down_graph,
            net_up_graph,
            bat_card,
            bat_pct,
            bat_state,
            bat_bar,
            top_cpu,
            top_mem,
            termed: Rc::new(RefCell::new(HashSet::new())),
        })
    }

    /// Show a sample.
    pub fn update(self: &Rc<Self>, s: &Snapshot) {
        let lang = self.lang;
        self.cpu_pct.set_text(&Fmt::pct(s.cpu));
        *self.cpu_graph.1.borrow_mut() = s.cpu_history.clone();
        self.cpu_graph.0.queue_draw();

        match s.gpu {
            Some(g) => {
                self.gpu_box.set_visible(true);
                self.gpu_pct.set_text(&Fmt::pct(g));
            }
            None => self.gpu_box.set_visible(false),
        }

        if s.cores.is_empty() {
            self.cores_box.set_visible(false);
        } else {
            self.cores_box.set_visible(true);
            let avg = s.cores.iter().sum::<f64>() / s.cores.len() as f64;
            self.cores_avg.set_text(&Fmt::pct(avg));
            let n = s.cores.len().to_string();
            self.cores_title.set_text(&tr("mon_n_cores", lang, &[&n]));
            *self.cores.1.borrow_mut() = s.cores.clone();
            let tip = s
                .cores
                .iter()
                .enumerate()
                .map(|(i, v)| tr("mon_core_n", lang, &[&(i + 1).to_string(), &Fmt::pct(*v)]))
                .collect::<Vec<_>>()
                .join("\n");
            self.cores.0.set_tooltip_text(Some(&tip));
            self.cores.0.queue_draw();
        }

        if let Some(m) = &s.mem {
            self.mem_card.set_visible(true);
            self.mem_badge.set_text(&tr(m.pressure.string_key(), lang, &[]));
            for c in ["success", "warning", "error"] {
                self.mem_badge.remove_css_class(c);
            }
            self.mem_badge.add_css_class(match m.pressure {
                MemoryPressure::Normal => "success",
                MemoryPressure::Warning => "warning",
                MemoryPressure::Critical => "error",
            });
            let b = m.breakdown;
            *self.mem_bar.1.borrow_mut() = (
                vec![(b.app as f64, MEM_COLOR), (b.wired as f64, SYS_COLOR), (b.compressed as f64, COMP_COLOR)],
                m.total as f64,
            );
            self.mem_bar.0.queue_draw();
            for (label, bytes) in self.mem_vals.iter().zip([b.app, b.wired, b.compressed, m.swap_used]) {
                label.set_text(&Fmt::mem_short(bytes, lang));
            }
        } else {
            self.mem_card.set_visible(false);
        }

        if let Some(d) = &s.disk {
            self.disk_free.set_text(&tr("mon_free", lang, &[&Fmt::bytes(d.free as i64, lang)]));
            self.disk_bar.set_value(d.fraction);
            self.disk_text.set_text(&format!(
                "{} / {} ({})",
                Fmt::bytes(d.used as i64, lang),
                Fmt::bytes(d.total as i64, lang),
                Fmt::pct(d.fraction)
            ));
        }

        self.net_down.set_text(&Fmt::rate(s.net_in, lang));
        self.net_up.set_text(&Fmt::rate(s.net_out, lang));
        *self.net_down_graph.1.borrow_mut() = s.net_in_history.clone();
        *self.net_up_graph.1.borrow_mut() = s.net_out_history.clone();
        self.net_down_graph.0.queue_draw();
        self.net_up_graph.0.queue_draw();

        match &s.battery {
            Some(b) => {
                self.bat_card.set_visible(true);
                self.bat_pct.set_text(&format!("{}%", b.percent));
                self.bat_bar.set_value(b.percent as f64 / 100.0);
                self.bat_state.set_text(&battery_state(b, lang));
            }
            None => self.bat_card.set_visible(false),
        }

        let alive: HashSet<u32> = s.top_cpu.iter().chain(&s.top_mem).map(|p| p.pid).collect();
        self.termed.borrow_mut().retain(|pid| alive.contains(pid));
        self.fill(&self.top_cpu, &s.top_cpu);
        self.fill(&self.top_mem, &s.top_mem);
    }

    fn fill(self: &Rc<Self>, list: &ProcList, procs: &[ProcInfo]) {
        while let Some(c) = list.list.first_child() {
            list.list.remove(&c);
        }
        if procs.is_empty() {
            let row = adw::ActionRow::new();
            row.set_title(&tr("mon_loading", self.lang, &[]));
            list.list.append(&row);
            return;
        }
        for p in procs {
            let row = adw::ActionRow::new();
            row.set_title(&glib::markup_escape_text(&p.name));
            row.set_subtitle(&format!("PID {} \u{b7} {} \u{b7} {}", p.pid, Fmt::mem(p.mem_kb * 1024, self.lang), Fmt::pct(p.cpu / 100.0)));
            let pid = p.pid;
            let name = p.name.clone();

            if self.termed.borrow().contains(&pid) {
                let force = gtk4::Button::with_label(&tr("mon_force_quit", self.lang, &[]));
                force.add_css_class("destructive-action");
                force.set_valign(gtk4::Align::Center);
                force.set_tooltip_text(Some(&tr("mon_force_quit_name", self.lang, &[&name])));
                force.connect_clicked(move |b| {
                    match monitor::sys::quit_process(pid, true) {
                        Ok(()) => info!("Monitor: force quit sent to PID {pid}"),
                        Err(e) => warn!("Monitor: force quit refused: {e}"),
                    }
                    b.set_sensitive(false);
                });
                row.add_suffix(&force);
            }

            let quit = gtk4::Button::from_icon_name("window-close-symbolic");
            quit.add_css_class("flat");
            quit.set_valign(gtk4::Align::Center);
            quit.set_tooltip_text(Some(&tr("mon_quit_name", self.lang, &[&name])));
            let me = Rc::clone(self);
            quit.connect_clicked(move |b| {
                match monitor::sys::quit_process(pid, false) {
                    Ok(()) => {
                        info!("Monitor: quit (SIGTERM) sent to PID {pid}");
                        me.termed.borrow_mut().insert(pid);
                    }
                    Err(e) => warn!("Monitor: quit refused: {e}"),
                }
                b.set_sensitive(false);
            });
            row.add_suffix(&quit);
            list.list.append(&row);
        }
    }
}

/// The monitor window. It samples every 2 seconds while it is on screen and not at all otherwise.
pub struct MonitorWindow {
    pub window: adw::ApplicationWindow,
    pub widget: Rc<MonitorWidget>,
    sampler: Rc<RefCell<Option<Sampler>>>,
    timer: Rc<RefCell<Option<glib::SourceId>>>,
}

impl MonitorWindow {
    fn build(app: &adw::Application, lang: Language) -> Rc<Self> {
        let window = adw::ApplicationWindow::new(app);
        let title = tr("act_monitor", lang, &[]);
        window.set_title(Some(&title));
        window.set_default_size(540, 760);
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        // paints the window color itself, so a picture of this box (--snapshot) has a background in dark mode too
        content.add_css_class("background");
        content.append(&adw::HeaderBar::new());
        let widget = MonitorWidget::new(lang);
        content.append(&widget.root);
        window.set_content(Some(&content));
        Rc::new(MonitorWindow { window, widget, sampler: Rc::new(RefCell::new(None)), timer: Rc::new(RefCell::new(None)) })
    }

    /// The real window: reads the machine while it is visible.
    pub fn new(app: &adw::Application, lang: Language) -> Rc<Self> {
        let win = Self::build(app, lang);
        let w = Rc::clone(&win);
        win.window.connect_map(move |_| w.start());
        let w = Rc::clone(&win);
        win.window.connect_unmap(move |_| w.stop());
        // closing only hides it, so it can be opened again
        win.window.set_hide_on_close(true);
        win
    }

    /// For `--snapshot`: shows the fake sample and never reads the machine.
    pub fn new_demo(app: &adw::Application, lang: Language) -> Rc<Self> {
        let win = Self::build(app, lang);
        win.widget.update(&Snapshot::demo());
        // the whole sample on one picture: the window takes its natural height instead of scrolling
        win.widget.root.set_propagate_natural_height(true);
        win.widget.root.set_vscrollbar_policy(gtk4::PolicyType::Never);
        win.window.set_default_size(540, 1);
        win
    }

    fn start(self: &Rc<Self>) {
        self.stop();
        info!("Monitor: sampling started");
        *self.sampler.borrow_mut() = Some(Sampler::new(true));
        self.sample();
        // the first rates need two samples: take the second one soon, then every INTERVAL_SECS
        let w = Rc::clone(self);
        glib::timeout_add_local_once(Duration::from_millis(600), move || w.sample());
        let w = Rc::clone(self);
        let id = glib::timeout_add_local(Duration::from_secs(INTERVAL_SECS as u64), move || {
            w.sample();
            glib::ControlFlow::Continue
        });
        *self.timer.borrow_mut() = Some(id);
    }

    fn stop(&self) {
        if let Some(id) = self.timer.borrow_mut().take() {
            id.remove();
            info!("Monitor: sampling stopped");
        }
        *self.sampler.borrow_mut() = None;
    }

    fn sample(self: &Rc<Self>) {
        let snap = match self.sampler.borrow_mut().as_mut() {
            Some(s) => s.tick().clone(),
            None => return,
        };
        self.widget.update(&snap);
    }

    pub fn show(&self) {
        self.window.present();
    }

    /// Draw the window content with a soft shadow into a PNG (`menew --snapshot`).
    pub fn capture_png(&self, path: &std::path::Path) -> Result<(), String> {
        use gtk4::{graphene, gsk};
        let child = self.window.content().ok_or("no content")?;
        let (w, h) = (child.width() as f32, child.height() as f32);
        if w < 1.0 || h < 1.0 {
            return Err("the monitor window has no size yet".into());
        }
        let margin = 32.0_f32;
        let snap = gtk4::Snapshot::new();
        let card = gsk::RoundedRect::from_rect(graphene::Rect::new(margin, margin, w, h), 14.0);
        snap.append_outset_shadow(&card, &gdk::RGBA::new(0.0, 0.0, 0.0, 0.30), 0.0, 8.0, 0.0, 24.0);
        snap.translate(&graphene::Point::new(margin, margin));
        gtk4::WidgetPaintable::new(Some(&child)).snapshot(&snap, w as f64, h as f64);
        let node = snap.to_node().ok_or("nothing was drawn")?;
        let renderer = self.window.native().and_then(|n| n.renderer()).ok_or("no renderer")?;
        let texture = renderer.render_texture(&node, Some(&graphene::Rect::new(0.0, 0.0, w + 2.0 * margin, h + 2.0 * margin)));
        texture.save_to_png(path).map_err(|e| e.to_string())
    }
}
