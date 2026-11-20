use crate::auth::AuthenticatedUser;
use crate::errors::{AppError, AppResult};
use crate::i18n::{self, Language};
use crate::image_validator::{validate_image, MAX_IMAGE_SIZE};
use crate::storage::SupabaseStorage;
use crate::AppState;
use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::Arc;
use tracing::info;

#[derive(Serialize)]
pub struct AvatarUploadResponse {
    pub success: bool,
    pub message: String,
    pub avatar_url: String,
}

#[derive(Deserialize)]
pub struct UpdateProfileRequest {
    #[serde(alias = "fullName")]
    pub full_name: Option<String>,
    #[serde(alias = "phoneNumber")]
    pub phone_number: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct UserProfileResponse {
    pub id: String,
    pub email: String,
    pub full_name: String,
    pub phone_number: String,
    pub avatar_url: String,
}

/// Upload ảnh avatar thông qua Multipart Form-Data
pub async fn upload_avatar_handler(
    State(state): State<Arc<AppState>>,
    lang: Language,
    auth: AuthenticatedUser,
    mut multipart: Multipart,
) -> AppResult<(StatusCode, Json<AvatarUploadResponse>)> {
    let mut file_bytes: Option<Vec<u8>> = None;

    // Đọc trường file từ multipart form-data
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Lỗi đọc dữ liệu form-data: {}", e)))?
    {
        let name = field.name().unwrap_or_default();
        if name == "avatar" || name == "file" || name == "image" || file_bytes.is_none() {
            let data = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(format!("Lỗi đọc nội dung file: {}", e)))?;

            if data.len() > MAX_IMAGE_SIZE {
                return Err(AppError::BadRequest(
                    "Dung lượng ảnh vượt quá giới hạn 3MB".into(),
                ));
            }

            file_bytes = Some(data.to_vec());
            break;
        }
    }

    let bytes = file_bytes
        .ok_or_else(|| AppError::BadRequest("Vui lòng đính kèm file ảnh trong form-data (key: 'avatar' hoặc 'file')".into()))?;

    // 1. Kiểm tra tính hợp lệ của ảnh (Magic Bytes, dimensions, security checks)
    let validated_image = validate_image(bytes)?;

    // 2. Upload lên Supabase Storage
    let avatar_url = SupabaseStorage::upload_avatar(&auth.user_id, &validated_image, &state.config).await?;

    // 3. Cập nhật thông tin avatar_url trong database
    sqlx::query("UPDATE findora.users SET avatar_url = $1, updated_at = NOW() WHERE id = $2")
        .bind(&avatar_url)
        .bind(&auth.user_id)
        .execute(&*state.pool)
        .await
        .map_err(AppError::Database)?;

    // Xóa cache User Profile trong Redis (Cache Invalidation)
    state
        .redis
        .del(&format!("findora:cache:user:{}:profile", auth.user_id))
        .await;

    info!(user_id = %auth.user_id, avatar_url = %avatar_url, "🎉 Đã cập nhật ảnh đại diện thành công vào database");

    Ok((
        StatusCode::OK,
        Json(AvatarUploadResponse {
            success: true,
            message: i18n::msg_avatar_upload_success(lang).to_string(),
            avatar_url,
        }),
    ))
}

pub async fn get_profile_handler(
    State(state): State<Arc<AppState>>,
    lang: Language,
    auth: AuthenticatedUser,
) -> AppResult<Json<UserProfileResponse>> {
    let cache_key = format!("findora:cache:user:{}:profile", auth.user_id);

    // 1. Thử lấy Profile từ Redis Cache (TTL 30 phút)
    if let Some(cached_str) = state.redis.get(&cache_key).await {
        if let Ok(profile) = serde_json::from_str::<UserProfileResponse>(&cached_str) {
            return Ok(Json(profile));
        }
    }

    // 2. Cache Miss -> Truy vấn PostgreSQL
    let row = sqlx::query(
        "SELECT id, email, full_name, phone_number, avatar_url 
         FROM findora.users 
         WHERE id = $1"
    )
    .bind(&auth.user_id)
    .fetch_optional(&*state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound(i18n::msg_user_not_found(lang).into()))?;

    let profile = UserProfileResponse {
        id: row.get("id"),
        email: row.get("email"),
        full_name: row.get("full_name"),
        phone_number: row.get("phone_number"),
        avatar_url: row.get("avatar_url"),
    };

    // 3. Ghi vào Redis Cache với TTL 30 phút (1800 giây)
    if let Ok(json_str) = serde_json::to_string(&profile) {
        state.redis.set_ex(&cache_key, &json_str, 1800).await;
    }

    Ok(Json(profile))
}

pub async fn update_profile_handler(
    State(state): State<Arc<AppState>>,
    lang: Language,
    auth: AuthenticatedUser,
    Json(payload): Json<UpdateProfileRequest>,
) -> AppResult<Json<UserProfileResponse>> {
    if let Some(ref name) = payload.full_name {
        if name.trim().is_empty() {
            return Err(AppError::BadRequest(i18n::msg_empty_name(lang).into()));
        }
    }

    let row = sqlx::query(
        "UPDATE findora.users 
         SET full_name = COALESCE($1, full_name),
             phone_number = COALESCE($2, phone_number),
             updated_at = NOW()
         WHERE id = $3
         RETURNING id, email, full_name, phone_number, avatar_url"
    )
    .bind(payload.full_name.map(|n| n.trim().to_string()))
    .bind(payload.phone_number.map(|p| p.trim().to_string()))
    .bind(&auth.user_id)
    .fetch_one(&*state.pool)
    .await
    .map_err(AppError::Database)?;

    let res = UserProfileResponse {
        id: row.get("id"),
        email: row.get("email"),
        full_name: row.get("full_name"),
        phone_number: row.get("phone_number"),
        avatar_url: row.get("avatar_url"),
    };

    // Xóa cache User Profile trong Redis (Cache Invalidation)
    state
        .redis
        .del(&format!("findora:cache:user:{}:profile", auth.user_id))
        .await;

    Ok(Json(res))
}
