use std::{fs, path::PathBuf};

use directories::BaseDirs;

use crate::error::{BlinkError, Result};

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub copy_to_clipboard: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            copy_to_clipboard: true,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let Some(path) = settings_path() else {
            return Self::default();
        };
        match fs::read_to_string(path) {
            Ok(contents) => Self {
                copy_to_clipboard: contents
                    .lines()
                    .any(|line| line.trim() == "copy_to_clipboard=true"),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                eprintln!("Blink: could not read settings: {error}");
                Self::default()
            }
        }
    }

    pub fn save(self) -> Result<()> {
        let path = settings_path().ok_or_else(|| BlinkError::Settings {
            path: PathBuf::from("~/.config/blink/settings.conf"),
            source: std::io::Error::other("configuration directory is unavailable"),
        })?;
        if let Some(directory) = path.parent() {
            fs::create_dir_all(directory).map_err(|source| BlinkError::Settings {
                path: path.clone(),
                source,
            })?;
        }
        fs::write(
            &path,
            format!("copy_to_clipboard={}\n", self.copy_to_clipboard),
        )
        .map_err(|source| BlinkError::Settings { path, source })
    }
}

fn settings_path() -> Option<PathBuf> {
    BaseDirs::new().map(|dirs| dirs.config_dir().join("blink/settings.conf"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_copy_is_enabled_by_default() {
        assert!(Settings::default().copy_to_clipboard);
    }
}
