//! The screenshot portal (`org.freedesktop.portal.Screenshot`): the system takes the picture, Menew only asks.
//! With `interactive` the system shows its own picker for an area, a window or the whole screen; without it the
//! whole screen is taken. Menew never reads the framebuffer or the X11 root window.

use std::path::PathBuf;

/// The file the system wrote, `None` when the user cancelled, or why it failed (no portal, refused...).
pub async fn screenshot(interactive: bool) -> Result<Option<PathBuf>, String> {
    use ashpd::desktop::screenshot::Screenshot;
    let request = Screenshot::request().interactive(interactive).modal(true).send().await.map_err(|e| e.to_string())?;
    match request.response() {
        Ok(shot) => match menew_core::capture::uri_to_path(shot.uri().as_str()) {
            Some(path) => Ok(Some(path)),
            None => Err(format!("the portal gave an address that is not a file: {}", shot.uri())),
        },
        Err(ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
