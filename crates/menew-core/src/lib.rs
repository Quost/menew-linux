pub mod autostart;
pub mod calculator;
pub mod clipboard_db;
pub mod clipboard_view;
pub mod config;
pub mod conversions;
pub mod currency;
pub mod desktop_entry;
pub mod dev_tools;
pub mod emoji;
mod emoji_data;
pub mod focus;
pub mod format;
pub mod i18n;
pub mod palette;
pub mod paste;
pub mod quick_links;
pub mod recents;
pub mod script_commands;
pub mod search;
pub mod strings;
pub mod text_transforms;
pub mod time_zones;
pub mod timers;

pub use calculator::{evaluate, CalcResult};
pub use clipboard_db::{ClipboardDatabase, ClipboardEntry};
pub use clipboard_view::{
    age_between, format_age, preview, sort_for_display, Age, PaletteMode, PREVIEW_MAX_CHARS,
};
pub use config::{Config, PasteMode};
pub use desktop_entry::{DesktopApp, DesktopScanner, ScoredApp};
pub use i18n::{current_language, set_language, t, t_in, Language};
pub use paste::{decide_paste, PasteAction};
