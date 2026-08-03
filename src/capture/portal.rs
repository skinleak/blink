use ashpd::{
    Error as PortalClientError, PortalError,
    desktop::{
        ResponseError,
        screenshot::{AvailableTargets, Screenshot},
    },
};
use std::{ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};

use super::{CaptureBackend, CaptureMode, CapturedImage};
use crate::error::{BlinkError, Result};

#[derive(Debug, Default)]
pub struct PortalBackend;

impl CaptureBackend for PortalBackend {
    async fn capture(&self, mode: CaptureMode) -> Result<CapturedImage> {
        let (target, interactive) = match mode {
            CaptureMode::Area => (AvailableTargets::Area, true),
            CaptureMode::Screen => (AvailableTargets::Screen, false),
            CaptureMode::Window => (AvailableTargets::Window, true),
        };

        let request = Screenshot::request()
            .interactive(interactive)
            .modal(false)
            .target(target)
            .send()
            .await
            .map_err(classify_portal_error)?;

        let screenshot = request.response().map_err(classify_portal_error)?;
        let uri = screenshot.uri().as_str();
        let source = file_uri_to_path(uri)?;

        Ok(CapturedImage { source })
    }
}

fn file_uri_to_path(uri: &str) -> Result<PathBuf> {
    let encoded = uri
        .strip_prefix("file://")
        .filter(|path| path.starts_with('/'))
        .ok_or_else(|| BlinkError::UnsupportedUri(uri.to_owned()))?;
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = bytes.get(index + 1).and_then(|value| hex_value(*value));
            let low = bytes.get(index + 2).and_then(|value| hex_value(*value));
            match (high, low) {
                (Some(high), Some(low)) => {
                    decoded.push((high << 4) | low);
                    index += 3;
                }
                _ => return Err(BlinkError::UnsupportedUri(uri.to_owned())),
            }
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    Ok(PathBuf::from(OsString::from_vec(decoded)))
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn classify_portal_error(error: PortalClientError) -> BlinkError {
    match error {
        PortalClientError::Response(ResponseError::Cancelled)
        | PortalClientError::Portal(PortalError::Cancelled(_)) => BlinkError::Cancelled,
        PortalClientError::Portal(PortalError::NotAllowed(message)) => {
            BlinkError::PermissionDenied(message)
        }
        PortalClientError::PortalNotFound(_)
        | PortalClientError::Portal(PortalError::NotFound(_))
        | PortalClientError::Zbus(_) => BlinkError::PortalUnavailable(error.to_string()),
        other => BlinkError::Capture(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_local_file_uri() {
        let result = file_uri_to_path("file:///tmp/a%20shot.png");
        assert!(matches!(result, Ok(path) if path == std::path::Path::new("/tmp/a shot.png")));
    }

    #[test]
    fn rejects_non_file_uri() {
        assert!(file_uri_to_path("https://example.com/image.png").is_err());
    }
}
