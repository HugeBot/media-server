//! CPU-bound image decoding, resizing and WebP re-encoding.
//!
//! [`process_image`] is run inside `tokio::task::spawn_blocking` by the
//! upload handler so this synchronous, CPU-heavy work doesn't block the
//! async runtime.

use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, GenericImageView, ImageEncoder, imageops::FilterType};

use crate::error::AppError;

/// Decodes the input image, resizes it so its longest side is at most
/// `max_dimension` (preserving aspect ratio, never upscaling), and encodes
/// the result as a lossless WebP.
///
/// `max_dimension` is the *effective* cap for this upload: the upload
/// handler computes it from the bucket's configured `max_dimension` and any
/// `max_dimension_override` from the request.
///
/// Re-encoding is always lossless (`WebPEncoder::new_lossless`) regardless
/// of the input format, trading file size for fidelity and avoiding the
/// extra native dependencies a lossy WebP encoder would require.
pub fn process_image(bytes: &[u8], max_dimension: u32) -> Result<Vec<u8>, AppError> {
    let img = image::load_from_memory(bytes).map_err(AppError::DecodeError)?;

    let (width, height) = img.dimensions();
    let (target_width, target_height) = target_dimensions(width, height, max_dimension);

    let resized = if (target_width, target_height) == (width, height) {
        img
    } else {
        img.resize(target_width, target_height, FilterType::Lanczos3)
    };

    let rgba = resized.into_rgba8();
    let (width, height) = rgba.dimensions();

    let mut buf = Vec::new();
    WebPEncoder::new_lossless(&mut buf)
        .write_image(&rgba, width, height, ExtendedColorType::Rgba8)
        .map_err(AppError::EncodeError)?;

    Ok(buf)
}

/// Computes the output dimensions: the longest side is capped at
/// `max_dimension`, the other side scales proportionally. Images smaller than
/// `max_dimension` on their longest side are left unchanged.
// The scaled side is always >= 0 and <= `max_dimension` (capped at
// `buckets::MAX_DIMENSION`, well within `u32`), so the f64 -> u32 round-trip
// below never truncates or loses sign.
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn target_dimensions(width: u32, height: u32, max_dimension: u32) -> (u32, u32) {
    if width >= height {
        if width <= max_dimension {
            (width, height)
        } else {
            let new_height =
                (f64::from(height) * f64::from(max_dimension) / f64::from(width)).round() as u32;
            (max_dimension, new_height.max(1))
        }
    } else if height <= max_dimension {
        (width, height)
    } else {
        let new_width =
            (f64::from(width) * f64::from(max_dimension) / f64::from(height)).round() as u32;
        (new_width.max(1), max_dimension)
    }
}
