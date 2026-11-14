pub mod autostart;
pub mod calculator;
pub mod clipboard_db;
pub mod clipboard_view;
pub mod config;
pub mod desktop_entry;
pub mod i18n;
pub mod paste;

pub use calculator::{evaluate_expression, format_result};
pub use clipboard_db::{ClipboardDatabase, ClipboardEntry};
pub use clipboard_view::{
    age_between, format_age, preview, sort_for_display, Age, PaletteMode, PREVIEW_MAX_CHARS,
};
pub use config::{Config, PasteMode};
pub use desktop_entry::{DesktopApp, DesktopScanner, ScoredApp};
pub use i18n::{current_language, set_language, t, t_in, Language};
pub use paste::{decide_paste, PasteAction};
