//! What a focus session turns on and back off (`Focus.swift`): mute the microphone, keep the
//! computer awake and pause the clipboard, as chosen in Settings. Only what the focus changed is
//! undone at the end.

use crate::commands::Commands;
use menew_core::focus::FocusHooks;
use menew_core::quick_actions as qa;
use menew_core::Config;
use std::rc::Rc;

pub struct DaemonFocusHooks {
    pub cmds: Rc<Commands>,
    pub pause: Rc<dyn Fn(Option<u32>)>,
    pub resume: Rc<dyn Fn()>,
    pub is_paused: Rc<dyn Fn() -> bool>,
    muted: bool,
    awake: bool,
    paused: bool,
}

impl DaemonFocusHooks {
    pub fn new(cmds: Rc<Commands>, pause: Rc<dyn Fn(Option<u32>)>, resume: Rc<dyn Fn()>, is_paused: Rc<dyn Fn() -> bool>) -> Self {
        Self { cmds, pause, resume, is_paused, muted: false, awake: false, paused: false }
    }
}

fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

impl FocusHooks for DaemonFocusHooks {
    fn started(&mut self, minutes: u32) {
        let cfg = Config::load();
        if cfg.focus_mute_mic && qa::is_muted(true) == Some(false) {
            self.muted = qa::toggle_mute(true) == Some(true);
        }
        if cfg.focus_keep_awake && !self.cmds.awake.borrow_mut().is_on() {
            self.awake = self.cmds.awake.borrow_mut().start(Some(minutes), now(), "Focus");
        }
        if cfg.focus_pause_clipboard && cfg.module_clipboard && !(self.is_paused)() {
            (self.pause)(Some(minutes));
            self.paused = true;
        }
    }

    fn ended(&mut self) {
        if std::mem::take(&mut self.muted) && qa::is_muted(true) == Some(true) {
            qa::toggle_mute(true);
        }
        if std::mem::take(&mut self.awake) {
            self.cmds.awake.borrow_mut().stop();
        }
        if std::mem::take(&mut self.paused) {
            (self.resume)();
        }
    }
}
