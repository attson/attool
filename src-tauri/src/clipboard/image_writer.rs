use image::DynamicImage;
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

#[cfg(target_os = "linux")]
const X11_OWNER_ENV: &str = "ATTOOL_X11_CLIPBOARD_OWNER";

#[cfg(target_os = "linux")]
const X11_OWNER_START_ATTEMPTS: usize = 3;

#[cfg(target_os = "linux")]
const X11_OWNER_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(40);

#[cfg(target_os = "linux")]
static X11_OWNER_PROCESS: std::sync::OnceLock<std::sync::Mutex<Option<std::process::Child>>> =
    std::sync::OnceLock::new();

#[cfg(target_os = "linux")]
fn replace_owner_with_retry<T>(
    owner: &mut Option<T>,
    attempts: usize,
    mut start_candidate: impl FnMut() -> Result<T, String>,
    mut stop_owner: impl FnMut(&mut T),
    mut wait_before_retry: impl FnMut(),
) -> Result<(), String> {
    let attempts = attempts.max(1);
    let mut last_error = None;

    for attempt in 0..attempts {
        match start_candidate() {
            Ok(candidate) => {
                let previous = owner.replace(candidate);
                if let Some(mut previous) = previous {
                    stop_owner(&mut previous);
                }
                return Ok(());
            }
            Err(error) => {
                last_error = Some(error);
                if attempt + 1 < attempts {
                    wait_before_retry();
                }
            }
        }
    }

    Err(last_error.expect("at least one owner start attempt"))
}

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
    use std::io::Cursor;

    let mut encoded = Cursor::new(Vec::new());
    image
        .write_to(&mut encoded, ImageFormat::Png)
        .map_err(|error| format!("encode X11 clipboard image: {error}"))?;

    let process_slot = X11_OWNER_PROCESS.get_or_init(|| std::sync::Mutex::new(None));
    let mut process_slot = process_slot
        .lock()
        .map_err(|_| "X11 clipboard owner lock is poisoned".to_string())?;

    replace_owner_with_retry(
        &mut process_slot,
        X11_OWNER_START_ATTEMPTS,
        || start_x11_owner(encoded.get_ref()),
        stop_x11_owner,
        || std::thread::sleep(X11_OWNER_RETRY_DELAY),
    )
}

#[cfg(target_os = "linux")]
fn start_x11_owner(encoded: &[u8]) -> Result<std::process::Child, String> {
    use std::{
        io::{BufRead, BufReader, Write},
        process::{Command, Stdio},
    };

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

    let ready_result = (|| {
        child
            .stdin
            .take()
            .ok_or_else(|| "X11 clipboard owner stdin is unavailable".to_string())
            .and_then(|mut stdin| {
                stdin
                    .write_all(encoded)
                    .map_err(|error| format!("send image to X11 clipboard owner: {error}"))
            })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "X11 clipboard owner stdout is unavailable".to_string())?;
        let mut response = String::new();
        BufReader::new(stdout)
            .read_line(&mut response)
            .map_err(|error| format!("read X11 clipboard owner status: {error}"))?;
        if response.trim() != "READY" {
            return Err(response
                .strip_prefix("ERROR ")
                .unwrap_or("X11 clipboard owner exited before it was ready")
                .trim()
                .to_string());
        }
        Ok(())
    })();

    if let Err(error) = ready_result {
        stop_x11_owner(&mut child);
        return Err(error);
    }

    Ok(child)
}

#[cfg(target_os = "linux")]
fn stop_x11_owner(owner: &mut std::process::Child) {
    let _ = owner.kill();
    let _ = owner.wait();
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
    use super::{encode_wayland_formats, replace_owner_with_retry};
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use std::{
        cell::Cell,
        sync::{mpsc, Arc, Barrier, Mutex},
        thread,
    };

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

    #[test]
    fn keeps_previous_owner_when_candidate_fails() {
        let mut owner = Some("previous");
        let mut stopped = Vec::new();

        let result = replace_owner_with_retry(
            &mut owner,
            1,
            || Err("candidate failed".to_string()),
            |owner| stopped.push(*owner),
            || {},
        );

        assert_eq!(result, Err("candidate failed".to_string()));
        assert_eq!(owner, Some("previous"));
        assert!(stopped.is_empty());
    }

    #[test]
    fn replaces_owner_before_stopping_previous_owner() {
        let mut owner = Some("previous");
        let candidate_started = Cell::new(false);
        let stopped_after_start = Cell::new(false);

        replace_owner_with_retry(
            &mut owner,
            1,
            || {
                candidate_started.set(true);
                Ok("candidate")
            },
            |_| stopped_after_start.set(candidate_started.get()),
            || {},
        )
        .expect("candidate should replace previous owner");

        assert_eq!(owner, Some("candidate"));
        assert!(stopped_after_start.get());
    }

    #[test]
    fn retries_candidate_start_and_keeps_last_error() {
        let mut owner = Some("previous");
        let attempts = Cell::new(0);
        let waits = Cell::new(0);

        replace_owner_with_retry(
            &mut owner,
            3,
            || {
                let attempt = attempts.get() + 1;
                attempts.set(attempt);
                (attempt == 3)
                    .then_some("candidate")
                    .ok_or_else(|| format!("failure {attempt}"))
            },
            |_| {},
            || waits.set(waits.get() + 1),
        )
        .expect("third attempt should succeed");

        assert_eq!(attempts.get(), 3);
        assert_eq!(waits.get(), 2);
        assert_eq!(owner, Some("candidate"));

        let result = replace_owner_with_retry(
            &mut owner,
            2,
            || Err("last failure".to_string()),
            |_| {},
            || {},
        );
        assert_eq!(result, Err("last failure".to_string()));
        assert_eq!(owner, Some("candidate"));
    }

    #[test]
    fn serializes_concurrent_owner_replacements() {
        let owner = Arc::new(Mutex::new(Some("initial")));
        let stopped = Arc::new(Mutex::new(Vec::new()));
        let (first_started_tx, first_started_rx) = mpsc::channel();
        let (release_first_tx, release_first_rx) = mpsc::channel();

        let first_owner = Arc::clone(&owner);
        let first_stopped = Arc::clone(&stopped);
        let first = thread::spawn(move || {
            let mut owner = first_owner.lock().expect("owner lock");
            replace_owner_with_retry(
                &mut owner,
                1,
                || {
                    first_started_tx.send(()).expect("report first start");
                    release_first_rx.recv().expect("release first start");
                    Ok("first")
                },
                |owner| first_stopped.lock().expect("stopped lock").push(*owner),
                || {},
            )
        });

        first_started_rx
            .recv()
            .expect("first candidate should start");
        let second_ready = Arc::new(Barrier::new(2));
        let second_owner = Arc::clone(&owner);
        let second_stopped = Arc::clone(&stopped);
        let second_ready_thread = Arc::clone(&second_ready);
        let second = thread::spawn(move || {
            second_ready_thread.wait();
            let mut owner = second_owner.lock().expect("owner lock");
            replace_owner_with_retry(
                &mut owner,
                1,
                || Ok("second"),
                |owner| second_stopped.lock().expect("stopped lock").push(*owner),
                || {},
            )
        });

        second_ready.wait();
        release_first_tx.send(()).expect("release first candidate");
        first
            .join()
            .expect("first thread")
            .expect("first replacement");
        second
            .join()
            .expect("second thread")
            .expect("second replacement");

        assert_eq!(*owner.lock().expect("owner lock"), Some("second"));
        assert_eq!(
            stopped.lock().expect("stopped lock").as_slice(),
            ["initial", "first"]
        );
    }
}
