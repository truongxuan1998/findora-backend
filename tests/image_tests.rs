use findora_backend::image_validator::{validate_image, MAX_IMAGE_SIZE};

#[test]
fn test_valid_png() {
    let mut png = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]; // PNG Header
    png.extend_from_slice(&[0x00, 0x00, 0x00, 0x0D]); // IHDR chunk length = 13
    png.extend_from_slice(b"IHDR");
    png.extend_from_slice(&[0x00, 0x00, 0x00, 0xC8]); // Width = 200
    png.extend_from_slice(&[0x00, 0x00, 0x00, 0xC8]); // Height = 200
    png.resize(120, 0);

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
