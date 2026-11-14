//! Virtual paste: inject Ctrl+V through `/dev/uinput`.
//!
//! The key sequence and the decision logic are pure and unit tested. The device
//! handling uses raw `libc` ioctls (no evdev/uinput crate) and is Linux only.

use crate::config::PasteMode;

// linux/input-event-codes.h
pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const SYN_REPORT: u16 = 0;
pub const KEY_LEFTCTRL: u16 = 29;
pub const KEY_V: u16 = 47;
pub const KEY_LEFT: u16 = 105;

pub const KEY_RELEASE: i32 = 0;
pub const KEY_PRESS: i32 = 1;

/// One input event, as written to uinput (without the timestamp, which the kernel fills in).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub kind: u16,
    pub code: u16,
    pub value: i32,
}

const fn ev(kind: u16, code: u16, value: i32) -> KeyEvent {
    KeyEvent { kind, code, value }
}

/// The exact event sequence for "press Ctrl+V": Ctrl down, V down, sync, V up, Ctrl up, sync.
pub fn ctrl_v_sequence() -> Vec<KeyEvent> {
    vec![
        ev(EV_KEY, KEY_LEFTCTRL, KEY_PRESS),
        ev(EV_KEY, KEY_V, KEY_PRESS),
        ev(EV_SYN, SYN_REPORT, 0),
        ev(EV_KEY, KEY_V, KEY_RELEASE),
        ev(EV_KEY, KEY_LEFTCTRL, KEY_RELEASE),
        ev(EV_SYN, SYN_REPORT, 0),
    ]
}

/// `count` presses of the left arrow (the cursor goes back after a snippet with `{cursor}`); at most 2000, like the Mac.
pub fn left_sequence(count: usize) -> Vec<KeyEvent> {
    let mut out = vec![];
    for _ in 0..count.min(2000) {
        out.push(ev(EV_KEY, KEY_LEFT, KEY_PRESS));
        out.push(ev(EV_SYN, SYN_REPORT, 0));
        out.push(ev(EV_KEY, KEY_LEFT, KEY_RELEASE));
        out.push(ev(EV_SYN, SYN_REPORT, 0));
    }
    out
}

/// What to do after an entry was copied and the palette hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteAction {
    /// Wait for focus to return, then inject Ctrl+V.
    InjectCtrlV,
    /// Only copy and show the "Copied. Press Ctrl+V to paste" notification.
    NotifyOnly,
}

pub fn decide_paste(mode: PasteMode, uinput_writable: bool) -> PasteAction {
    match (mode, uinput_writable) {
        (PasteMode::Auto, true) => PasteAction::InjectCtrlV,
        _ => PasteAction::NotifyOnly,
    }
}

/// Delay between hiding the palette and injecting the keys, so focus returns to the previous window.
pub const FOCUS_DELAY_MS: u64 = 150;

#[cfg(target_os = "linux")]
pub use linux::{inject_ctrl_v, inject_left, uinput_writable};

#[cfg(not(target_os = "linux"))]
pub fn uinput_writable() -> bool {
    false
}

#[cfg(not(target_os = "linux"))]
pub fn inject_ctrl_v() -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "uinput is only available on Linux",
    ))
}

