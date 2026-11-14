use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopApp {
    pub id: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub exec: String,
    pub icon: Option<String>,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub terminal: bool,
    pub file_path: PathBuf,
}

impl DesktopApp {
    pub fn parse_file(path: &Path) -> Option<Self> {
        let content = fs::read_to_string(path).ok()?;
        Self::parse_content(&content, path)
    }

    pub fn parse_content(content: &str, path: &Path) -> Option<Self> {
        let mut in_desktop_entry = false;
        let mut props: HashMap<String, String> = HashMap::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                in_desktop_entry = trimmed == "[Desktop Entry]";
                continue;
            }

            if !in_desktop_entry {
                continue;
            }

            if let Some((key, val)) = trimmed.split_once('=') {
                props.insert(key.trim().to_string(), val.trim().to_string());
            }
        }

        // Validate basic application requirements
        if props.get("Type").map(|s| s.as_str()) != Some("Application") {
            return None;
        }

        if props.get("NoDisplay").map(|s| s.as_str()) == Some("true") {
            return None;
        }

        if props.get("Hidden").map(|s| s.as_str()) == Some("true") {
            return None;
        }

        let name = props.get("Name")?.to_string();
        let exec = props.get("Exec")?.to_string();

        let id = path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("unknown")
            .trim_end_matches(".desktop")
            .to_string();

        let generic_name = props.get("GenericName").cloned();
        let comment = props.get("Comment").cloned();
        let icon = props.get("Icon").cloned();
        let terminal = props.get("Terminal").map(|s| s.as_str()) == Some("true");

        let categories = props
            .get("Categories")
            .map(|s| {
                s.split(';')
                    .filter(|item| !item.is_empty())
                    .map(|item| item.to_string())
                    .collect()
            })
            .unwrap_or_default();

        let keywords = props
            .get("Keywords")
            .map(|s| {
                s.split(';')
                    .filter(|item| !item.is_empty())
                    .map(|item| item.to_string())
                    .collect()
            })
            .unwrap_or_default();

        Some(Self {
            id,
            name,
            generic_name,
            comment,
            exec,
            icon,
            categories,
            keywords,
            terminal,
            file_path: path.to_path_buf(),
        })
    }

    /// Clean Exec command by stripping field codes (%u, %F, %i, etc.)
    pub fn clean_exec_args(&self) -> Vec<String> {
        let parts = shlex_split(&self.exec);
        parts
            .into_iter()
            .filter(|arg| {
                if arg.starts_with('%') && arg.len() == 2 {
                    let code = arg.chars().nth(1).unwrap_or(' ');
                    // Standard desktop field codes: %f, %F, %u, %U, %d, %D, %n, %N, %i, %c, %k, %v, %m
                    !"fFuUdDnNicvkm".contains(code)
                } else {
                    true
                }
            })
            .collect()
    }

    /// Launch the application process
    pub fn launch(&self) -> std::io::Result<std::process::Child> {
        let args = self.clean_exec_args();
        if args.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Empty exec command in desktop file",
            ));
        }

        let program = &args[0];
        let mut cmd = Command::new(program);
        if args.len() > 1 {
            cmd.args(&args[1..]);
        }

        // Spawn detached
        cmd.spawn()
    }
}

