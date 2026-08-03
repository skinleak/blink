use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, BlinkError>;

#[derive(Debug, Error)]
pub enum BlinkError {
    #[error("capture was cancelled")]
    Cancelled,

    #[error("the screenshot portal is unavailable: {0}")]
    PortalUnavailable(String),

    #[error("the screenshot portal denied the request or lacks permission: {0}")]
    PermissionDenied(String),

    #[error("screenshot capture failed: {0}")]
    Capture(String),

    #[error("the portal returned an unsupported screenshot URI: {0}")]
    UnsupportedUri(String),

    #[error("could not determine the user's Pictures directory")]
    PicturesDirectoryUnavailable,

    #[error("could not create screenshot directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("could not save screenshot to {path}: {source}")]
    Save {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("could not open screenshots folder: {0}")]
    OpenFolder(String),

    #[error("could not start the system tray: {0}")]
    Tray(String),
}

impl BlinkError {
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }
}
