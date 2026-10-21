use image::DynamicImage;
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

#[cfg(target_os = "linux")]
const X11_OWNER_ENV: &str = "ATTOOL_X11_CLIPBOARD_OWNER";

#[cfg(target_os = "linux")]
static X11_OWNER_PROCESS: std::sync::OnceLock<std::sync::Mutex<Option<std::process::Child>>> =
    std::sync::OnceLock::new();

pub(crate) fn write_image(app: &AppHandle, image: &DynamicImage) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        if std::env::var_os("DISPLAY").is_some() {
            // Claiming both selections races COSMIC's clipboard bridge: its delayed Wayland-to-X11
            // claim can replace our valid X11 owner with an offer whose image payload is empty.
            // A dedicated X11 process also keeps GDK's event loop isolated from the Wayland UI.
            return write_x11_image(image);
        }
        return write_wayland_image(image);
    }

    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let image = tauri::image::Image::new_owned(rgba.into_raw(), width, height);
    app.clipboard()
        .write_image(&image)
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "linux")]
fn write_x11_image(image: &DynamicImage) -> Result<(), String> {
    use image::ImageFormat;
    use std::{
        io::{BufRead, BufReader, Cursor, Write},
        process::{Command, Stdio},
    };

    let mut encoded = Cursor::new(Vec::new());
    image
        .write_to(&mut encoded, ImageFormat::Png)
        .map_err(|error| format!("encode X11 clipboard image: {error}"))?;

    let process_slot = X11_OWNER_PROCESS.get_or_init(|| std::sync::Mutex::new(None));
    let mut process_slot = process_slot
        .lock()
        .map_err(|_| "X11 clipboard owner lock is poisoned".to_string())?;
    if let Some(mut previous) = process_slot.take() {
        let _ = previous.kill();
        let _ = previous.wait();
    }

    let executable = std::env::current_exe()
        .map_err(|error| format!("resolve X11 clipboard owner executable: {error}"))?;
    let mut child = Command::new(executable)
        .env(X11_OWNER_ENV, "1")
        .env("GDK_BACKEND", "x11")
        .env(
            "ATTOOL_CLIPBOARD_PARENT_PID",
            std::process::id().to_string(),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("start X11 clipboard owner: {error}"))?;

    let write_result = child
        .stdin
        .take()
        .ok_or_else(|| "X11 clipboard owner stdin is unavailable".to_string())
        .and_then(|mut stdin| {
            stdin
                .write_all(encoded.get_ref())
                .map_err(|error| format!("send image to X11 clipboard owner: {error}"))
        });
    if let Err(error) = write_result {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "X11 clipboard owner stdout is unavailable".to_string())?;
    let mut response = String::new();
    BufReader::new(stdout)
        .read_line(&mut response)
        .map_err(|error| format!("read X11 clipboard owner status: {error}"))?;
    if response.trim() != "READY" {
        let _ = child.kill();
        let _ = child.wait();
        return Err(response
            .strip_prefix("ERROR ")
            .unwrap_or("X11 clipboard owner exited before it was ready")
            .trim()
            .to_string());
    }

    *process_slot = Some(child);
    Ok(())
}

#[cfg(target_os = "linux")]
fn run_x11_clipboard_owner() -> Result<(), String> {
    use std::io::{Read, Write};

    gtk::init().map_err(|error| format!("initialize GTK X11 clipboard owner: {error}"))?;
    let mut encoded = Vec::new();
    std::io::stdin()
        .read_to_end(&mut encoded)
        .map_err(|error| format!("read X11 clipboard image: {error}"))?;
    let image = image::load_from_memory(&encoded)
        .map_err(|error| format!("decode X11 clipboard image: {error}"))?
        .to_rgba8();
    let (width, height) = image.dimensions();
    let width = i32::try_from(width).map_err(|_| "image width exceeds X11 limit".to_string())?;
    let height = i32::try_from(height).map_err(|_| "image height exceeds X11 limit".to_string())?;
    let row_stride = width
        .checked_mul(4)
        .ok_or_else(|| "image row stride exceeds X11 limit".to_string())?;
    let pixbuf = gtk::gdk_pixbuf::Pixbuf::from_mut_slice(
        image.into_raw(),
        gtk::gdk_pixbuf::Colorspace::Rgb,
        true,
        8,
        width,
        height,
        row_stride,
    );
    let display =
        gtk::gdk::Display::default().ok_or_else(|| "X11 display is not available".to_string())?;
    let clipboard = gtk::Clipboard::default(&display)
        .ok_or_else(|| "X11 clipboard is not available".to_string())?;
    clipboard.set_image(&pixbuf);
    display.flush();

    let parent_pid = std::env::var("ATTOOL_CLIPBOARD_PARENT_PID").unwrap_or_default();
    gtk::glib::timeout_add_seconds_local(1, move || {
        if parent_pid.is_empty() || std::path::Path::new("/proc").join(&parent_pid).exists() {
            gtk::glib::ControlFlow::Continue
        } else {
            gtk::main_quit();
            gtk::glib::ControlFlow::Break
        }
    });

    println!("READY");
    std::io::stdout()
        .flush()
        .map_err(|error| format!("report X11 clipboard owner status: {error}"))?;
    gtk::main();
    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) fn run_x11_clipboard_owner_if_requested() -> bool {
    if std::env::var_os(X11_OWNER_ENV).as_deref() != Some(std::ffi::OsStr::new("1")) {
        return false;
    }

    if let Err(error) = run_x11_clipboard_owner() {
        println!("ERROR {}", error.to_string().replace(['\r', '\n'], " "));
    }
    true
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
    let rgb = RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let pixel = rgba.get_pixel(x, y);
        let alpha = u16::from(pixel[3]);
        Rgb(std::array::from_fn(|channel| {
            ((u16::from(pixel[channel]) * alpha + 255 * (255 - alpha) + 127) / 255) as u8
        }))
    });
    let png_image = DynamicImage::ImageRgba8(rgba);
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
    fn encodes_rgba_png_and_common_wayland_fallbacks() {
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
        assert_eq!(png.color(), image::ColorType::Rgba8);
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
