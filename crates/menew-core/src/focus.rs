//! Focus timer (port of `Focus.swift`): 25 minutes by default, "focus 50" for another length,
//! +5 minutes, stop, and a 5 minute break at the end. Pure state: the daemon ticks it and shows the
//! time left in the tray and the notification at the end.
//!
//! Like the Mac, a focus can also mute the microphone, keep the computer awake and pause the
//! clipboard while it runs: the daemon does those through the [`FocusHooks`] trait
//! (`crates/menew/src/focus_hooks.rs`), following the Settings.

use crate::i18n::Language;
use crate::strings::tr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Focus,
    Rest,
}

/// What a tick found out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusEvent {
    /// The focus ended; a break of `rest_minutes` started (0 when breaks are off).
    FocusFinished { rest_minutes: u32 },
    BreakFinished,
}

/// Things a focus turns on when it starts and turns off when it ends (Phase 5 and 9).
pub trait FocusHooks {
    /// Mute the microphone, keep awake for `minutes`, pause the clipboard for `minutes`...
    fn started(&mut self, minutes: u32);
    /// Undo only what `started` changed.
    fn ended(&mut self);
}

/// Does nothing (tests, and before the daemon sets its own hooks).
pub struct NoHooks;

impl FocusHooks for NoHooks {
    fn started(&mut self, _minutes: u32) {}
    fn ended(&mut self) {}
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FocusTimer {
    pub phase: Option<Phase>,
    pub started_at: f64,
    pub ends_at: f64,
    /// A 5 minute break after the focus (on by default, like the Mac).
    pub breaks: bool,
}

impl Default for FocusTimer {
    fn default() -> Self {
        Self { phase: None, started_at: 0.0, ends_at: 0.0, breaks: true }
    }
}

pub const DEFAULT_MINUTES: u32 = 25;
pub const REST_MINUTES: u32 = 5;
pub const EXTEND_MINUTES: u32 = 5;

impl FocusTimer {
    pub fn is_running(&self) -> bool {
        self.phase.is_some()
    }

    pub fn start(&mut self, minutes: u32, now: f64, hooks: &mut dyn FocusHooks) {
        self.stop(hooks);
        hooks.started(minutes);
        self.run(Phase::Focus, minutes, now);
    }

    fn run(&mut self, phase: Phase, minutes: u32, now: f64) {
        self.phase = Some(phase);
        self.started_at = now;
        self.ends_at = now + minutes as f64 * 60.0;
    }

    /// +5 minutes on the running phase.
    pub fn extend(&mut self, minutes: u32, now: f64) {
        if self.is_running() {
            self.ends_at = self.ends_at.max(now) + minutes as f64 * 60.0;
        }
    }

    /// Stop before time: undoes what the focus turned on, no break and no notification.
    pub fn stop(&mut self, hooks: &mut dyn FocusHooks) -> bool {
        if !self.is_running() {
            return false;
        }
        hooks.ended();
        self.phase = None;
        true
    }

    /// Call about once a second.
    pub fn tick(&mut self, now: f64, hooks: &mut dyn FocusHooks) -> Option<FocusEvent> {
        let phase = self.phase?;
        if now < self.ends_at {
            return None;
        }
        match phase {
            Phase::Focus => {
                hooks.ended();
                if self.breaks {
                    self.run(Phase::Rest, REST_MINUTES, now);
                    Some(FocusEvent::FocusFinished { rest_minutes: REST_MINUTES })
                } else {
                    self.phase = None;
                    Some(FocusEvent::FocusFinished { rest_minutes: 0 })
                }
            }
            Phase::Rest => {
                self.phase = None;
                Some(FocusEvent::BreakFinished)
            }
        }
    }

    /// The time left for the tray: ("FOCUS", "24m"); minutes, and seconds in the last minute.
    pub fn segment(&self, now: f64, lang: Language) -> Option<(String, String)> {
        let phase = self.phase?;
        let left = (self.ends_at - now).ceil().max(0.0) as i64;
        let value = if left >= 60 { format!("{}m", (left + 59) / 60) } else { format!("{left}s") };
        Some((tr(if phase == Phase::Focus { "seg_focus" } else { "seg_break" }, lang, &[]), value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Spy {
        started: Vec<u32>,
        ended: u32,
    }
    impl FocusHooks for Spy {
        fn started(&mut self, m: u32) {
            self.started.push(m);
        }
        fn ended(&mut self) {
            self.ended += 1;
        }
    }

    #[test]
    fn default_flow_focus_then_break() {
        let mut f = FocusTimer::default();
        let mut spy = Spy::default();
        f.start(25, 1000.0, &mut spy);
        assert_eq!(spy.started, vec![25]);
        assert_eq!(f.ends_at, 1000.0 + 1500.0);
        assert_eq!(f.tick(1000.0 + 1499.0, &mut spy), None);
        assert_eq!(f.tick(1000.0 + 1500.0, &mut spy), Some(FocusEvent::FocusFinished { rest_minutes: 5 }));
        assert_eq!(f.phase, Some(Phase::Rest));
        assert_eq!(spy.ended, 1);
        assert_eq!(f.tick(1000.0 + 1500.0 + 300.0, &mut spy), Some(FocusEvent::BreakFinished));
        assert!(!f.is_running());
    }

    #[test]
    fn extend_and_stop() {
        let mut f = FocusTimer::default();
        let mut spy = Spy::default();
        f.start(50, 0.0, &mut spy);
        f.extend(EXTEND_MINUTES, 10.0);
        assert_eq!(f.ends_at, 3000.0 + 300.0);
        assert!(f.stop(&mut spy));
        assert!(!f.stop(&mut spy));
        assert_eq!(spy.ended, 1);
    }

    #[test]
    fn no_break_when_off() {
        let mut f = FocusTimer { breaks: false, ..Default::default() };
        f.start(1, 0.0, &mut NoHooks);
        assert_eq!(f.tick(60.0, &mut NoHooks), Some(FocusEvent::FocusFinished { rest_minutes: 0 }));
        assert!(!f.is_running());
    }

    #[test]
    fn segment_text() {
        let mut f = FocusTimer::default();
        f.start(25, 0.0, &mut NoHooks);
        assert_eq!(f.segment(0.0, Language::En), Some(("FOCUS".into(), "25m".into())));
        assert_eq!(f.segment(1500.0 - 30.0, Language::En), Some(("FOCUS".into(), "30s".into())));
    }
}
