use crate::errors::AppError;
use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Thời hạn mặc định của access token (ngày).
pub const TOKEN_VALID_DAYS: i64 = 30;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // user_id
    pub email: String,
    pub exp: usize,
    pub iat: usize,
    /// Phiên bản token của user (`findora.users.token_version`) tại thời điểm cấp.
    /// Token cũ (trước khi có claim này) được coi là version 0 để không bị đăng xuất hàng loạt.
    #[serde(default)]
    pub ver: i32,
}

/// Tạo JWT version 0. Giữ lại để tương thích; luồng đăng nhập dùng `create_versioned_jwt`.
pub fn create_jwt(user_id: &str, email: &str, secret: &str, valid_days: i64) -> Result<String, AppError> {
    create_versioned_jwt(user_id, email, 0, secret, valid_days)
}

/// Tạo JWT gắn với `token_version` hiện tại của user. Khi version trong DB tăng
/// (đặt lại mật khẩu, đăng xuất mọi thiết bị), mọi token cấp trước đó bị từ chối.
pub fn create_versioned_jwt(
    user_id: &str,
    email: &str,
    token_version: i32,
    secret: &str,
    valid_days: i64,
) -> Result<String, AppError> {
    let now = chrono::Utc::now();
    let exp = (now + chrono::Duration::days(valid_days)).timestamp() as usize;
    let iat = now.timestamp() as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_string(),
        exp,
        iat,
        ver: token_version,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("Lỗi tạo token JWT: {}", e)))
}

pub fn verify_jwt(token: &str, secret: &str) -> Result<Claims, AppError> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| AppError::Unauthorized(format!("Token không hợp lệ hoặc đã hết hạn: {}", e)))?;

    Ok(token_data.claims)
}

/// Lỗi trả về khi token đúng chữ ký nhưng đã bị thu hồi (version cũ hoặc user không còn tồn tại).
pub fn revoked_token_error() -> AppError {
    AppError::Unauthorized("Phiên đăng nhập đã bị thu hồi. Vui lòng đăng nhập lại.".into())
}

/// So khớp version trong token với version hiện tại trong DB (`None` = user không còn tồn tại).
pub fn check_token_version(claims: &Claims, current_version: Option<i32>) -> Result<(), AppError> {
    match current_version {
        Some(v) if v == claims.ver => Ok(()),
        _ => Err(revoked_token_error()),
    }
}

/// Đọc `token_version` hiện tại của user. Trả `None` nếu user đã bị xoá.
pub async fn fetch_token_version(pool: &sqlx::PgPool, user_id: &str) -> Result<Option<i32>, AppError> {
    sqlx::query_scalar::<_, i32>("SELECT token_version FROM findora.users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::Database)
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub email: String,
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
    Arc<crate::AppState>: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = Arc::<crate::AppState>::from_ref(state);

        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("Yêu cầu đăng nhập: Thiếu Authorization header".into()))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::Unauthorized("Định dạng Authorization phải là 'Bearer <token>'".into()))?;

        let claims = verify_jwt(token, &app_state.config.jwt_secret)?;

        // Token đúng chữ ký vẫn có thể đã bị thu hồi (đặt lại mật khẩu / đăng xuất mọi thiết bị)
        let current_version = fetch_token_version(&app_state.pool, &claims.sub).await?;
        check_token_version(&claims, current_version)?;

        Ok(AuthenticatedUser {
            user_id: claims.sub,
            email: claims.email,
        })
    }
}

