use exif::{In, Reader};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

pub struct ExifField {
    pub tag: String,
    pub value: String,
}

pub fn read_exif(path: &Path) -> Result<Vec<ExifField>, String> {
    let file = File::open(path).map_err(|error| format!("打开图片失败：{error}"))?;
    let mut reader = BufReader::new(file);
    let exif = Reader::new()
        .read_from_container(&mut reader)
        .map_err(|error| format!("读取 EXIF 失败：{error}"))?;

    let mut fields = Vec::new();
    for field in exif.fields() {
        if field.ifd_num != In::PRIMARY && field.ifd_num != In::THUMBNAIL {
            continue;
        }
        fields.push(ExifField {
            tag: field.tag.to_string(),
            value: field.display_value().to_string(),
        });
    }
    Ok(fields)
}

/// Strip EXIF from a JPEG by rewriting file without APP1 (EXIF) segments.
/// For PNG/WebP, EXIF removal is not implemented (returns error).
pub fn strip_exif(input: &Path, output_dir: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(output_dir).map_err(|error| format!("创建输出目录失败：{error}"))?;

    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    if ext != "jpg" && ext != "jpeg" {
        return Err("目前仅支持 JPEG 清除 EXIF；PNG/WebP 请在格式转换 tab 转成 JPEG 后再清".to_string());
    }

    let mut file = File::open(input).map_err(|error| format!("打开图片失败：{error}"))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|error| format!("读取图片失败：{error}"))?;

    let cleaned = remove_jpeg_exif(&buf)?;

    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let mut idx = 0;
    let output_path = loop {
        let name = if idx == 0 {
            format!("{stem}_no_exif.jpg")
        } else {
            format!("{stem}_no_exif_{idx}.jpg")
        };
        let candidate = output_dir.join(name);
        if !candidate.exists() {
            break candidate;
        }
        idx += 1;
    };

    let out = File::create(&output_path).map_err(|error| format!("创建输出文件失败：{error}"))?;
    let mut writer = BufWriter::new(out);
    writer
        .write_all(&cleaned)
        .map_err(|error| format!("写出图片失败：{error}"))?;
    Ok(output_path)
}

/// Walk JPEG segments, drop APP1 (EXIF/XMP) segments, keep everything else.
fn remove_jpeg_exif(input: &[u8]) -> Result<Vec<u8>, String> {
    if input.len() < 2 || input[0] != 0xFF || input[1] != 0xD8 {
        return Err("不是有效的 JPEG 文件".to_string());
    }

    let mut out = Vec::with_capacity(input.len());
    out.extend_from_slice(&input[0..2]); // SOI

    let mut i = 2usize;
    while i + 1 < input.len() {
        if input[i] != 0xFF {
            // stream body, copy rest
            out.extend_from_slice(&input[i..]);
            break;
        }
        let marker = input[i + 1];
        // Standalone markers (no length)
        if matches!(marker, 0xD0..=0xD9) || marker == 0x01 {
            out.push(0xFF);
            out.push(marker);
            i += 2;
            continue;
        }
        // Segment with length
        if i + 3 >= input.len() {
            return Err("JPEG 数据不完整".to_string());
        }
        let length = ((input[i + 2] as usize) << 8) | (input[i + 3] as usize);
        if length < 2 || i + 2 + length > input.len() {
            return Err("JPEG segment 长度非法".to_string());
        }
        let is_exif_app1 = marker == 0xE1 && input.len() >= i + 10 && &input[i + 4..i + 10] == b"Exif\0\0";
        if !is_exif_app1 {
            out.extend_from_slice(&input[i..i + 2 + length]);
        }
        i += 2 + length;

        // On SOS (0xDA), following is entropy-coded image data — copy to end.
        if marker == 0xDA {
            out.extend_from_slice(&input[i..]);
            break;
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::remove_jpeg_exif;

    #[test]
    fn errors_on_non_jpeg() {
        let data = vec![0x89, 0x50, 0x4E, 0x47];
        assert!(remove_jpeg_exif(&data).is_err());
    }
}
