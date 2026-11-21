use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Service quản lý kết nối và thao tác với Redis (Dual-Engine: TCP ConnectionManager + Upstash REST Fallback)
#[derive(Clone)]
pub struct RedisService {
    manager: Option<ConnectionManager>,
    rest_url: Option<String>,
    rest_token: Option<String>,
    http_client: Client,
}

impl Default for RedisService {
    fn default() -> Self {
        Self {
            manager: None,
            rest_url: None,
            rest_token: None,
            http_client: Client::new(),
        }
    }
}

impl RedisService {
    /// Khởi tạo RedisService ở chế độ tắt (không dùng Redis, dùng cho unit test hoặc fallback)
    pub fn disabled() -> Self {
        Self::default()
    }

    pub async fn new(
        redis_url: Option<&str>,
        rest_url: Option<&str>,
        rest_token: Option<&str>,
    ) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_millis(1500))
            .connect_timeout(Duration::from_millis(1000))
            .build()
            .unwrap_or_default();

        let rest_url_owned = rest_url
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let rest_token_owned = rest_token
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let mut manager = None;

        if let Some(url) = redis_url.filter(|s| !s.trim().is_empty()) {
            match redis::Client::open(url.trim()) {
                Ok(client) => {
                    let connect_future = ConnectionManager::new(client);
                    match tokio::time::timeout(Duration::from_millis(1500), connect_future).await {
                        Ok(Ok(mut mgr)) => {
                            let ping_cmd = redis::cmd("PING");
                            let ping_future = ping_cmd.query_async::<String>(&mut mgr);
                            if let Ok(Ok(pong)) = tokio::time::timeout(Duration::from_millis(1500), ping_future).await {
                                info!("✅ Kết nối Upstash Redis (TCP Engine) thành công! (PING -> {})", pong);
                                manager = Some(mgr);
                            } else {
                                warn!("⚠️ Kết nối Redis TCP không phản hồi PING trong 1.5s. Kích hoạt Upstash REST Engine.");
                                manager = None;
                            }
                        }
                        Ok(Err(e)) => {
                            warn!("⚠️ Không thể kết nối Redis qua TCP ({}). Sử dụng Upstash REST Engine.", e);
                        }
                        Err(_) => {
                            warn!("⚠️ Kết nối Redis TCP timeout sau 1.5s. Sử dụng Upstash REST Engine.");
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Định dạng REDIS_URL không hợp lệ: {}", e);
                }
            }
        }

        if manager.is_none() && rest_url_owned.is_some() {
            info!("🚀 Đã kích hoạt Upstash Redis REST API Engine thành công!");
        }

        Self {
            manager,
            rest_url: rest_url_owned,
            rest_token: rest_token_owned,
            http_client,
        }
    }

    /// Kiểm tra xem Redis có được kích hoạt (qua TCP hoặc REST) hay không
    pub fn is_enabled(&self) -> bool {
        self.manager.is_some() || (self.rest_url.is_some() && self.rest_token.is_some())
    }

    /// Kiểm tra trạng thái sống của Redis (dùng cho health check)
    pub async fn ping(&self) -> bool {
        if let Some(mut mgr) = self.manager.clone() {
            let res: Result<String, _> = redis::cmd("PING").query_async(&mut mgr).await;
            if res.is_ok() {
                return true;
            }
        }

        if let (Some(url), Some(token)) = (&self.rest_url, &self.rest_token) {
            let ping_url = format!("{}/ping", url.trim_end_matches('/'));
            if let Ok(resp) = self
                .http_client
                .get(&ping_url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await
            {
                return resp.status().is_success();
            }
        }

        false
    }

    /// Lấy giá trị key từ Redis (TCP trước, nếu lỗi hoặc không có thì gọi REST)
    pub async fn get(&self, key: &str) -> Option<String> {
        if let Some(mut mgr) = self.manager.clone() {
            match mgr.get::<_, Option<String>>(key).await {
                Ok(Some(s)) => {
                    debug!(key = %key, "⚡ [REDIS TCP CACHE HIT]");
                    return Some(s);
                }
                Ok(None) => return None,
                Err(e) => {
                    debug!(key = %key, error = %e, "TCP get failed, attempting REST fallback");
                }
            }
        }

        if let (Some(url), Some(token)) = (&self.rest_url, &self.rest_token) {
            let cmd = serde_json::json!(["GET", key]);
            if let Ok(resp) = self
                .http_client
                .post(url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .json(&cmd)
                .send()
                .await
            {
                if resp.status().is_success() {
                    if let Ok(json) = resp.json::<Value>().await {
                        if let Some(Value::String(s)) = json.get("result") {
                            debug!(key = %key, "⚡ [REDIS REST CACHE HIT]");
                            return Some(s.clone());
                        }
                    }
                }
            }
        }

        None
    }

    /// Lưu giá trị có thời gian sống (TTL theo giây)
    pub async fn set_ex(&self, key: &str, value: &str, ttl_secs: u64) -> bool {
        if let Some(mut mgr) = self.manager.clone() {
            if mgr.set_ex::<_, _, ()>(key, value, ttl_secs).await.is_ok() {
                return true;
            }
        }

        if let (Some(url), Some(token)) = (&self.rest_url, &self.rest_token) {
            let cmd = serde_json::json!(["SET", key, value, "EX", ttl_secs]);
            if let Ok(resp) = self
                .http_client
                .post(url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .json(&cmd)
                .send()
                .await
            {
                return resp.status().is_success();
            }
        }

        false
    }

    /// Xóa một key (Cache Invalidation)
    pub async fn del(&self, key: &str) -> bool {
        if let Some(mut mgr) = self.manager.clone() {
            if mgr.del::<_, ()>(key).await.is_ok() {
                return true;
            }
        }

        if let (Some(url), Some(token)) = (&self.rest_url, &self.rest_token) {
            let cmd = serde_json::json!(["DEL", key]);
            if let Ok(resp) = self
                .http_client
                .post(url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .json(&cmd)
                .send()
                .await
            {
                return resp.status().is_success();
            }
        }

        false
    }

    /// Xóa toàn bộ key khớp với pattern (ví dụ: findora:cache:report:user_id:*)
    pub async fn del_pattern(&self, pattern: &str) -> bool {
        if let Some(mut mgr) = self.manager.clone() {
            let keys_res: Result<Vec<String>, _> = redis::cmd("KEYS").arg(pattern).query_async(&mut mgr).await;
            if let Ok(keys) = keys_res {
                if !keys.is_empty() {
                    let _: Result<(), _> = redis::cmd("DEL").arg(&keys).query_async(&mut mgr).await;
                }
                return true;
            }
        }

        if let (Some(url), Some(token)) = (&self.rest_url, &self.rest_token) {
            let cmd = serde_json::json!(["KEYS", pattern]);
            if let Ok(resp) = self
                .http_client
                .post(url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .json(&cmd)
                .send()
                .await
            {
                if resp.status().is_success() {
                    if let Ok(json) = resp.json::<Value>().await {
                        if let Some(keys_val) = json.get("result").and_then(|v| v.as_array()) {
                            let keys: Vec<String> = keys_val.iter().filter_map(|k| k.as_str().map(|s| s.to_string())).collect();
                            if !keys.is_empty() {
                                let mut del_cmd = vec![serde_json::json!("DEL")];
                                for k in keys {
                                    del_cmd.push(serde_json::json!(k));
                                }
                                let _ = self
                                    .http_client
                                    .post(url)
                                    .header("Authorization", format!("Bearer {}", token))
                                    .header("Content-Type", "application/json")
                                    .json(&del_cmd)
                                    .send()
                                    .await;
                            }
                        }
                    }
                }
            }
        }

        false
    }

    /// Tăng giá trị key lên 1 kèm TTL (dùng cho Distributed Rate Limiting)
    pub async fn incr(&self, key: &str, ttl_secs: u64) -> Option<i64> {
        if let Some(mut mgr) = self.manager.clone() {
            if let Ok(count) = mgr.incr::<_, _, i64>(key, 1).await {
                if count == 1 {
                    let _ = mgr.expire::<_, ()>(key, ttl_secs as i64).await;
                }
                return Some(count);
            }
        }

        if let (Some(url), Some(token)) = (&self.rest_url, &self.rest_token) {
            let cmd = serde_json::json!(["INCR", key]);
            if let Ok(resp) = self
                .http_client
                .post(url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .json(&cmd)
                .send()
                .await
            {
                if let Ok(json) = resp.json::<Value>().await {
                    if let Some(count) = json.get("result").and_then(|v| v.as_i64()) {
                        if count == 1 {
                            let exp_cmd = serde_json::json!(["EXPIRE", key, ttl_secs]);
                            let _ = self
                                .http_client
                                .post(url)
                                .header("Authorization", format!("Bearer {}", token))
                                .header("Content-Type", "application/json")
                                .json(&exp_cmd)
                                .send()
                                .await;
                        }
                        return Some(count);
                    }
                }
            }
        }

        None
    }
}
