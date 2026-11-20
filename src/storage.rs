use crate::config::Config;
use crate::errors::{AppError, AppResult};
use crate::image_validator::ValidatedImage;
use std::time::Duration;
use tracing::{error, info};

pub struct SupabaseStorage;

impl SupabaseStorage {
    /// Upload file ảnh avatar lên bucket của Supabase Storage qua REST API
    pub async fn upload_avatar(
        user_id: &str,
        image: &ValidatedImage,
        config: &Config,
    ) -> AppResult<String> {
        if config.supabase_key.is_empty() {
            return Err(AppError::BadRequest(
                "Chưa cấu hình SUPABASE_KEY / SUPABASE_SERVICE_KEY trên máy chủ để upload ảnh lên Supabase Storage.".into(),
            ));
        }

        // Đặt tên file theo user_id và timestamp để chống cache CDN/Mobile
        let timestamp = chrono::Utc::now().timestamp();
        let filename = format!("{}_{}.{}", user_id, timestamp, image.extension);
        let object_path = format!("avatars/{}", filename);

        let upload_url = format!(
            "{}/storage/v1/object/{}/{}",
            config.supabase_url.trim_end_matches('/'),
            config.supabase_bucket,
            object_path
        );

        info!(
            user_id = %user_id,
            upload_url = %upload_url,
            mime_type = %image.mime_type,
            size_bytes = %image.bytes.len(),
            "🚀 Đang tải ảnh avatar lên Supabase Storage..."
        );

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(12))
            .build()
            .map_err(|e| AppError::Internal(format!("Lỗi khởi tạo HTTP client: {}", e)))?;

        let res = client
            .post(&upload_url)
            .header("Authorization", format!("Bearer {}", config.supabase_key))
            .header("apikey", &config.supabase_key)
            .header("Content-Type", image.mime_type)
            .header("x-upsert", "true")
            .body(image.bytes.clone())
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Không thể kết nối tới Supabase Storage: {}", e)))?;

        let status = res.status();
        if !status.is_success() {
            let error_body = res.text().await.unwrap_or_default();
            error!(
                status = %status,
                error = %error_body,
                "❌ Supabase Storage từ chối upload avatar"
            );
            return Err(AppError::Internal(format!(
                "Lỗi lưu trữ Supabase Storage ({status}): {error_body}"
            )));
        }

        // Public URL chuẩn để ứng dụng di động truy cập trực tiếp
        let public_url = format!(
            "{}/storage/v1/object/public/{}/{}",
            config.supabase_url.trim_end_matches('/'),
            config.supabase_bucket,
            object_path
        );

        info!(user_id = %user_id, public_url = %public_url, "✅ Upload avatar lên Supabase Storage thành công");
        Ok(public_url)
    }
}
