//! Recent picks (from `Palette.swift`): the last 20 items you ran, most recent first, kept in a
//! local file. They come first in the empty palette and get a bonus in the search score.

use std::path::{Path, PathBuf};

pub const MAX_RECENTS: usize = 20;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recents {
    pub list: Vec<String>,
}

pub fn default_path() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".local/share")).join("menew").join("recents.json")
}

impl Recents {
    pub fn load_from(path: &Path) -> Self {
        let list = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
            .unwrap_or_default();
        Self { list }
    }

    pub fn load() -> Self {
        Self::load_from(&default_path())
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::write(path, serde_json::to_string(&self.list).map_err(std::io::Error::other)?)
    }

    /// Move `id` to the front.
    pub fn remember(&mut self, id: &str) {
        self.list.retain(|x| x != id);
        self.list.insert(0, id.to_string());
        self.list.truncate(MAX_RECENTS);
    }

    /// Score bonus: 60 for the latest, 3 less for each step, never below 0.
    pub fn bonus(&self, id: &str) -> i32 {
        self.list.iter().position(|x| x == id).map(|i| (60 - i as i32 * 3).max(0)).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remember_moves_to_front_and_caps_at_20() {
        let mut r = Recents::default();
        for i in 0..25 {
            r.remember(&format!("a{i}"));
        }
        assert_eq!(r.list.len(), 20);
        assert_eq!(r.list[0], "a24");
        r.remember("a10");
        assert_eq!(r.list[0], "a10");
        assert_eq!(r.list.iter().filter(|x| *x == "a10").count(), 1);
    }

    #[test]
    fn bonus_decays_by_three() {
        let mut r = Recents::default();
        r.remember("b");
        r.remember("a");
        assert_eq!(r.bonus("a"), 60);
        assert_eq!(r.bonus("b"), 57);
        assert_eq!(r.bonus("zzz"), 0);
    }

    #[test]
    fn round_trips_a_file_and_survives_garbage() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub").join("recents.json");
        let mut r = Recents::default();
        r.remember("x");
        r.save_to(&p).unwrap();
        assert_eq!(Recents::load_from(&p), r);
        std::fs::write(&p, "garbage").unwrap();
        assert_eq!(Recents::load_from(&p), Recents::default());
    }
}
