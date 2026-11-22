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

    // 3. Tên thiết bị (Ưu tiên: X-Device-Model -> X-Device-Name -> Phân tích User-Agent)
    let user_agent = headers
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    let model_hdr = headers
        .get("x-device-model")
        .map(|h| String::from_utf8_lossy(h.as_bytes()).trim().to_string());
    let name_hdr = headers
        .get("x-device-name")
        .map(|h| String::from_utf8_lossy(h.as_bytes()).trim().to_string());

    let device_name = if let Some(ref model) = model_hdr {
        if let Some(mapped) = map_apple_model_identifier(model) {
            mapped.to_string()
        } else if let Some(ref name) = name_hdr {
            if name != "iPhone" && !name.is_empty() {
                name.clone()
            } else {
                model.clone()
            }
        } else {
            model.clone()
        }
    } else if let Some(ref name) = name_hdr {
        if let Some(mapped) = map_apple_model_identifier(name) {
            mapped.to_string()
        } else {
            name.clone()
        }
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

/// Ánh xạ mã định danh phần cứng của Apple (Apple Machine Identifier) sang tên thương mại chi tiết
pub fn map_apple_model_identifier(raw: &str) -> Option<&'static str> {
    let trimmed = raw.trim();
    match trimmed {
        // iPhone 16 series
        "iPhone17,1" => Some("iPhone 16 Pro"),
        "iPhone17,2" => Some("iPhone 16 Pro Max"),
        "iPhone17,3" => Some("iPhone 16"),
        "iPhone17,4" => Some("iPhone 16 Plus"),

        // iPhone 15 series
        "iPhone16,1" => Some("iPhone 15 Pro"),
        "iPhone16,2" => Some("iPhone 15 Pro Max"),
        "iPhone15,4" => Some("iPhone 15"),
        "iPhone15,5" => Some("iPhone 15 Plus"),

        // iPhone 14 series
        "iPhone15,2" => Some("iPhone 14 Pro"),
        "iPhone15,3" => Some("iPhone 14 Pro Max"),
        "iPhone14,7" => Some("iPhone 14"),
        "iPhone14,8" => Some("iPhone 14 Plus"),

        // iPhone SE (3rd Gen)
        "iPhone14,6" => Some("iPhone SE (3rd Gen)"),

        // iPhone 13 series
        "iPhone14,2" => Some("iPhone 13 Pro"),
        "iPhone14,3" => Some("iPhone 13 Pro Max"),
        "iPhone14,4" => Some("iPhone 13 mini"),
        "iPhone14,5" => Some("iPhone 13"),

        // iPhone 12 series
        "iPhone13,1" => Some("iPhone 12 mini"),
        "iPhone13,2" => Some("iPhone 12"),
        "iPhone13,3" => Some("iPhone 12 Pro"),
        "iPhone13,4" => Some("iPhone 12 Pro Max"),

        // iPhone SE (2nd Gen)
        "iPhone12,8" => Some("iPhone SE (2nd Gen)"),

        // iPhone 11 series
        "iPhone12,1" => Some("iPhone 11"),
        "iPhone12,3" => Some("iPhone 11 Pro"),
        "iPhone12,5" => Some("iPhone 11 Pro Max"),

        // iPhone XS / XR / X series
        "iPhone11,2" => Some("iPhone XS"),
        "iPhone11,4" | "iPhone11,6" => Some("iPhone XS Max"),
        "iPhone11,8" => Some("iPhone XR"),
        "iPhone10,3" | "iPhone10,6" => Some("iPhone X"),

        // iPhone 8 series
        "iPhone10,1" | "iPhone10,4" => Some("iPhone 8"),
        "iPhone10,2" | "iPhone10,5" => Some("iPhone 8 Plus"),

        // iPhone 7 / 6s / SE 1st series
        "iPhone9,1" | "iPhone9,3" => Some("iPhone 7"),
        "iPhone9,2" | "iPhone9,4" => Some("iPhone 7 Plus"),
        "iPhone8,1" => Some("iPhone 6s"),
        "iPhone8,2" => Some("iPhone 6s Plus"),
        "iPhone8,4" => Some("iPhone SE (1st Gen)"),

        // Simulators
        "arm64" | "x86_64" | "i386" => Some("iOS Simulator"),

        _ => None,
    }
}

fn parse_device_from_ua(ua: &str) -> String {
    // 1. Kiểm tra mã định danh phần cứng trực tiếp trong User-Agent (e.g. iPhone16,1 hoặc iPhone17,2)
    for part in ua.split([' ', ';', '(', ')', '/', '_']) {
        let trimmed = part.trim();
        if trimmed.starts_with("iPhone") && trimmed.contains(',') {
            if let Some(mapped) = map_apple_model_identifier(trimmed) {
                return mapped.to_string();
            }
        }
    }

    let lower = ua.to_lowercase();

    // 2. Kiểm tra tên thương mại cụ thể của iPhone trong User-Agent
    if lower.contains("iphone 16 pro max") {
        "iPhone 16 Pro Max".to_string()
    } else if lower.contains("iphone 16 pro") {
        "iPhone 16 Pro".to_string()
    } else if lower.contains("iphone 16 plus") {
        "iPhone 16 Plus".to_string()
    } else if lower.contains("iphone 16") {
        "iPhone 16".to_string()
    } else if lower.contains("iphone 15 pro max") {
        "iPhone 15 Pro Max".to_string()
    } else if lower.contains("iphone 15 pro") {
        "iPhone 15 Pro".to_string()
    } else if lower.contains("iphone 15 plus") {
        "iPhone 15 Plus".to_string()
    } else if lower.contains("iphone 15") {
        "iPhone 15".to_string()
    } else if lower.contains("iphone 14 pro max") {
        "iPhone 14 Pro Max".to_string()
    } else if lower.contains("iphone 14 pro") {
        "iPhone 14 Pro".to_string()
    } else if lower.contains("iphone 14 plus") {
        "iPhone 14 Plus".to_string()
    } else if lower.contains("iphone 14") {
        "iPhone 14".to_string()
    } else if lower.contains("iphone 13 pro max") {
        "iPhone 13 Pro Max".to_string()
    } else if lower.contains("iphone 13 pro") {
        "iPhone 13 Pro".to_string()
    } else if lower.contains("iphone 13 mini") {
        "iPhone 13 mini".to_string()
    } else if lower.contains("iphone 13") {
        "iPhone 13".to_string()
    } else if lower.contains("iphone 12 pro max") {
        "iPhone 12 Pro Max".to_string()
    } else if lower.contains("iphone 12 pro") {
        "iPhone 12 Pro".to_string()
    } else if lower.contains("iphone 12 mini") {
        "iPhone 12 mini".to_string()
    } else if lower.contains("iphone 12") {
        "iPhone 12".to_string()
    } else if lower.contains("iphone 11 pro max") {
        "iPhone 11 Pro Max".to_string()
    } else if lower.contains("iphone 11 pro") {
        "iPhone 11 Pro".to_string()
    } else if lower.contains("iphone 11") {
        "iPhone 11".to_string()
    } else if lower.contains("iphone xs max") {
        "iPhone XS Max".to_string()
    } else if lower.contains("iphone xs") {
        "iPhone XS".to_string()
    } else if lower.contains("iphone xr") {
        "iPhone XR".to_string()
    } else if lower.contains("iphone x") {
        "iPhone X".to_string()
    } else if lower.contains("iphone se") {
        "iPhone SE".to_string()
    } else if lower.contains("iphone") {
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

