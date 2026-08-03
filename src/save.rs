use std::{
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
};

use chrono::Local;
use directories::UserDirs;
use image::{GenericImageView, ImageFormat};

use crate::{
    capture::NormalizedRegion,
    error::{BlinkError, Result},
};

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
    let mut input = fs::File::open(source).map_err(|source_error| BlinkError::Save {
        path: source.to_path_buf(),
        source: source_error,
    })?;
    let (destination, mut output) = create_destination()?;

    if let Err(source) = io::copy(&mut input, &mut output) {
        let _ = fs::remove_file(&destination);
        return Err(BlinkError::Save {
            path: destination,
            source,
        });
    }
    Ok(destination)
}

pub fn save_cropped_png(source: &Path, region: NormalizedRegion) -> Result<PathBuf> {
    let image = image::open(source).map_err(|image_error| BlinkError::Image {
        path: source.to_path_buf(),
        source: image_error,
    })?;
    let (image_width, image_height) = image.dimensions();
    let x = (region.x.clamp(0.0, 1.0) * f64::from(image_width)).round() as u32;
    let y = (region.y.clamp(0.0, 1.0) * f64::from(image_height)).round() as u32;
    let width = (region.width.clamp(0.0, 1.0) * f64::from(image_width))
        .round()
        .max(1.0) as u32;
    let height = (region.height.clamp(0.0, 1.0) * f64::from(image_height))
        .round()
        .max(1.0) as u32;
    let width = width.min(image_width.saturating_sub(x));
    let height = height.min(image_height.saturating_sub(y));
    if width == 0 || height == 0 {
        return Err(BlinkError::Capture(
            "the selected area was empty".to_owned(),
        ));
    }

    let cropped = image.crop_imm(x, y, width, height);
    let (destination, mut output) = create_destination()?;
    if let Err(source) = cropped.write_to(&mut output, ImageFormat::Png) {
        let _ = fs::remove_file(&destination);
        return Err(BlinkError::Image {
            path: destination,
            source,
        });
    }
    Ok(destination)
}

fn create_destination() -> Result<(PathBuf, fs::File)> {
    let directory = ensure_screenshots_dir()?;
    let timestamp = Local::now().format("%Y-%m-%d-%H-%M-%S");

    for suffix in 0_u32.. {
        let filename = if suffix == 0 {
            format!("blink-{timestamp}.png")
        } else {
            format!("blink-{timestamp}-{suffix}.png")
        };
        let destination = directory.join(filename);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
        {
            Ok(file) => return Ok((destination, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(BlinkError::Save {
                    path: destination,
                    source,
                });
            }
        }
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
