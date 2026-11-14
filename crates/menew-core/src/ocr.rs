//! Text from copied images with the `tesseract` program (the Mac uses Vision). It runs on this
//! computer, nothing is uploaded. Without tesseract the images are simply not read.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcrError {
    /// `tesseract` is not installed.
    Missing,
    Failed(String),
}

impl std::fmt::Display for OcrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OcrError::Missing => write!(f, "tesseract is not installed (install tesseract-ocr)"),
            OcrError::Failed(why) => write!(f, "tesseract failed: {why}"),
        }
    }
}

/// The languages of Menew in the names of tesseract, in the order they are tried.
const WANTED: [&str; 5] = ["eng", "por", "deu", "spa", "chi_sim"];

/// `eng+por+...` with the languages of the output of `tesseract --list-langs` that Menew speaks.
pub fn pick_languages(list_output: &str) -> String {
    let have: Vec<&str> = list_output.lines().map(str::trim).collect();
    let picked: Vec<&str> = WANTED.iter().copied().filter(|w| have.contains(w)).collect();
    if picked.is_empty() {
        "eng".to_string()
    } else {
        picked.join("+")
    }
}

/// Is `tesseract` there? (checked once)
pub fn available() -> bool {
    languages().is_some()
}

fn languages() -> Option<&'static str> {
    static LANGS: OnceLock<Option<String>> = OnceLock::new();
    LANGS
        .get_or_init(|| {
            let out = Command::new("tesseract").arg("--list-langs").output().ok()?;
            if !out.status.success() {
                return None;
            }
            Some(pick_languages(&String::from_utf8_lossy(&out.stdout)))
        })
        .as_deref()
}

/// The recognized text of the image, trimmed (empty when there is none). Waits at most 2 minutes.
pub fn recognize(image: &Path) -> Result<String, OcrError> {
    let langs = languages().ok_or(OcrError::Missing)?;
    let mut child = Command::new("tesseract")
        .arg(image)
        .arg("stdout")
        .arg("-l")
        .arg(langs)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| if e.kind() == std::io::ErrorKind::NotFound { OcrError::Missing } else { OcrError::Failed(e.to_string()) })?;
    let mut stdout = child.stdout.take().ok_or_else(|| OcrError::Failed("no output".into()))?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() > Duration::from_secs(120) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(OcrError::Failed("timeout".into()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(OcrError::Failed(e.to_string())),
        }
    };
    let bytes = reader.join().unwrap_or_default();
    if !status.success() {
        return Err(OcrError::Failed(format!("exit status {}", status.code().unwrap_or(-1))));
    }
    Ok(String::from_utf8_lossy(&bytes).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_are_picked_from_the_list() {
        let out = "List of available languages in \"/usr/share/tesseract-ocr/5/tessdata/\" (4):\ndeu\neng\nosd\npor\n";
        assert_eq!(pick_languages(out), "eng+por+deu");
        assert_eq!(pick_languages("List of available languages (1):\nfra\n"), "eng");
        assert_eq!(pick_languages("eng\nchi_sim\n"), "eng+chi_sim");
    }

}
