use axum::http::{HeaderMap, HeaderValue};
use findora_backend::audit::extract_client_info;

#[test]
fn test_extract_client_info_custom_headers() {
    let mut headers = HeaderMap::new();
    headers.insert("x-client-ip", HeaderValue::from_static("14.161.22.89"));
    headers.insert(
        "x-device-location",
        HeaderValue::from_bytes("Hồ Chí Minh, Việt Nam".as_bytes()).unwrap(),
    );
    headers.insert(
        "x-device-name",
        HeaderValue::from_bytes("iPhone 17 Pro".as_bytes()).unwrap(),
    );
    headers.insert("x-device-os", HeaderValue::from_static("iOS 18.2"));

    let info = extract_client_info(&headers);
    assert_eq!(info.ip, "14.161.22.89");
    assert_eq!(info.location, "Hồ Chí Minh, Việt Nam");
    assert_eq!(info.device_name, "iPhone 17 Pro");
    assert_eq!(info.device_os, "iOS 18.2");
}

#[test]
fn test_extract_client_info_cloudflare_and_ua() {
    let mut headers = HeaderMap::new();
    headers.insert("cf-connecting-ip", HeaderValue::from_static("113.160.18.45"));
    headers.insert("cf-ipcity", HeaderValue::from_static("Hanoi"));
    headers.insert("cf-ipcountry", HeaderValue::from_static("VN"));
    headers.insert(
        "user-agent",
        HeaderValue::from_static("Mozilla/5.0 (iPhone; CPU iPhone OS 18_2 like Mac OS X) AppleWebKit/605.1.15"),
    );

    let info = extract_client_info(&headers);
    assert_eq!(info.ip, "113.160.18.45");
    assert_eq!(info.location, "Hanoi, VN");
    assert_eq!(info.device_name, "iPhone");
    assert_eq!(info.device_os, "iOS 18.2");
}
