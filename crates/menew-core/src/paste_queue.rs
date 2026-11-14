//! The paste queue (port of `PasteQueue.swift`): turned on, everything that is copied joins the queue
//! and is pasted one by one in the order it was copied.
//!
//! On the Mac the queue listens to every Command+V and moves on after each one. Wayland tells no
//! program when another one pastes, so here the step is explicit: [`PasteQueue::paste_next`], run
//! by the "Paste next" shortcut, the palette action or the tray item, takes the first item out
//! and the caller puts it on the clipboard and presses Ctrl+V for the user when it can.
//! The limits are in `docs/parity.md`.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueEntry {
    /// The id of the entry in the history.
    pub id: i64,
    /// One line of what it holds, for the banner.
    pub preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pasted {
    pub entry: QueueEntry,
    /// How many are left after this one. 0 means the queue ended and turned itself off.
    pub remaining: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PasteQueue {
    on: bool,
    items: Vec<QueueEntry>,
}

impl PasteQueue {
    pub fn is_on(&self) -> bool {
        self.on
    }

    pub fn items(&self) -> &[QueueEntry] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The next to be pasted.
    pub fn head(&self) -> Option<&QueueEntry> {
        self.items.first()
    }

    /// Turn on with an empty queue. False when it already was on.
    pub fn start(&mut self) -> bool {
        if self.on {
            return false;
        }
        self.on = true;
        self.items.clear();
        true
    }

    /// Turn off and forget the items. False when it already was off.
    pub fn stop(&mut self) -> bool {
        if !self.on {
            return false;
        }
        self.on = false;
        self.items.clear();
        true
    }

    pub fn toggle(&mut self) {
        if self.on {
            self.stop();
        } else {
            self.start();
        }
    }

    /// Something new was copied: joins the queue when it is on. Returns the new length.
    pub fn captured(&mut self, e: QueueEntry) -> Option<usize> {
        if !self.on {
            return None;
        }
        self.items.push(e);
        Some(self.items.len())
    }

    /// "Add to paste queue" of an item of the history: turns the queue on when needed. Returns the new length.
    pub fn add(&mut self, e: QueueEntry) -> usize {
        self.start();
        self.items.push(e);
        self.items.len()
    }

    /// Take an item out. Turns the queue off when it was the last one. False when it was not there.
    pub fn remove(&mut self, id: i64) -> bool {
        let Some(i) = self.items.iter().position(|e| e.id == id) else { return false };
        self.items.remove(i);
        if self.items.is_empty() {
            self.stop();
        }
        true
    }

    /// The step of the queue: the first item leaves. When it was the last one the queue turns off.
    pub fn paste_next(&mut self) -> Option<Pasted> {
        if !self.on || self.items.is_empty() {
            return None;
        }
        let entry = self.items.remove(0);
        let remaining = self.items.len();
        if remaining == 0 {
            self.stop();
        }
        Some(Pasted { entry, remaining })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(id: i64) -> QueueEntry {
        QueueEntry { id, preview: format!("item {id}") }
    }

    #[test]
    fn copies_join_only_when_on() {
        let mut q = PasteQueue::default();
        assert_eq!(q.captured(e(1)), None);
        assert!(q.start());
        assert!(!q.start());
        assert_eq!(q.captured(e(1)), Some(1));
        assert_eq!(q.captured(e(2)), Some(2));
        assert_eq!(q.head().unwrap().id, 1);
    }

    #[test]
    fn next_goes_in_copy_order_and_ends_the_queue() {
        let mut q = PasteQueue::default();
        q.start();
        q.captured(e(1));
        q.captured(e(2));
        assert_eq!(q.paste_next().unwrap(), Pasted { entry: e(1), remaining: 1 });
        assert!(q.is_on());
        assert_eq!(q.paste_next().unwrap(), Pasted { entry: e(2), remaining: 0 });
        assert!(!q.is_on());
        assert_eq!(q.paste_next(), None);
    }

    #[test]
    fn add_starts_the_queue_and_remove_of_the_last_stops_it() {
        let mut q = PasteQueue::default();
        assert_eq!(q.add(e(5)), 1);
        assert!(q.is_on());
        assert!(!q.remove(9));
        assert!(q.remove(5));
        assert!(!q.is_on());
    }

    #[test]
    fn stop_forgets_the_items() {
        let mut q = PasteQueue::default();
        q.start();
        q.captured(e(1));
        assert!(q.stop());
        assert!(q.is_empty());
        assert!(!q.stop());
        q.toggle();
        assert!(q.is_on() && q.is_empty());
    }
}
