//! Scripts as palette commands (port of `ScriptCommands.swift`), in the Raycast Script Commands
//! format: every file of the scripts folder becomes a command. The header can give a title, an icon
//! and a mode (`# @raycast.title`, `# @raycast.icon`, `# @raycast.mode`) and one argument
//! (`# @raycast.argument1 { "placeholder": "..." }`), which is the text typed after the title.
//! `@menew.` works in place of `@raycast.`. A script only runs when it is picked in the palette.
//!
//! Folder: `~/.local/share/menew/scripts`. AppleScript files are not offered on Linux.

use crate::i18n::Language;
use crate::palette::{Icon, ItemAction, Kind, PaletteItem};
use crate::search::fold;
use crate::strings::tr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Silent,
    Compact,
    FullOutput,
    Inline,
}

impl Mode {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "silent" => Some(Self::Silent),
            "compact" => Some(Self::Compact),
            "fullOutput" => Some(Self::FullOutput),
            "inline" => Some(Self::Inline),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    pub path: PathBuf,
    pub title: String,
    pub icon: Option<String>,
    pub mode: Mode,
    /// Placeholder of the first argument, when the script takes one.
    pub argument: Option<String>,
    pub package_name: Option<String>,
}

pub fn folder() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".local/share")).join("menew").join("scripts")
}

/// Raycast-style header of a script's text.
pub fn parse_text(text: &str, path: &Path) -> Script {
    let mut meta = std::collections::HashMap::<String, String>::new();
    for line in text.lines().take(40) {
        let Some(at) = line.find("@raycast.").or_else(|| line.find("@menew.")) else { continue };
        let rest = &line[at..];
        let rest = &rest[rest.find('.').map(|i| i + 1).unwrap_or(0)..];
        let key: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        if key.is_empty() {
            continue;
        }
        let value = rest[key.len()..].trim();
        // the regex of the Mac needs whitespace and a value after the key
        if rest[key.len()..].starts_with(char::is_whitespace) {
            meta.insert(key, value.to_string());
        }
    }
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let title = meta.get("title").filter(|t| !t.is_empty()).cloned().unwrap_or(stem);
    let argument = meta.get("argument1").map(|raw| {
        serde_json::from_str::<serde_json::Value>(raw)
            .ok()
            .and_then(|v| v.get("placeholder").and_then(|p| p.as_str()).map(String::from))
            .unwrap_or_else(|| "text".to_string())
    });
    Script {
        path: path.to_path_buf(),
        title,
        icon: meta.get("icon").cloned(),
        mode: meta.get("mode").and_then(|m| Mode::parse(m)).unwrap_or(Mode::Compact),
        argument,
        package_name: meta.get("packageName").cloned(),
    }
}

/// Scripts in `dir`, by title. Hidden files, `.md` files and AppleScript are skipped.
pub fn scan(dir: &Path) -> Vec<Script> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut list: Vec<Script> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            p.is_file() && !name.starts_with('.') && ext != "md" && ext != "applescript" && ext != "scpt"
        })
        .filter_map(|p| {
            let mut buf = vec![0u8; 4096];
            let n = std::fs::File::open(&p).ok()?.read(&mut buf).ok()?;
            buf.truncate(n);
            // like the Mac: a file that is not UTF-8 text in its first 4096 bytes is not a script
            Some(parse_text(&String::from_utf8(buf).ok()?, &p))
        })
        .collect();
    list.sort_by(|a, b| a.title.cmp(&b.title));
    list
}

/// How to run a file: an executable runs directly, the others through the interpreter of the extension.
pub fn launch_for(path: &Path, executable: bool) -> (String, Vec<String>) {
    let p = path.to_string_lossy().to_string();
    if executable {
        return (p, vec![]);
    }
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let env = |prog: &str| ("/usr/bin/env".to_string(), vec![prog.to_string(), p.clone()]);
    match ext.as_str() {
        "py" => env("python3"),
        "js" | "mjs" => env("node"),
        "rb" => env("ruby"),
        "swift" => env("swift"),
        _ => env("bash"),
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunResult {
    pub status: i32,
    pub output: String,
    pub error: String,
}

/// Run outside the main thread, with up to 2 minutes; the scripts folder is the working directory.
pub fn execute(script: &Script, argument: Option<&str>) -> RunResult {
    let (exe, mut args) = launch_for(&script.path, is_executable(&script.path));
    if let Some(a) = argument {
        args.push(a.to_string());
    }
    let mut path_env = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into());
    if let Some(home) = dirs::home_dir() {
        path_env = format!("{}:/usr/local/bin:{}", home.join(".local/bin").display(), path_env);
    }
    let child = std::process::Command::new(&exe)
        .args(&args)
        .current_dir(script.path.parent().unwrap_or(Path::new(".")))
        .env("PATH", path_env)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => return RunResult { status: -1, output: String::new(), error: e.to_string() },
    };
    let mut so = child.stdout.take();
    let mut se = child.stderr.take();
    let out_t = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(o) = so.as_mut() {
            let mut b = vec![];
            let _ = o.read_to_end(&mut b);
            s = String::from_utf8_lossy(&b).to_string();
        }
        s
    });
    let err_t = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(o) = se.as_mut() {
            let mut b = vec![];
            let _ = o.read_to_end(&mut b);
            s = String::from_utf8_lossy(&b).to_string();
        }
        s
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st.code().unwrap_or(-1),
            Ok(None) => {
                if start.elapsed() > Duration::from_secs(120) {
                    let _ = child.kill();
                    let _ = child.wait();
                    break -1;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break -1,
        }
    };
    RunResult { status, output: out_t.join().unwrap_or_default(), error: err_t.join().unwrap_or_default() }
}

