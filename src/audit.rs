use axum::{
    extract::Request,
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use tracing::info;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientAuditInfo {
    pub ip: String,
    pub location: String,
    pub device_name: String,
    pub device_os: String,
}

/// Trích xuất IP, Vị trí (Location), Tên thiết bị và Version iOS từ request headers
pub fn extract_client_info(headers: &HeaderMap) -> ClientAuditInfo {
    // 1. Lấy địa chỉ IP (Ưu tiên Cloudflare -> X-Forwarded-For -> X-Real-IP -> X-Client-IP)
    let ip = headers
        .get("cf-connecting-ip")
        .and_then(|h| h.to_str().ok())
        .or_else(|| {
            headers
                .get("x-forwarded-for")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.split(',').next()) // Lấy IP gốc trước các proxy
        })
        .or_else(|| headers.get("x-real-ip").and_then(|h| h.to_str().ok()))
        .or_else(|| headers.get("x-client-ip").and_then(|h| h.to_str().ok()))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    // 2. Lấy vị trí địa lý (Ưu tiên GPS từ Mobile Client -> Cloudflare Geo City & Country)
    let location = if let Some(h) = headers.get("x-device-location") {
        String::from_utf8_lossy(h.as_bytes()).trim().to_string()
    } else {
        let city = headers.get("cf-ipcity").map(|h| String::from_utf8_lossy(h.as_bytes()).trim().to_string());
        let country = headers.get("cf-ipcountry").map(|h| String::from_utf8_lossy(h.as_bytes()).trim().to_string());

        match (city, country) {
            (Some(c), Some(co)) => format!("{}, {}", c, co),
            (None, Some(co)) => co,
            _ => "Không xác định".to_string(),
        }
    };

    // 3. Tên thiết bị (Ưu tiên header X-Device-Name -> Phân tích User-Agent)
    let user_agent = headers
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    let device_name = if let Some(h) = headers.get("x-device-name") {
        String::from_utf8_lossy(h.as_bytes()).trim().to_string()
    } else {
        parse_device_from_ua(user_agent)
    };

    // 4. Phiên bản hệ điều hành & iOS (Ưu tiên header X-Device-OS -> Phân tích User-Agent)
    let device_os = if let Some(h) = headers.get("x-device-os") {
        String::from_utf8_lossy(h.as_bytes()).trim().to_string()
    } else if let Some(h) = headers.get("x-os-version") {
        format!("iOS {}", String::from_utf8_lossy(h.as_bytes()).trim())
    } else {
        parse_os_from_ua(user_agent)
    };

    ClientAuditInfo {
        ip,
        location,
        device_name,
        device_os,
    }
}

fn parse_device_from_ua(ua: &str) -> String {
    let lower = ua.to_lowercase();
    if lower.contains("iphone") {
        "iPhone".to_string()
    } else if lower.contains("ipad") {
        "iPad".to_string()
    } else if lower.contains("macintosh") || lower.contains("mac os") {
        "MacBook / Mac".to_string()
    } else if lower.contains("android") {
        "Android Device".to_string()
    } else if lower.contains("curl") {
        "cURL Client".to_string()
    } else if lower.contains("postman") {
        "Postman".to_string()
    } else {
        "Thiết bị không rõ".to_string()
    }
}

fn parse_os_from_ua(ua: &str) -> String {
    if let Some(pos) = ua.find("CPU iPhone OS ") {
        let sub = &ua[pos + 14..];
        if let Some(end) = sub.find(' ') {
            let ver = sub[..end].replace('_', ".");
            return format!("iOS {}", ver);
        }
    } else if let Some(pos) = ua.find("iPhone OS ") {
        let sub = &ua[pos + 10..];
        if let Some(end) = sub.find(' ') {
            let ver = sub[..end].replace('_', ".");
            return format!("iOS {}", ver);
        }
    } else if let Some(pos) = ua.find("iOS/") {
        let sub = &ua[pos + 4..];
        let end = sub.find(' ').unwrap_or(sub.len());
        return format!("iOS {}", &sub[..end]);
    } else if let Some(pos) = ua.find("Mac OS X ") {
        let sub = &ua[pos + 9..];
        if let Some(end) = sub.find(';') {
            let ver = sub[..end].replace('_', ".");
            return format!("macOS {}", ver);
        }
    } else if ua.to_lowercase().contains("darwin") {
        return "Apple Darwin Core".to_string();
    }
    "Không rõ phiên bản".to_string()
}

/// Middleware ghi nhận log chi tiết mỗi request của client tới hệ thống
pub async fn audit_log_middleware(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().path().to_string();
    let headers = req.headers();

    let client = extract_client_info(headers);

    info!(
        "📱 [CLIENT REQUEST] -> {} {} | IP: {} | Vị trí: {} | Thiết bị: {} | OS: {}",
        method, uri, client.ip, client.location, client.device_name, client.device_os
    );

    let start = Instant::now();
    let res = next.run(req).await;
    let duration = start.elapsed();

    info!(
        "⚡ [CLIENT RESPONSE] <- {} {} -> {} ({}ms) | IP: {} | Thiết bị: {}",
        method, uri, res.status(), duration.as_millis(), client.ip, client.device_name
    );

    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

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
}
