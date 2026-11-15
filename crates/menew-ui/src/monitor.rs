//! The Performance section (port of `MonitorViews.swift`): CPU and memory with a live graph, GPU,
//! disk space and speed, network, battery, uptime and the apps using the most, with Quit.

use crate::window;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use menew_core::strings::tr;
use menew_core::sysmon::{self, Process, Snapshot};
use menew_core::{current_language, Language};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

const HISTORY: usize = 60;

struct Card {
    value: gtk4::Label,
    detail: gtk4::Label,
    bar: gtk4::LevelBar,
    graph: Option<(gtk4::DrawingArea, Rc<RefCell<VecDeque<f64>>>)>,
    root: gtk4::Box,
}

fn card(title: &str, icon: &str, with_graph: bool) -> Card {
    let root = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(6).css_classes(["card", "menew-mon-card"]).build();
    let inner = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(6).margin_top(12).margin_bottom(12).margin_start(14).margin_end(14).build();
    let top = gtk4::Box::builder().spacing(8).build();
    top.append(&gtk4::Image::from_icon_name(icon));
    top.append(&gtk4::Label::builder().label(title).xalign(0.0).hexpand(true).css_classes(["heading"]).build());
    let value = gtk4::Label::builder().xalign(1.0).css_classes(["title-3", "numeric"]).build();
    top.append(&value);
    inner.append(&top);
    let bar = gtk4::LevelBar::builder().min_value(0.0).max_value(100.0).build();
    inner.append(&bar);
    let graph = with_graph.then(|| {
        let data = Rc::new(RefCell::new(VecDeque::<f64>::with_capacity(HISTORY)));
        let area = gtk4::DrawingArea::builder().content_height(48).hexpand(true).build();
        let d = Rc::clone(&data);
        area.set_draw_func(move |w, cr, width, height| {
            let pts = d.borrow();
            if pts.len() < 2 {
                return;
            }
            let color = w.color();
            let (wf, hf) = (width as f64, height as f64);
            let step = wf / (HISTORY - 1) as f64;
            let x0 = wf - step * (pts.len() - 1) as f64;
            cr.move_to(x0, hf);
            for (i, v) in pts.iter().enumerate() {
                cr.line_to(x0 + step * i as f64, hf - (*v / 100.0_f64).clamp(0.0, 1.0) * (hf - 2.0));
            }
            cr.line_to(wf, hf);
            cr.close_path();
            cr.set_source_rgba(color.red() as f64, color.green() as f64, color.blue() as f64, 0.18);
            let _ = cr.fill_preserve();
            cr.set_source_rgba(color.red() as f64, color.green() as f64, color.blue() as f64, 0.7);
            cr.set_line_width(1.5);
            let _ = cr.stroke();
        });
        inner.append(&area);
        (area, data)
    });
    let detail = gtk4::Label::builder().xalign(0.0).wrap(true).css_classes(["dim-label", "caption"]).build();
    inner.append(&detail);
    root.append(&inner);
    Card { value, detail, bar, graph, root }
}

impl Card {
    /// Hide or show the card (its FlowBox cell, so no hole is left).
    fn show(&self, on: bool) {
        match self.root.parent() {
            Some(cell) => cell.set_visible(on),
            None => self.root.set_visible(on),
        }
    }

    fn set(&self, value: &str, percent: Option<f64>, detail: &str) {
        self.value.set_label(value);
        self.bar.set_visible(percent.is_some());
        if let Some(p) = percent {
            self.bar.set_value(p.clamp(0.0, 100.0));
            if let Some((area, data)) = &self.graph {
                let mut d = data.borrow_mut();
                if d.len() == HISTORY {
                    d.pop_front();
                }
                d.push_back(p);
                drop(d);
                area.queue_draw();
            }
        }
        self.detail.set_label(detail);
        self.detail.set_visible(!detail.is_empty());
    }
}

