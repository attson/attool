use image::{ImageFormat, Luma};
use qrcode::{EcLevel, QrCode};
use std::io::Cursor;

pub fn encode_png(text: &str, ec: EcLevel, module_pixels: u32, quiet: u32) -> Result<Vec<u8>, String> {
    if text.is_empty() {
        return Err("请输入要编码的内容".to_string());
    }
    let code = QrCode::with_error_correction_level(text.as_bytes(), ec)
        .map_err(|error| format!("QR 编码失败：{error}"))?;
    let module = module_pixels.max(1);
    let quiet = quiet.max(0);
    let image = code
        .render::<Luma<u8>>()
        .min_dimensions(0, 0)
        .quiet_zone(quiet > 0)
        .module_dimensions(module, module)
        .build();
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageLuma8(image)
        .write_to(&mut bytes, ImageFormat::Png)
        .map_err(|error| format!("PNG 编码失败：{error}"))?;
    Ok(bytes.into_inner())
}

pub fn ec_from_str(s: &str) -> EcLevel {
    match s.to_ascii_uppercase().as_str() {
        "L" => EcLevel::L,
        "M" => EcLevel::M,
        "Q" => EcLevel::Q,
        "H" => EcLevel::H,
        _ => EcLevel::M,
    }
}
