#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeBackend {
    Primary,
    Fallback,
}

pub fn target_dimensions(width: u32, height: u32, max_dimension: u32) -> (u32, u32) {
    if width == 0 || height == 0 || max_dimension == 0 {
        return (width, height);
    }
    if width <= max_dimension && height <= max_dimension {
        return (width, height);
    }

    if width >= height {
        let scaled_height = ((height as u64 * max_dimension as u64) / width as u64).max(1);
        (max_dimension, scaled_height as u32)
    } else {
        let scaled_width = ((width as u64 * max_dimension as u64) / height as u64).max(1);
        (scaled_width as u32, max_dimension)
    }
}

pub fn decode_with_fallback<T, P, F>(primary: P, fallback: F) -> Option<(T, DecodeBackend)>
where
    P: FnOnce() -> Option<T>,
    F: FnOnce() -> Option<T>,
{
    if let Some(value) = primary() {
        return Some((value, DecodeBackend::Primary));
    }
    fallback().map(|value| (value, DecodeBackend::Fallback))
}

pub fn decode_target_image(
    source_path: &str,
    max_dimension: u32,
) -> Option<(image::DynamicImage, DecodeBackend)> {
    decode_with_fallback(
        || {
            #[cfg(windows)]
            {
                match crate::windows_image_decoder::decode_target(source_path, max_dimension) {
                    Ok(image) => Some(image),
                    Err(error) => {
                        eprintln!("[PureWall] WIC decode fallback for {source_path}: {error}");
                        None
                    }
                }
            }
            #[cfg(not(windows))]
            {
                None
            }
        },
        || {
            let image = image::open(source_path).ok()?;
            let (target_width, target_height) =
                target_dimensions(image.width(), image.height(), max_dimension);
            if target_width == image.width() && target_height == image.height() {
                Some(image)
            } else {
                Some(image.resize_exact(
                    target_width,
                    target_height,
                    image::imageops::FilterType::Lanczos3,
                ))
            }
        },
    )
}
