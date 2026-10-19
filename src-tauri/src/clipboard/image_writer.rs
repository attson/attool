use image::DynamicImage;
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

pub(crate) fn write_image(app: &AppHandle, image: &DynamicImage) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        if write_wayland_image(image).is_ok() {
            return Ok(());
        }
    }

    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let image = tauri::image::Image::new_owned(rgba.into_raw(), width, height);
    app.clipboard()
        .write_image(&image)
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "linux")]
fn write_wayland_image(image: &DynamicImage) -> Result<(), String> {
    use wl_clipboard_rs::copy::{MimeSource, MimeType, Options, Source};

    let sources = encode_wayland_formats(image)?
        .into_iter()
        .map(|(mime_type, bytes)| MimeSource {
            source: Source::Bytes(bytes.into_boxed_slice()),
            mime_type: MimeType::Specific(mime_type.to_string()),
        })
        .collect();

    Options::new()
        .copy_multi(sources)
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "linux")]
fn encode_wayland_formats(image: &DynamicImage) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
    use std::io::Cursor;

    let rgba = image.to_rgba8();
    let opaque = rgba.pixels().all(|pixel| pixel[3] == u8::MAX);
    let rgb = if opaque {
        image.to_rgb8()
    } else {
        RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
            let pixel = rgba.get_pixel(x, y);
            let alpha = u16::from(pixel[3]);
            Rgb(std::array::from_fn(|channel| {
                ((u16::from(pixel[channel]) * alpha + 255 * (255 - alpha) + 127) / 255) as u8
            }))
        })
    };
    let png_image = if opaque {
        DynamicImage::ImageRgb8(rgb.clone())
    } else {
        DynamicImage::ImageRgba8(rgba)
    };
    let fallback_image = DynamicImage::ImageRgb8(rgb);
    let encode = |source: &DynamicImage, format| {
        let mut output = Cursor::new(Vec::new());
        source
            .write_to(&mut output, format)
            .map_err(|error| error.to_string())?;
        Ok::<_, String>(output.into_inner())
    };

    let png = encode(&png_image, ImageFormat::Png)?;
    let bmp = encode(&fallback_image, ImageFormat::Bmp)?;
    let jpeg = encode(&fallback_image, ImageFormat::Jpeg)?;

    Ok(vec![
        ("image/png", png),
        ("image/bmp", bmp),
        ("image/jpeg", jpeg.clone()),
        ("image/jpg", jpeg),
    ])
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::encode_wayland_formats;
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

    #[test]
    fn encodes_rgb_image_in_common_wayland_formats() {
        let source = DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 1, Rgba([12, 34, 56, 255])));

        let formats = encode_wayland_formats(&source).expect("formats should encode");

        assert_eq!(
            formats.iter().map(|(mime, _)| *mime).collect::<Vec<_>>(),
            vec!["image/png", "image/bmp", "image/jpeg", "image/jpg"]
        );
        for (mime, bytes) in &formats {
            let format = match *mime {
                "image/png" => ImageFormat::Png,
                "image/bmp" => ImageFormat::Bmp,
                "image/jpeg" | "image/jpg" => ImageFormat::Jpeg,
                _ => unreachable!(),
            };
            let decoded = image::load_from_memory_with_format(bytes, format)
                .expect("encoded image should decode");
            assert_eq!((decoded.width(), decoded.height()), (2, 1));
        }

        let png = image::load_from_memory_with_format(&formats[0].1, ImageFormat::Png)
            .expect("PNG should decode");
        assert_eq!(png.color(), image::ColorType::Rgb8);
    }

    #[test]
    fn preserves_transparency_in_png_and_flattens_fallbacks() {
        let source = DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([12, 34, 56, 0])));

        let formats = encode_wayland_formats(&source).expect("formats should encode");
        let png = image::load_from_memory_with_format(&formats[0].1, ImageFormat::Png)
            .expect("PNG should decode");
        let bmp = image::load_from_memory_with_format(&formats[1].1, ImageFormat::Bmp)
            .expect("BMP should decode")
            .to_rgb8();

        assert_eq!(png.color(), image::ColorType::Rgba8);
        assert_eq!(bmp.get_pixel(0, 0).0, [255, 255, 255]);
    }
}
