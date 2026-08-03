mod portal;

use std::{future::Future, path::PathBuf};

use crate::error::Result;

pub use portal::PortalBackend;

#[derive(Clone, Copy, Debug)]
pub enum CaptureMode {
    Area,
    Screen,
    Window,
}

#[derive(Debug)]
pub struct CapturedImage {
    pub source: PathBuf,
}

pub trait CaptureBackend {
    fn capture(&self, mode: CaptureMode) -> impl Future<Output = Result<CapturedImage>> + Send;
}
