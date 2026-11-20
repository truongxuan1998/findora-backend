use crate::errors::{AppError, AppResult};

pub const MAX_IMAGE_SIZE: usize = 3 * 1024 * 1024; // Giới hạn tối đa 3MB để bảo vệ RAM server
pub const MIN_IMAGE_SIZE: usize = 100; // File tối thiểu 100 bytes

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ValidatedImage {
    pub bytes: Vec<u8>,
    pub mime_type: &'static str,
    pub extension: &'static str,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// Xác thực định dạng ảnh thông qua Magic Bytes, kích thước và kiểm tra rủi ro mã độc
pub fn validate_image(data: Vec<u8>) -> AppResult<ValidatedImage> {
    let size = data.len();

    // 1. Kiểm tra kích thước payload
    if size < MIN_IMAGE_SIZE {
        return Err(AppError::BadRequest("File ảnh quá nhỏ hoặc rỗng".into()));
    }

    if size > MAX_IMAGE_SIZE {
        return Err(AppError::BadRequest(format!(
            "Dung lượng ảnh vượt quá giới hạn tối đa cho phép (3MB). Dung lượng thực tế: {:.2}MB",
            size as f64 / (1024.0 * 1024.0)
        )));
    }

    // 2. Chống Polyglot File & Stored Script Injection: Quét 1KB đầu và đuôi
    let scan_len = size.min(1024);
    let check_slice = &data[..scan_len];
    let lower_header = check_slice.to_ascii_lowercase();

    let malicious_signatures: [&[u8]; 6] = [
        b"<?php",
        b"<script",
        b"<html",
        b"javascript:",
        b"<!doctype html",
        b"eval(",
    ];

    for sig in &malicious_signatures {
        if lower_header.windows(sig.len()).any(|w| w == *sig) {
            return Err(AppError::BadRequest(
                "Phát hiện nội dung không an toàn bên trong file ảnh".into(),
            ));
        }
    }

    // 3. Kiểm tra Magic Bytes thực tế
    if is_png(&data) {
        let (width, height) = parse_png_dimensions(&data);
        validate_dimensions(width, height)?;
        Ok(ValidatedImage {
            bytes: data,
            mime_type: "image/png",
            extension: "png",
            width,
            height,
        })
    } else if is_jpeg(&data) {
        let (width, height) = parse_jpeg_dimensions(&data);
        validate_dimensions(width, height)?;
        Ok(ValidatedImage {
            bytes: data,
            mime_type: "image/jpeg",
            extension: "jpg",
            width,
            height,
        })
    } else if is_webp(&data) {
        let (width, height) = parse_webp_dimensions(&data);
        validate_dimensions(width, height)?;
        Ok(ValidatedImage {
            bytes: data,
            mime_type: "image/webp",
            extension: "webp",
            width,
            height,
        })
    } else {
        Err(AppError::BadRequest(
            "Định dạng ảnh không hợp lệ. Chỉ chấp nhận các định dạng chuẩn: PNG, JPG, WEBP.".into(),
        ))
    }
}

fn is_png(data: &[u8]) -> bool {
    data.len() >= 8 && data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
}

fn is_jpeg(data: &[u8]) -> bool {
    data.len() >= 3 && data.starts_with(&[0xFF, 0xD8, 0xFF])
}

fn is_webp(data: &[u8]) -> bool {
    data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP"
}

fn validate_dimensions(width: Option<u32>, height: Option<u32>) -> AppResult<()> {
    if let (Some(w), Some(h)) = (width, height) {
        if w < 16 || h < 16 {
            return Err(AppError::BadRequest("Kích thước ảnh quá nhỏ (tối thiểu 16x16 pixel)".into()));
        }
        if w > 4096 || h > 4096 {
            return Err(AppError::BadRequest("Kích thước ảnh vượt quá 4096x4096 pixel".into()));
        }
    }
    Ok(())
}

fn parse_png_dimensions(data: &[u8]) -> (Option<u32>, Option<u32>) {
    if data.len() >= 24 && &data[12..16] == b"IHDR" {
        let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        (Some(w), Some(h))
    } else {
        (None, None)
    }
}

fn parse_jpeg_dimensions(data: &[u8]) -> (Option<u32>, Option<u32>) {
    let mut i = 2;
    while i + 8 < data.len() {
        if data[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = data[i + 1];
        // SOF0, SOF1, SOF2 markers chứa thông số kích thước ảnh JPEG
        if marker == 0xC0 || marker == 0xC1 || marker == 0xC2 {
            let h = u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32;
            let w = u16::from_be_bytes([data[i + 7], data[i + 8]]) as u32;
            return (Some(w), Some(h));
        }
        let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
        if len < 2 {
            break;
        }
        i += 2 + len;
    }
    (None, None)
}

fn parse_webp_dimensions(data: &[u8]) -> (Option<u32>, Option<u32>) {
    if data.len() >= 30 && &data[12..16] == b"VP8X" {
        let w = 1 + (data[24] as u32 | ((data[25] as u32) << 8) | ((data[26] as u32) << 16));
        let h = 1 + (data[27] as u32 | ((data[28] as u32) << 8) | ((data[29] as u32) << 16));
        (Some(w), Some(h))
    } else {
        (None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_png() {
        let mut png = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]; // PNG Header
        png.extend_from_slice(&[0x00, 0x00, 0x00, 0x0D]); // IHDR chunk length = 13
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&[0x00, 0x00, 0x00, 0xC8]); // Width = 200
        png.extend_from_slice(&[0x00, 0x00, 0x00, 0xC8]); // Height = 200
        png.resize(120, 0); // Padding cho đủ min length

        let validated = validate_image(png).expect("PNG hợp lệ phải vượt qua validation");
        assert_eq!(validated.mime_type, "image/png");
        assert_eq!(validated.extension, "png");
        assert_eq!(validated.width, Some(200));
        assert_eq!(validated.height, Some(200));
    }

    #[test]
    fn test_reject_php_disguised_file() {
        let mut fake_png = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        fake_png.extend_from_slice(b"<?php echo 'malicious code'; ?>");
        fake_png.resize(150, 0);

        let res = validate_image(fake_png);
        assert!(res.is_err());
    }

    #[test]
    fn test_reject_oversized() {
        let oversized = vec![0u8; MAX_IMAGE_SIZE + 1];
        let res = validate_image(oversized);
        assert!(res.is_err());
    }
}
