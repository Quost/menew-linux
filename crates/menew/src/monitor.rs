//! Samples the system every 2 s for the tray metrics, the Performance section and the popover.

use crate::commands::Commands;
use gtk4::glib;
use menew_core::palette::{Icon, ItemAction, Kind, PaletteItem};
use menew_core::strings::tr;
use menew_core::sysmon::{self, Monitor, Snapshot};
use menew_core::{current_language, Config};
use menew_ui::monitor::MonitorView;
use std::cell::RefCell;
use std::rc::Rc;

/// Called after each sample (the popover's Now tab).
pub type SampleListener = Rc<dyn Fn(&Snapshot)>;

pub struct Sampler {
    monitor: RefCell<Monitor>,
    pub last: RefCell<Snapshot>,
    pub view: Rc<MonitorView>,
    metrics: RefCell<Vec<String>>,
    /// The tray text for the chosen metrics, read by the daemon's status push.
    pub tray_text: Rc<RefCell<String>>,
    /// Whether to read the processes too (the Performance section or the popover is open).
    pub detail_check: RefCell<Option<Rc<dyn Fn() -> bool>>>,
    listeners: RefCell<Vec<SampleListener>>,
}

fn secs() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

impl Sampler {
    pub fn new(cmds: &Rc<Commands>, push: Rc<dyn Fn()>, tray_text: Rc<RefCell<String>>) -> Rc<Self> {
        let cfg = Config::load();
        let metrics_cell: Rc<RefCell<Option<Rc<Sampler>>>> = Rc::new(RefCell::new(None));
        let mc = Rc::clone(&metrics_cell);
        let push2 = Rc::clone(&push);
        let view = MonitorView::new(
            Rc::new(|pid| {
                menew_core::sys::signal(pid, 15);
            }),
            cfg.tray_metrics.clone(),
            Rc::new(move |m: Vec<String>| {
                let mut c = Config::load();
                c.tray_metrics = m.clone();
                let _ = c.save_to(&Config::default_path());
                if let Some(s) = mc.borrow().as_ref() {
                    *s.metrics.borrow_mut() = m;
                    s.update_tray();
                    push2();
                }
            }),
        );
        let me = Rc::new(Self {
            monitor: RefCell::new(Monitor::default()),
            last: RefCell::new(Snapshot::default()),
            view,
            metrics: RefCell::new(cfg.tray_metrics),
            tray_text,
            detail_check: RefCell::new(None),
            listeners: RefCell::new(vec![]),
        });
        *metrics_cell.borrow_mut() = Some(Rc::clone(&me));
        me.sample();
        let w = Rc::downgrade(&me);
        glib::timeout_add_seconds_local(2, move || {
            let Some(s) = w.upgrade() else { return glib::ControlFlow::Break };
            s.sample();
            push();
            glib::ControlFlow::Continue
        });
        cmds.add_provider("always", Rc::new(|_raw, lang| {
            vec![PaletteItem::new("action.performance", Kind::Action, &tr("mon_open", lang, &[]))
                .subtitle(&tr("mon_open_sub", lang, &[]))
                .kw("desempenho performance cpu memória memory ram processos processes monitor sistema system leistung rendimiento 性能 task manager")
                .icon(Icon::Symbol("computer-symbolic"))
                .act(ItemAction::Command("open.performance".into()))]
        }));
        me
    }

    fn update_tray(&self) {
        let lang = current_language();
        *self.tray_text.borrow_mut() = sysmon::tray_text(&self.last.borrow(), &self.metrics.borrow(), lang);
    }

    pub fn sample(&self) {
        let detailed = self.detail_check.borrow().as_ref().map(|f| f()).unwrap_or(false);
        let snap = self.monitor.borrow_mut().sample(secs(), detailed);
        *self.last.borrow_mut() = snap;
        self.update_tray();
        let s = self.last.borrow().clone();
        if detailed {
            self.view.update(&s);
        }
        let ls = self.listeners.borrow().clone();
        for l in ls {
            l(&s);
        }
    }
}