fn shlex_split(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = ' ';

    for ch in input.chars() {
        match ch {
            '"' | '\'' if !in_quotes => {
                in_quotes = true;
                quote_char = ch;
            }
            c if in_quotes && c == quote_char => {
                in_quotes = false;
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        args.push(current);
    }

    args
}

#[derive(Debug, Clone)]
pub struct ScoredApp {
    pub app: DesktopApp,
    pub score: i32,
}

pub struct DesktopScanner;

impl DesktopScanner {
    pub fn standard_directories() -> Vec<PathBuf> {
        let mut dirs_list = Vec::new();

        if let Some(data_dir) = dirs::data_dir() {
            dirs_list.push(data_dir.join("applications"));
        }

        dirs_list.push(PathBuf::from("/usr/local/share/applications"));
        dirs_list.push(PathBuf::from("/usr/share/applications"));

        // Flatpak exports
        dirs_list.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
        if let Some(home) = dirs::home_dir() {
            dirs_list.push(home.join(".local/share/flatpak/exports/share/applications"));
        }

        // Snap exports
        dirs_list.push(PathBuf::from("/var/lib/snapd/desktop/applications"));

        dirs_list
    }

    pub fn scan_directories(directories: &[PathBuf]) -> Vec<DesktopApp> {
        let mut apps_by_id: HashMap<String, DesktopApp> = HashMap::new();

        for dir in directories {
            if !dir.exists() || !dir.is_dir() {
                continue;
            }

            for entry in walkdir::WalkDir::new(dir)
                .max_depth(2)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("desktop") {
                    if let Some(app) = DesktopApp::parse_file(path) {
                        // User directories take precedence over system ones
                        apps_by_id.entry(app.id.clone()).or_insert(app);
                    }
                }
            }
        }

        let mut list: Vec<DesktopApp> = apps_by_id.into_values().collect();
        list.sort_by_key(|a| a.name.to_lowercase());
        list
    }

    pub fn scan_all() -> Vec<DesktopApp> {
        let dirs = Self::standard_directories();
        Self::scan_directories(&dirs)
    }

    pub fn search(apps: &[DesktopApp], query: &str) -> Vec<ScoredApp> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return apps
                .iter()
                .cloned()
                .map(|app| ScoredApp { app, score: 0 })
                .collect();
        }

        let mut results = Vec::new();

        for app in apps {
            let name_lower = app.name.to_lowercase();
            let mut score = 0;

            if name_lower == q {
                score += 1000;
            } else if name_lower.starts_with(&q) {
                score += 500;
            } else if name_lower.contains(&q) {
                score += 200;
            }

            if let Some(gen_name) = &app.generic_name {
                let gen_lower = gen_name.to_lowercase();
                if gen_lower.starts_with(&q) {
                    score += 150;
                } else if gen_lower.contains(&q) {
                    score += 80;
                }
            }

            for kw in &app.keywords {
                if kw.to_lowercase().contains(&q) {
                    score += 50;
                }
            }

            if app.id.to_lowercase().contains(&q) {
                score += 40;
            }

            if score > 0 {
                results.push(ScoredApp {
                    app: app.clone(),
                    score,
                });
            }
        }

        results.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.app.name.cmp(&b.app.name)));
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_desktop_entry() {
        let content = r#"
[Desktop Entry]
Type=Application
Name=Firefox Web Browser
GenericName=Web Browser
Comment=Browse the World Wide Web
Exec=firefox %u
Icon=firefox
Terminal=false
Categories=Network;WebBrowser;
Keywords=internet;web;browser;
"#;
        let path = PathBuf::from("/usr/share/applications/firefox.desktop");
        let app = DesktopApp::parse_content(content, &path).expect("Should parse firefox.desktop");

        assert_eq!(app.id, "firefox");
        assert_eq!(app.name, "Firefox Web Browser");
        assert_eq!(app.generic_name, Some("Web Browser".to_string()));
        assert_eq!(app.exec, "firefox %u");
        assert_eq!(app.clean_exec_args(), vec!["firefox"]);
        assert_eq!(app.icon, Some("firefox".to_string()));
        assert!(!app.terminal);
        assert!(app.keywords.contains(&"browser".to_string()));
    }

    #[test]
    fn test_search_and_ranking() {
        let app1 = DesktopApp {
            id: "firefox".into(),
            name: "Firefox".into(),
            generic_name: Some("Web Browser".into()),
            comment: None,
            exec: "firefox".into(),
            icon: None,
            categories: vec![],
            keywords: vec!["web".into(), "internet".into()],
            terminal: false,
            file_path: PathBuf::from("/tmp/firefox.desktop"),
        };

        let app2 = DesktopApp {
            id: "files".into(),
            name: "Files".into(),
            generic_name: Some("File Manager".into()),
            comment: None,
            exec: "nautilus".into(),
            icon: None,
            categories: vec![],
            keywords: vec!["folder".into()],
            terminal: false,
            file_path: PathBuf::from("/tmp/files.desktop"),
        };

        let apps = vec![app1, app2];
        let res = DesktopScanner::search(&apps, "fire");
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].app.id, "firefox");

        let res_kw = DesktopScanner::search(&apps, "web");
        assert_eq!(res_kw.len(), 1);
        assert_eq!(res_kw[0].app.id, "firefox");
    }
}