/// Create the folder with an example and a short guide, the first time it is opened.
pub fn ensure_folder() -> std::io::Result<PathBuf> {
    let dir = folder();
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
        let example = dir.join("hello.sh");
        std::fs::write(
            &example,
            "#!/bin/bash\n\
             # @raycast.title Hello from Menew\n\
             # @raycast.icon 👋\n\
             # @raycast.mode compact\n\
             #\n\
             # Each file in this folder becomes a command in Menew's palette (Ctrl+Alt+Space).\n\
             # Modes: silent (nothing shown), compact (last line in a toast), fullOutput (a window with everything).\n\
             # Add \"# @raycast.argument1 { \\\"placeholder\\\": \\\"name\\\" }\" and type the text after the title.\n\
             \n\
             echo \"Hello! Today is $(date +%A).\"\n",
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&example, std::fs::Permissions::from_mode(0o755))?;
        }
    }
    Ok(dir)
}

/// Palette items: one per script, plus "Open scripts folder".
pub fn items(scripts: &[Script], raw: &str, lang: Language) -> Vec<PaletteItem> {
    let typed = raw.trim();
    let mut items: Vec<PaletteItem> = scripts
        .iter()
        .map(|s| {
            // "translate hello world": what comes after the title is the argument
            let mut arg: Option<String> = None;
            let tlen = s.title.chars().count();
            if s.argument.is_some() && typed.chars().count() > tlen {
                let head: String = typed.chars().take(tlen).collect();
                if fold(&head) == fold(&s.title) {
                    arg = Some(typed.chars().skip(tlen).collect::<String>().trim().to_string());
                }
            }
            let icon = match &s.icon {
                Some(i) if i.chars().count() <= 2 => Icon::Glyph(i.clone()),
                _ => Icon::Symbol("utilities-terminal-symbolic"),
            };
            let subtitle = if let Some(a) = &arg {
                tr("run_with", lang, &[a])
            } else if let Some(p) = &s.argument {
                tr("script_arg_sub", lang, &[p])
            } else if let Some(pkg) = &s.package_name {
                tr("script_pkg_sub", lang, &[pkg])
            } else {
                tr("script", lang, &[])
            };
            let file = s.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let kw = format!("script {} {}", file, arg.as_ref().map(|a| format!("{} {}", s.title, a)).unwrap_or_default());
            PaletteItem::new(&format!("script:{file}"), Kind::Action, &s.title)
                .subtitle(&subtitle)
                .kw(&kw)
                .icon(icon)
                .act(ItemAction::RunScript { script: s.clone(), argument: arg })
        })
        .collect();
    items.push(
        PaletteItem::new("action.scriptsfolder", Kind::Action, &tr("open_scripts", lang, &[]))
            .subtitle(&tr("open_scripts_sub", lang, &[]))
            .kw("scripts script pasta folder raycast comandos commands shell bash python")
            .icon(Icon::Symbol("folder-symbolic"))
            .act(ItemAction::OpenScriptsFolder),
    );
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_raycast_header() {
        let text = "#!/bin/bash\n# @raycast.schemaVersion 1\n# @raycast.title Translate\n# @raycast.icon 🌍\n# @raycast.mode fullOutput\n# @raycast.packageName Tools\n# @raycast.argument1 { \"type\": \"text\", \"placeholder\": \"phrase\" }\necho hi\n";
        let s = parse_text(text, Path::new("/x/translate.sh"));
        assert_eq!(s.title, "Translate");
        assert_eq!(s.icon.as_deref(), Some("🌍"));
        assert_eq!(s.mode, Mode::FullOutput);
        assert_eq!(s.argument.as_deref(), Some("phrase"));
        assert_eq!(s.package_name.as_deref(), Some("Tools"));
    }

    #[test]
    fn menew_prefix_and_defaults() {
        let s = parse_text("# @menew.title Hi\n", Path::new("/x/hi.sh"));
        assert_eq!(s.title, "Hi");
        assert_eq!(s.mode, Mode::Compact);
        assert_eq!(s.argument, None);
        let t = parse_text("echo no header\n", Path::new("/x/backup-home.sh"));
        assert_eq!(t.title, "backup-home");
    }

    #[test]
    fn argument_without_a_placeholder_is_called_text() {
        let s = parse_text("# @raycast.argument1 {}\n", Path::new("/x/a.sh"));
        assert_eq!(s.argument.as_deref(), Some("text"));
    }

    #[test]
    fn interpreters_by_extension() {
        let p = |f: &str| launch_for(Path::new(f), false);
        assert_eq!(p("/x/a.py"), ("/usr/bin/env".into(), vec!["python3".into(), "/x/a.py".into()]));
        assert_eq!(p("/x/a.js").1[0], "node");
        assert_eq!(p("/x/a.rb").1[0], "ruby");
        assert_eq!(p("/x/a.sh").1[0], "bash");
        assert_eq!(launch_for(Path::new("/x/a.sh"), true), ("/x/a.sh".into(), vec![]));
    }

    #[test]
    fn scan_and_run_a_script() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("greet.sh");
        std::fs::write(&f, "#!/bin/bash\n# @raycast.title Greet\n# @raycast.argument1 { \"placeholder\": \"who\" }\necho \"hello $1\"\n").unwrap();
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(dir.path().join("notes.md"), "ignore me").unwrap();
        std::fs::write(dir.path().join("x.applescript"), "ignore me").unwrap();
        let list = scan(dir.path());
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "Greet");
        let r = execute(&list[0], Some("world"));
        assert_eq!(r.status, 0);
        assert_eq!(r.output.trim(), "hello world");
    }
}
