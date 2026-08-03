use std::path::Path;

use notify_rust::{Notification, Timeout};

use crate::error::BlinkError;

pub async fn success(path: &Path) -> notify_rust::error::Result<()> {
    Notification::new()
        .appname("Blink")
        .summary("Screenshot saved")
        .body(&path.display().to_string())
        .icon("camera-photo")
        .timeout(Timeout::Milliseconds(4000))
        .show_async()
        .await
        .map(|_| ())
}

pub async fn error(error: &BlinkError) -> notify_rust::error::Result<()> {
    Notification::new()
        .appname("Blink")
        .summary("Screenshot failed")
        .body(&error.to_string())
        .icon("dialog-error")
        .timeout(Timeout::Milliseconds(6000))
        .show_async()
        .await
        .map(|_| ())
}