pub struct MonitorView {
    pub root: gtk4::Widget,
    status: gtk4::Label,
    cpu: Card,
    mem: Card,
    gpu: Card,
    disk: Card,
    net: Card,
    battery: Card,
    system: Card,
    top_cpu: gtk4::ListBox,
    top_mem: gtk4::ListBox,
    on_quit: Rc<dyn Fn(u32)>,
}

impl MonitorView {
    /// `on_quit(pid)` quits a process; `on_metrics` saves the metrics chosen for the tray.
    pub fn new(on_quit: Rc<dyn Fn(u32)>, metrics: Vec<String>, on_metrics: Rc<dyn Fn(Vec<String>)>) -> Rc<Self> {
        let lang = current_language();
        let (scroll, body) = window::page();
        let status = gtk4::Label::builder().xalign(0.0).css_classes(["title-4"]).build();
        body.append(&status);
        let grid = gtk4::FlowBox::builder().selection_mode(gtk4::SelectionMode::None).min_children_per_line(1).max_children_per_line(3).column_spacing(12).row_spacing(12).homogeneous(true).build();
        let cpu = card(&tr("mon_cpu", lang, &[]), "computer-symbolic", true);
        let mem = card(&tr("mon_memory", lang, &[]), "drive-harddisk-solidstate-symbolic", true);
        let gpu = card(&tr("mon_gpu", lang, &[]), "video-display-symbolic", true);
        let disk = card(&tr("mon_disk", lang, &[]), "drive-harddisk-symbolic", false);
        let net = card(&tr("mon_network", lang, &[]), "network-transmit-receive-symbolic", false);
        let battery = card(&tr("mon_battery", lang, &[]), "battery-good-symbolic", false);
        let system = card(&tr("mon_uptime", lang, &[]), "preferences-system-time-symbolic", false);
        for c in [&cpu, &mem, &gpu, &disk, &net, &battery, &system] {
            grid.append(&c.root);
        }
        body.append(&grid);

        let lists = gtk4::Box::builder().spacing(12).homogeneous(true).build();
        let mut boxes = vec![];
        for key in ["mon_top_cpu", "mon_top_memory"] {
            let g = adw::PreferencesGroup::builder().title(tr(key, lang, &[])).build();
            let lb = gtk4::ListBox::builder().css_classes(["boxed-list"]).selection_mode(gtk4::SelectionMode::None).build();
            g.add(&lb);
            lists.append(&g);
            boxes.push(lb);
        }
        body.append(&lists);

        let mg = adw::PreferencesGroup::builder().title(tr("mon_tray_metrics", lang, &[])).description(tr("mon_tray_metrics_sub", lang, &[])).build();
        let chosen = Rc::new(RefCell::new(metrics));
        for m in sysmon::TRAY_METRICS {
            let label = match m {
                "cpu" => tr("mon_cpu", lang, &[]),
                "memory" => tr("mon_memory", lang, &[]),
                "gpu" => tr("mon_gpu", lang, &[]),
                "disk" => tr("mon_disk", lang, &[]),
                "network" => tr("mon_network", lang, &[]),
                _ => tr("mon_battery", lang, &[]),
            };
            let sw = adw::SwitchRow::builder().title(label).active(chosen.borrow().iter().any(|x| x == m)).build();
            let (c, f) = (Rc::clone(&chosen), Rc::clone(&on_metrics));
            sw.connect_active_notify(move |s| {
                let mut v = c.borrow_mut();
                v.retain(|x| x != m);
                if s.is_active() {
                    v.push(m.to_string());
                }
                // keep the order of the list
                v.sort_by_key(|x| sysmon::TRAY_METRICS.iter().position(|t| t == x));
                f(v.clone());
            });
            mg.add(&sw);
        }
        body.append(&mg);
        let top_mem = boxes.pop().unwrap();
        let top_cpu = boxes.pop().unwrap();
        Rc::new(Self { root: scroll.upcast(), status, cpu, mem, gpu, disk, net, battery, system, top_cpu, top_mem, on_quit })
    }

