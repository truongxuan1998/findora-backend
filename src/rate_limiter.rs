use crate::audit::extract_client_info;
use crate::i18n::Language;
use crate::AppState;
use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderValue, Request, Response, StatusCode},
    middleware::Next,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::warn;

#[derive(Clone, Debug)]
struct RateLimitEntry {
    count: u32,
    reset_at: u64,
}

#[derive(Clone)]
pub struct IpRateLimiter {
    state: Arc<RwLock<HashMap<String, RateLimitEntry>>>,
    request_counter: Arc<AtomicUsize>,
}

#[derive(Debug, Clone)]
pub struct RateLimitResult {
    pub limit: u32,
    pub remaining: u32,
    pub reset_at: u64,
}

#[derive(Debug, Clone)]
pub struct RateLimitExceeded {
    pub limit: u32,
    pub retry_after_secs: u64,
}

impl IpRateLimiter {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(HashMap::new())),
            request_counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Kiểm tra và tăng số lượng request của một IP trong khung thời gian
    pub async fn check(
        &self,
        ip: &str,
        limit: u32,
        window_secs: u64,
    ) -> Result<RateLimitResult, RateLimitExceeded> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();

        // Định kỳ mỗi 1.000 requests thì dọn dẹp các IP đã hết hạn để tiết kiệm RAM
        if self.request_counter.fetch_add(1, Ordering::Relaxed).is_multiple_of(1000) {
            let state_clone = self.state.clone();
            tokio::spawn(async move {
                let mut map = state_clone.write().await;
                map.retain(|_, entry| entry.reset_at > now);
            });
        }

        let mut map = self.state.write().await;
        let entry = map.entry(ip.to_string()).or_insert(RateLimitEntry {
            count: 0,
            reset_at: now + window_secs,
        });

        // Nếu khung thời gian cũ đã hết hạn, reset lại chu kỳ mới
        if now >= entry.reset_at {
            entry.count = 1;
            entry.reset_at = now + window_secs;
            return Ok(RateLimitResult {
                limit,
                remaining: limit.saturating_sub(1),
                reset_at: entry.reset_at,
            });
        }

        // Kiểm tra xem có vượt quá giới hạn không
        if entry.count >= limit {
            let retry_after = entry.reset_at.saturating_sub(now).max(1);
            return Err(RateLimitExceeded {
                limit,
                retry_after_secs: retry_after,
            });
        }

        entry.count += 1;
        let remaining = limit.saturating_sub(entry.count);

        Ok(RateLimitResult {
            limit,
            remaining,
            reset_at: entry.reset_at,
        })
    }
}

impl Default for IpRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL_LIMITER: std::sync::OnceLock<IpRateLimiter> = std::sync::OnceLock::new();

pub fn get_global_limiter() -> &'static IpRateLimiter {
    GLOBAL_LIMITER.get_or_init(IpRateLimiter::new)
}

/// Middleware kiểm soát tần suất request (Rate Limiting) chống DDoS theo từng cấp độ API
pub async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Response<Body> {
    let path = req.uri().path();

    // 1. Ngoại lệ: Health check và Root ping không bao giờ bị chặn để Render/Uptime Monitor kiểm tra sống
    if path == "/" || path == "/health" {
        return next.run(req).await;
    }

    let headers = req.headers();
    let client_info = extract_client_info(headers);
    let ip = client_info.ip;

    // Xác định ngôn ngữ từ header Accept-Language
    let lang = match headers.get(header::ACCEPT_LANGUAGE) {
        Some(val) => Language::from_header_value(val.to_str().unwrap_or_default()),
        None => Language::Vi,
    };

    // 2. Phân cấp mức độ Rate Limit theo Endpoint
    // - Nhóm nhạy cảm (Auth, Forgot Password, Reset Password, Avatar upload): Tối đa 20 req / phút
    // - Nhóm API chung (Giao dịch, Ví, Danh mục, Báo cáo): Tối đa 120 req / phút (thừa đủ cho app, chặn triệt để DDoS)
    let (tier, limit, window_secs) = if path.starts_with("/api/auth/") || path == "/api/users/avatar" {
        ("auth", 20u32, 60u64)
    } else {
        ("general", 120u32, 60u64)
    };

    // 3. Ưu tiên rate limit PHÂN TÁN qua Redis (chính xác khi scale nhiều instance);
    //    tự động fallback về bộ đếm in-memory nếu Redis không khả dụng.
    let outcome: Result<RateLimitResult, RateLimitExceeded> = {
        let rl_key = format!("findora:rl:{}:{}", tier, ip);
        if let Some(count) = state.redis.incr(&rl_key, window_secs).await {
            let count = count.max(0) as u32;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs();
            if count > limit {
                Err(RateLimitExceeded {
                    limit,
                    retry_after_secs: window_secs,
                })
            } else {
                Ok(RateLimitResult {
                    limit,
                    remaining: limit.saturating_sub(count),
                    reset_at: now + window_secs,
                })
            }
        } else {
            get_global_limiter().check(&ip, limit, window_secs).await
        }
    };

    match outcome {
        Ok(result) => {
            let mut response = next.run(req).await;

            // Đính kèm các Header RateLimit chuẩn RFC
            let headers_mut = response.headers_mut();
            if let Ok(v) = HeaderValue::from_str(&result.limit.to_string()) {
                headers_mut.insert("X-RateLimit-Limit", v);
            }
            if let Ok(v) = HeaderValue::from_str(&result.remaining.to_string()) {
                headers_mut.insert("X-RateLimit-Remaining", v);
            }
            if let Ok(v) = HeaderValue::from_str(&result.reset_at.to_string()) {
                headers_mut.insert("X-RateLimit-Reset", v);
            }

            response
        }
        Err(exceeded) => {
            warn!(
                ip = %ip,
                path = %path,
                limit = exceeded.limit,
                retry_after = exceeded.retry_after_secs,
                "🚨 [RATE LIMIT / DDOS DEFENSE] Chặn request do vượt ngưỡng cho phép"
            );

            let msg = match lang {
                Language::Vi => format!(
                    "Bạn đang gửi yêu cầu quá nhanh. Vui lòng thử lại sau {} giây.",
                    exceeded.retry_after_secs
                ),
                Language::En => format!(
                    "Too many requests. Please try again in {} second(s).",
                    exceeded.retry_after_secs
                ),
            };

            let body = json!({
                "success": false,
                "code": "TOO_MANY_REQUESTS",
                "message": msg,
                "retry_after_seconds": exceeded.retry_after_secs
            });

            let mut response = (StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response();
            let headers_mut = response.headers_mut();
            if let Ok(v) = HeaderValue::from_str(&exceeded.retry_after_secs.to_string()) {
                headers_mut.insert(header::RETRY_AFTER, v);
            }
            if let Ok(v) = HeaderValue::from_str(&exceeded.limit.to_string()) {
                headers_mut.insert("X-RateLimit-Limit", v);
            }
            if let Ok(v) = HeaderValue::from_str("0") {
                headers_mut.insert("X-RateLimit-Remaining", v);
            }

            response
        }
    }
}
