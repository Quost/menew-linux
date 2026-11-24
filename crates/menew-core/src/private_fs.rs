//! Folders and files only this user can read: the data folder (`~/.local/share/menew`) holds the
//! clipboard history, the notes and the snippets, so it is 0700 and its files are 0600, whatever the umask.

use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// Mode of the private folders.
pub const DIR_MODE: u32 = 0o700;
/// Mode of the private files.
pub const FILE_MODE: u32 = 0o600;

/// Create `dir` with mode 0700 (its missing parents get the usual modes); an existing `dir` is tightened
/// to 0700.
pub fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    if let Some(parent) = dir.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    match std::fs::DirBuilder::new().mode(DIR_MODE).create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && dir.is_dir() => {}
        Err(e) => return Err(e),
    }
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(DIR_MODE))
}

/// Write `data` to `path` atomically (a temporary file next to it, then a rename) with mode 0600.
pub fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".tmp");
    let tmp = path.with_file_name(name);
    let result = (|| {
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(FILE_MODE).open(&tmp)?;
        // a temporary file left behind by an older version keeps its mode: set it again
        f.set_permissions(std::fs::Permissions::from_mode(FILE_MODE))?;
        f.write_all(data)?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Create an empty file with mode 0600 if `path` does not exist yet (so SQLite, which gives its journal
/// files the mode of the database, keeps them private too).
pub fn ensure_private_file(path: &Path) -> std::io::Result<()> {
    match std::fs::OpenOptions::new().write(true).create_new(true).mode(FILE_MODE).open(path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    }
}

/// At startup: make an existing data folder 0700 and its `clipboard.db*` files 0600 (older versions
/// created them with the umask, usually readable by everyone). A missing folder is left alone.
pub fn tighten_data_dir(dir: &Path) {
    if !dir.is_dir() {
        return;
    }
    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(DIR_MODE));
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let is_db = name.to_string_lossy().starts_with("clipboard.db");
        let Ok(kind) = entry.file_type() else { continue };
        if is_db && kind.is_file() {
            let _ = std::fs::set_permissions(entry.path(), std::fs::Permissions::from_mode(FILE_MODE));
        } else if kind.is_dir() {
            let _ = std::fs::set_permissions(entry.path(), std::fs::Permissions::from_mode(DIR_MODE));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(p: &Path) -> u32 {
        std::fs::metadata(p).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn new_folders_and_files_are_private() {
        let base = tempfile::tempdir().unwrap();
        let dir = base.path().join("share/menew");
        create_private_dir(&dir).unwrap();
        assert_eq!(mode(&dir), 0o700);
        let file = dir.join("config.json");
        write_private(&file, b"{}").unwrap();
        assert_eq!(mode(&file), 0o600);
        assert_eq!(std::fs::read(&file).unwrap(), b"{}");
        assert!(!dir.join("config.json.tmp").exists());
        let db = dir.join("clipboard.db");
        ensure_private_file(&db).unwrap();
        assert_eq!(mode(&db), 0o600);
    }

    #[test]
    fn existing_data_folder_is_tightened() {
        let base = tempfile::tempdir().unwrap();
        let dir = base.path().join("menew");
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::create_dir(dir.join("clipboard")).unwrap();
        std::fs::set_permissions(dir.join("clipboard"), std::fs::Permissions::from_mode(0o755)).unwrap();
        for f in ["clipboard.db", "clipboard.db-wal", "clipboard.db-shm"] {
            std::fs::write(dir.join(f), b"x").unwrap();
            std::fs::set_permissions(dir.join(f), std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        tighten_data_dir(&dir);
        assert_eq!(mode(&dir), 0o700);
        assert_eq!(mode(&dir.join("clipboard")), 0o700);
        for f in ["clipboard.db", "clipboard.db-wal", "clipboard.db-shm"] {
            assert_eq!(mode(&dir.join(f)), 0o600, "{f}");
        }
        // an existing folder passed to create_private_dir is tightened too
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        create_private_dir(&dir).unwrap();
        assert_eq!(mode(&dir), 0o700);
    }
}