    fn fill(&self, lb: &gtk4::ListBox, list: &[Process], cpu: bool, lang: Language) {
        while let Some(c) = lb.first_child() {
            lb.remove(&c);
        }
        for p in list {
            let sub = if cpu { format!("{:.0}%", p.cpu) } else { sysmon::bytes(p.memory, lang) };
            let row = adw::ActionRow::builder().title(glib::markup_escape_text(&p.name)).subtitle(sub).build();
            let b = gtk4::Button::builder().label(tr("mon_quit_process", lang, &[])).valign(gtk4::Align::Center).css_classes(["flat"]).build();
            let (f, pid) = (Rc::clone(&self.on_quit), p.pid);
            b.connect_clicked(move |_| f(pid));
            row.add_suffix(&b);
            lb.append(&row);
        }
    }

    pub fn update(&self, s: &Snapshot) {
        let lang = current_language();
        let warn = sysmon::attention(s, lang);
        self.status.set_label(&if warn.is_empty() { tr("mon_all_good", lang, &[]) } else { warn.join(" · ") });
        let load = s.load.map(|l| format!("{} {:.2} {:.2} {:.2}", tr("mon_load", lang, &[]), l.0, l.1, l.2)).unwrap_or_default();
        self.cpu.set(&format!("{:.0}%", s.cpu), Some(s.cpu), &format!("{} cores · {load}", s.cores));
        let m = &s.memory;
        let swap = if m.swap_total > 0 { format!(" · {} {}", tr("mon_swap", lang, &[]), tr("mon_used_of", lang, &[&sysmon::bytes(m.swap_used(), lang), &sysmon::bytes(m.swap_total, lang)])) } else { String::new() };
        self.mem.set(&format!("{:.0}%", m.used_percent()), Some(m.used_percent()), &(tr("mon_used_of", lang, &[&sysmon::bytes(m.used(), lang), &sysmon::bytes(m.total, lang)]) + &swap));
        self.gpu.show(s.gpu.is_some());
        if let Some(g) = s.gpu {
            self.gpu.set(&format!("{g}%"), Some(g as f64), "");
        }
        match &s.disk {
            Some(d) => self.disk.set(
                &tr("mon_free", lang, &[&sysmon::bytes(d.free, lang)]),
                Some(d.used_percent()),
                &format!("{} · {}", tr("mon_used_of", lang, &[&sysmon::bytes(d.total - d.free, lang), &sysmon::bytes(d.total, lang)]), tr("mon_read_write", lang, &[&sysmon::speed(s.disk_read, lang), &sysmon::speed(s.disk_write, lang)])),
            ),
            None => self.disk.show(false),
        }
        self.net.set(&format!("↓ {}", sysmon::speed(s.net_down, lang)), None, &format!("↑ {}", sysmon::speed(s.net_up, lang)));
        self.battery.show(s.battery.is_some());
        if let Some(b) = &s.battery {
            let mut parts = vec![tr(if b.charging() { "mon_charging" } else { "mon_on_battery" }, lang, &[])];
            if let Some(h) = b.health {
                parts.push(tr("mon_health", lang, &[&h.to_string()]));
            }
            if let Some(c) = b.cycles {
                parts.push(tr("mon_cycles", lang, &[&c.to_string()]));
            }
            if let Some(w) = b.power {
                parts.push(tr("mon_watts", lang, &[&format!("{w:.1}")]));
            }
            self.battery.set(&format!("{}%", b.percent), Some(b.percent as f64), &parts.join(" · "));
        }
        self.system.set(&sysmon::uptime_text(s.uptime), None, menew_core::sys::Desktop::detect().name());
        self.fill(&self.top_cpu, &s.top_cpu, true, lang);
        self.fill(&self.top_mem, &s.top_memory, false, lang);
    }
}
