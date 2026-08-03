use std::{fs::File, os::fd::AsFd, path::PathBuf};

use ashpd::desktop::open_uri::OpenDirectoryRequest;

use crate::{
    capture::{CaptureBackend, CaptureMode, PortalBackend},
    error::{BlinkError, Result},
    notification, save,
};

pub async fn capture(mode: CaptureMode, notify_errors: bool) -> Result<PathBuf> {
    eprintln!("Blink: starting {mode:?} capture");
    let result = capture_inner(mode).await;

    match &result {
        Ok(path) => {
            println!("Saved screenshot to {}", path.display());
            if let Err(error) = notification::success(path).await {
                eprintln!("Blink: screenshot saved, but notification failed: {error}");
            }
        }
        Err(error) if error.is_cancelled() => {
            eprintln!("Blink: capture cancelled");
        }
        Err(error) if notify_errors => {
            if let Err(notification_error) = notification::error(error).await {
                eprintln!("Blink: notification failed: {notification_error}");
            }
        }
        Err(_) => {}
    }

    result
}

async fn capture_inner(mode: CaptureMode) -> Result<PathBuf> {
    let image = PortalBackend.capture(mode).await?;
    save::save_png(&image.source)
}

pub async fn open_screenshots_folder() -> Result<()> {
    let directory = save::ensure_screenshots_dir()?;
    let file = File::open(&directory).map_err(|error| BlinkError::OpenFolder(error.to_string()))?;
    OpenDirectoryRequest::default()
        .send(&file.as_fd())
        .await
        .map_err(|error| BlinkError::OpenFolder(error.to_string()))?;
    Ok(())
}
