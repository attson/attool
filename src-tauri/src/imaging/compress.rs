use image::{codecs::jpeg::JpegEncoder, DynamicImage, ImageFormat};
use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

pub struct CompressResult {
    pub output_path: PathBuf,
    pub original_size: u64,
    pub compressed_size: u64,
}

pub fn compress_image(
    input: &Path,
    output_dir: &Path,
    quality: u8,
) -> Result<CompressResult, String> {
    if !input.is_file() {
        return Err(format!("图片文件不存在：{}", input.display()));
    }
    fs::create_dir_all(output_dir).map_err(|error| format!("创建输出目录失败：{error}"))?;

    let original_size = fs::metadata(input)
        .map(|m| m.len())
        .map_err(|error| format!("读取原图信息失败：{error}"))?;

    let image = image::open(input).map_err(|error| format!("读取图片失败：{error}"))?;
    let format = detect_output_format(input);
    let output_path = unique_output_path(output_dir, input, &format);
    write_image(&image, &output_path, &format, quality)?;

    let compressed_size = fs::metadata(&output_path)
        .map(|m| m.len())
        .map_err(|error| format!("读取输出信息失败：{error}"))?;

    Ok(CompressResult {
        output_path,
        original_size,
        compressed_size,
    })
}

fn detect_output_format(path: &Path) -> ImageFormat {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "png" => ImageFormat::Png,
        "webp" => ImageFormat::WebP,
        _ => ImageFormat::Jpeg,
    }
}

fn write_image(
    image: &DynamicImage,
    output: &Path,
    format: &ImageFormat,
    quality: u8,
) -> Result<(), String> {
    let file = fs::File::create(output).map_err(|error| format!("创建输出文件失败：{error}"))?;
    let mut writer = BufWriter::new(file);

    match format {
        ImageFormat::Jpeg => {
            let rgb = image.to_rgb8();
            let mut encoder = JpegEncoder::new_with_quality(&mut writer, quality);
            encoder
                .encode_image(&rgb)
                .map_err(|error| format!("JPEG 编码失败：{error}"))
        }
        _ => image
            .write_to(&mut writer, *format)
            .map_err(|error| format!("图片编码失败：{error}")),
    }
}

fn unique_output_path(output_dir: &Path, input: &Path, format: &ImageFormat) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let ext = match format {
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Png => "png",
        ImageFormat::WebP => "webp",
        _ => "jpg",
    };

    let mut idx = 0;
    loop {
        let name = if idx == 0 {
            format!("{stem}_compressed.{ext}")
        } else {
            format!("{stem}_compressed_{idx}.{ext}")
        };
        let candidate = output_dir.join(name);
        if !candidate.exists() {
            return candidate;
        }
        idx += 1;
    }
}
