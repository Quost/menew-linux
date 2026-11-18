//! `menew check-updates [--install]`: the update check from a terminal, without the daemon or a window.
//! Prints what the check found; with `--install` it also downloads, verifies and installs (the way the
//! daemon does, see `menew_core::updates`) but does not restart anything: restart Menew yourself.
//! Exit status: 0 up to date or done, 1 on an error, 2 when `--install` could not install by itself.

use menew_core::strings::tr;
use menew_core::updates::{self, Feed, InstallError, InstallMethod, UpdateError, Version};
use menew_core::{current_language, Language};

pub fn run(args: &[String]) -> i32 {
    let lang: Language = current_language();
    let install = args.iter().any(|a| a == "--install");
    let feed = Feed::from_env();
    let current = Version::current();
    let found = match updates::check(&feed, &current) {
        Ok(Some(f)) => f,
        Ok(None) => {
            println!("{}", tr("upd_uptodate", lang, &[&current.to_string()]));
            return 0;
        }
        Err(e) => {
            eprintln!("{}: {}", tr("upd_check_failed", lang, &[]), e);
            return 1;
        }
    };
    println!("{}", tr("upd_available_tray", lang, &[&found.version.to_string()]));
    let notes = updates::plain_notes(&found.manifest.notes);
    if !notes.is_empty() {
        println!("\n{notes}\n");
    }
    println!("{}", found.page_url());
    if !install {
        return 0;
    }
    let method = updates::detect_install(&feed);
    println!("install method: {}", match &method {
        InstallMethod::AppImage(p) => format!("appimage ({})", p.display()),
        InstallMethod::Deb => "deb".to_string(),
        InstallMethod::Manual => "manual".to_string(),
    });
    match updates::install(&found, &method, &feed, &|stage| println!("stage: {stage}")) {
        Ok(()) => {
            println!("installed {}", found.version);
            0
        }
        Err(InstallError::Verify(UpdateError::Network)) => {
            eprintln!("{}", tr("upd_err_download", lang, &[]));
            1
        }
        Err(InstallError::Verify(e)) => {
            eprintln!("{} ({e})", tr("upd_err_verify", lang, &[]));
            1
        }
        Err(InstallError::NotWritable) => {
            eprintln!("{}", tr("upd_err_dir", lang, &[]));
            2
        }
        Err(InstallError::Manual) => {
            eprintln!("{}", tr("upd_manual", lang, &[&found.version.to_string()]));
            2
        }
        Err(InstallError::Declined) | Err(InstallError::Failed(_)) => {
            eprintln!("{}", tr("upd_err_install", lang, &[]));
            2
        }
    }
}
