use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, GenericImageView, ImageEncoder, imageops::FilterType};

use crate::error::AppError;

const MAX_DIMENSION: u32 = 1000;

/// Decodes the input image, resizes it so its longest side is at most
/// `MAX_DIMENSION` (preserving aspect ratio, never upscaling), and encodes
/// the result as a lossless WebP.
pub fn process_image(bytes: &[u8]) -> Result<Vec<u8>, AppError> {
    let img = image::load_from_memory(bytes).map_err(AppError::DecodeError)?;

    let (width, height) = img.dimensions();
    let (target_width, target_height) = target_dimensions(width, height);

    let resized = if (target_width, target_height) != (width, height) {
        img.resize(target_width, target_height, FilterType::Lanczos3)
    } else {
        img
    };

    let rgba = resized.to_rgba8();
    let (width, height) = rgba.dimensions();

    let mut buf = Vec::new();
    WebPEncoder::new_lossless(&mut buf)
        .write_image(&rgba, width, height, ExtendedColorType::Rgba8)
        .map_err(AppError::EncodeError)?;

    Ok(buf)
}

/// Computes the output dimensions: the longest side is capped at
/// `MAX_DIMENSION`, the other side scales proportionally. Images smaller than
/// `MAX_DIMENSION` on their longest side are left unchanged.
fn target_dimensions(width: u32, height: u32) -> (u32, u32) {
    if width >= height {
        if width <= MAX_DIMENSION {
            (width, height)
        } else {
            let new_height = (height as f64 * MAX_DIMENSION as f64 / width as f64).round() as u32;
            (MAX_DIMENSION, new_height.max(1))
        }
    } else if height <= MAX_DIMENSION {
        (width, height)
    } else {
        let new_width = (width as f64 * MAX_DIMENSION as f64 / height as f64).round() as u32;
        (new_width.max(1), MAX_DIMENSION)
    }
}