#[cfg(not(target_os = "linux"))]
pub fn inject_left(_count: usize) -> std::io::Result<()> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "uinput is only available on Linux"))
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::fs::{File, OpenOptions};
    use std::io::{Error, Result, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;

    const UINPUT_PATH: &str = "/dev/uinput";

    // ioctl request numbers (linux/uinput.h), built with the generic Linux _IOC encoding
    const fn ioc(dir: u32, ty: u8, nr: u8, size: usize) -> u32 {
        (dir << 30) | ((size as u32) << 16) | ((ty as u32) << 8) | nr as u32
    }
    const IOC_NONE: u32 = 0;
    const IOC_WRITE: u32 = 1;
    const UI_DEV_CREATE: u32 = ioc(IOC_NONE, b'U', 1, 0);
    const UI_DEV_DESTROY: u32 = ioc(IOC_NONE, b'U', 2, 0);
    const UI_DEV_SETUP: u32 = ioc(IOC_WRITE, b'U', 3, std::mem::size_of::<UinputSetup>());
    const UI_SET_EVBIT: u32 = ioc(IOC_WRITE, b'U', 100, std::mem::size_of::<libc::c_int>());
    const UI_SET_KEYBIT: u32 = ioc(IOC_WRITE, b'U', 101, std::mem::size_of::<libc::c_int>());

    #[repr(C)]
    struct InputId {
        bustype: u16,
        vendor: u16,
        product: u16,
        version: u16,
    }

    #[repr(C)]
    struct UinputSetup {
        id: InputId,
        name: [u8; 80],
        ff_effects_max: u32,
    }

    #[repr(C)]
    struct InputEvent {
        tv_sec: libc::time_t,
        tv_usec: libc::suseconds_t,
        kind: u16,
        code: u16,
        value: i32,
    }

    /// True when `/dev/uinput` exists and the current user may open it for writing.
    pub fn uinput_writable() -> bool {
        OpenOptions::new().write(true).open(UINPUT_PATH).is_ok()
    }

    fn ioctl(file: &File, request: u32, arg: libc::c_int) -> Result<()> {
        // SAFETY: plain integer argument ioctl on an fd we own.
        let r = unsafe { libc::ioctl(file.as_raw_fd(), request as _, arg) };
        if r < 0 {
            Err(Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn write_event(file: &mut File, e: &KeyEvent) -> Result<()> {
        let raw = InputEvent {
            tv_sec: 0,
            tv_usec: 0,
            kind: e.kind,
            code: e.code,
            value: e.value,
        };
        // SAFETY: InputEvent is repr(C) plain data; we view it as bytes for the write.
        let bytes = unsafe {
            std::slice::from_raw_parts(
                &raw as *const InputEvent as *const u8,
                std::mem::size_of::<InputEvent>(),
            )
        };
        file.write_all(bytes)
    }

    /// Create a temporary virtual keyboard, press Ctrl+V and destroy it.
    /// Blocking: call from a worker thread or after the focus delay.
    pub fn inject_ctrl_v() -> Result<()> {
        inject(&[KEY_LEFTCTRL, KEY_V], &ctrl_v_sequence())
    }

    /// Press the left arrow `count` times (the cursor goes back after pasting a snippet with `{cursor}`).
    pub fn inject_left(count: usize) -> Result<()> {
        if count == 0 {
            return Ok(());
        }
        inject(&[KEY_LEFT], &left_sequence(count))
    }

    /// Create a temporary virtual keyboard with these keys, play the events and destroy it.
    fn inject(keys: &[u16], events: &[KeyEvent]) -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(UINPUT_PATH)?;

        ioctl(&file, UI_SET_EVBIT, EV_KEY as libc::c_int)?;
        for k in keys {
            ioctl(&file, UI_SET_KEYBIT, *k as libc::c_int)?;
        }

        let mut name = [0u8; 80];
        let label = b"menew virtual keyboard";
        name[..label.len()].copy_from_slice(label);
        let setup = UinputSetup {
            id: InputId { bustype: 0x06 /* BUS_VIRTUAL */, vendor: 0x6d65, product: 0x6e77, version: 1 },
            name,
            ff_effects_max: 0,
        };
        // SAFETY: UI_DEV_SETUP takes a pointer to a uinput_setup struct, which `setup` mirrors.
        let r = unsafe {
            libc::ioctl(file.as_raw_fd(), UI_DEV_SETUP as _, &setup as *const UinputSetup)
        };
        if r < 0 {
            return Err(Error::last_os_error());
        }
        ioctl(&file, UI_DEV_CREATE, 0)?;

        // let the compositor notice the new device before the first key
        std::thread::sleep(std::time::Duration::from_millis(250));
        let mut result = Ok(());
        for e in events {
            if let Err(err) = write_event(&mut file, e) {
                result = Err(err);
                break;
            }
            if e.kind == EV_KEY {
                std::thread::sleep(std::time::Duration::from_millis(8));
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        let _ = ioctl(&file, UI_DEV_DESTROY, 0);
        result
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn ioctl_numbers_match_linux_uinput_h() {
            assert_eq!(std::mem::size_of::<UinputSetup>(), 92);
            assert_eq!(UI_DEV_CREATE, 0x5501);
            assert_eq!(UI_DEV_DESTROY, 0x5502);
            assert_eq!(UI_DEV_SETUP, 0x405c_5503);
            assert_eq!(UI_SET_EVBIT, 0x4004_5564);
            assert_eq!(UI_SET_KEYBIT, 0x4004_5565);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_v_sequence_is_press_ctrl_press_v_release_v_release_ctrl() {
        let seq = ctrl_v_sequence();
        assert_eq!(
            seq,
            vec![
                KeyEvent { kind: EV_KEY, code: KEY_LEFTCTRL, value: 1 },
                KeyEvent { kind: EV_KEY, code: KEY_V, value: 1 },
                KeyEvent { kind: EV_SYN, code: SYN_REPORT, value: 0 },
                KeyEvent { kind: EV_KEY, code: KEY_V, value: 0 },
                KeyEvent { kind: EV_KEY, code: KEY_LEFTCTRL, value: 0 },
                KeyEvent { kind: EV_SYN, code: SYN_REPORT, value: 0 },
            ]
        );
    }

    #[test]
    fn every_press_is_released_and_reports_are_synced() {
        let seq = ctrl_v_sequence();
        for code in [KEY_LEFTCTRL, KEY_V] {
            let presses = seq.iter().filter(|e| e.kind == EV_KEY && e.code == code && e.value == 1).count();
            let releases = seq.iter().filter(|e| e.kind == EV_KEY && e.code == code && e.value == 0).count();
            assert_eq!((presses, releases), (1, 1));
        }
        assert_eq!(seq.last().unwrap().kind, EV_SYN);
    }

    #[test]
    fn auto_injects_only_when_uinput_is_writable() {
        assert_eq!(decide_paste(PasteMode::Auto, true), PasteAction::InjectCtrlV);
        assert_eq!(decide_paste(PasteMode::Auto, false), PasteAction::NotifyOnly);
    }


    #[test]
    fn left_sequence_presses_and_releases_each_time_and_is_capped() {
        let seq = left_sequence(3);
        assert_eq!(seq.iter().filter(|e| e.kind == EV_KEY && e.value == 1).count(), 3);
        assert_eq!(seq.iter().filter(|e| e.kind == EV_KEY && e.value == 0).count(), 3);
        assert_eq!(left_sequence(5000).iter().filter(|e| e.kind == EV_KEY && e.value == 1).count(), 2000);
        assert!(left_sequence(0).is_empty());
    }
    #[test]
    fn copy_only_never_injects() {
        assert_eq!(decide_paste(PasteMode::CopyOnly, true), PasteAction::NotifyOnly);
        assert_eq!(decide_paste(PasteMode::CopyOnly, false), PasteAction::NotifyOnly);
    }
}
