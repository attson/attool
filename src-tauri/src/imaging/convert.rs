use image::{codecs::jpeg::JpegEncoder, ImageFormat};
use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

pub struct ConvertResult {
    pub output_path: PathBuf,
    pub original_size: u64,
    pub converted_size: u64,
}

pub fn convert_image(
    input: &Path,
    output_dir: &Path,
    target_format: &str,
    quality: u8,
) -> Result<ConvertResult, String> {
    if !input.is_file() {
        return Err(format!("图片文件不存在：{}", input.display()));
    }
    fs::create_dir_all(output_dir).map_err(|error| format!("创建输出目录失败：{error}"))?;

    let original_size = fs::metadata(input)
        .map(|m| m.len())
        .map_err(|error| format!("读取原图信息失败：{error}"))?;

    let image = image::open(input).map_err(|error| format!("读取图片失败：{error}"))?;
    let (format, ext) = parse_target_format(target_format)?;
    let output_path = unique_output_path(output_dir, input, ext);

    let file =
        fs::File::create(&output_path).map_err(|error| format!("创建输出文件失败：{error}"))?;
    let mut writer = BufWriter::new(file);

    match format {
        ImageFormat::Jpeg => {
            let rgb = image.to_rgb8();
            let mut encoder = JpegEncoder::new_with_quality(&mut writer, quality);
            encoder
                .encode_image(&rgb)
                .map_err(|error| format!("JPEG 编码失败：{error}"))?;
        }
        _ => image
            .write_to(&mut writer, format)
            .map_err(|error| format!("图片编码失败：{error}"))?,
    }

    let converted_size = fs::metadata(&output_path)
        .map(|m| m.len())
        .map_err(|error| format!("读取输出信息失败：{error}"))?;

    Ok(ConvertResult {
        output_path,
        original_size,
        converted_size,
    })
}

fn parse_target_format(target: &str) -> Result<(ImageFormat, &'static str), String> {
    match target.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Ok((ImageFormat::Jpeg, "jpg")),
        "png" => Ok((ImageFormat::Png, "png")),
        "webp" => Ok((ImageFormat::WebP, "webp")),
        other => Err(format!("不支持的目标格式：{other}")),
    }
}

fn unique_output_path(output_dir: &Path, input: &Path, ext: &str) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let mut idx = 0;
    loop {
        let name = if idx == 0 {
            format!("{stem}.{ext}")
        } else {
            format!("{stem}_{idx}.{ext}")
        };
        let candidate = output_dir.join(name);
        if !candidate.exists() {
            return candidate;
        }
        idx += 1;
    }
}
