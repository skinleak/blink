use std::{
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
};

use chrono::Local;
use directories::UserDirs;

use crate::error::{BlinkError, Result};

pub fn screenshots_dir() -> Result<PathBuf> {
    let pictures = UserDirs::new()
        .and_then(|dirs| dirs.picture_dir().map(Path::to_path_buf))
        .ok_or(BlinkError::PicturesDirectoryUnavailable)?;
    Ok(pictures.join("Blink"))
}

pub fn ensure_screenshots_dir() -> Result<PathBuf> {
    let directory = screenshots_dir()?;
    fs::create_dir_all(&directory).map_err(|source| BlinkError::CreateDirectory {
        path: directory.clone(),
        source,
    })?;
    Ok(directory)
}

pub fn save_png(source: &Path) -> Result<PathBuf> {
    let directory = ensure_screenshots_dir()?;
    let timestamp = Local::now().format("%Y-%m-%d-%H-%M-%S");
    let mut input = fs::File::open(source).map_err(|source_error| BlinkError::Save {
        path: source.to_path_buf(),
        source: source_error,
    })?;

    for suffix in 0_u32.. {
        let filename = if suffix == 0 {
            format!("blink-{timestamp}.png")
        } else {
            format!("blink-{timestamp}-{suffix}.png")
        };
        let destination = directory.join(filename);
        let output = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(BlinkError::Save {
                    path: destination,
                    source,
                });
            }
        };

        let mut output = output;
        if let Err(source) = io::copy(&mut input, &mut output) {
            let _ = fs::remove_file(&destination);
            return Err(BlinkError::Save {
                path: destination,
                source,
            });
        }

        return Ok(destination);
    }

    unreachable!("the suffix counter is unbounded")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_shape_is_stable() {
        let stamp = Local::now().format("%Y-%m-%d-%H-%M-%S").to_string();
        assert_eq!(stamp.len(), 19);
        assert!(
            stamp
                .chars()
                .all(|character| character.is_ascii_digit() || character == '-')
        );
    }
}
