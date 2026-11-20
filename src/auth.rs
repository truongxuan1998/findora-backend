use crate::errors::AppError;
use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // user_id
    pub email: String,
    pub exp: usize,
    pub iat: usize,
}

pub fn create_jwt(user_id: &str, email: &str, secret: &str, valid_days: i64) -> Result<String, AppError> {
    let now = chrono::Utc::now();
    let exp = (now + chrono::Duration::days(valid_days)).timestamp() as usize;
    let iat = now.timestamp() as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_string(),
        exp,
        iat,
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

        Ok(AuthenticatedUser {
            user_id: claims.sub,
            email: claims.email,
        })
    }
}

#[derive(Debug, Clone)]
pub struct OptionalUser(pub Option<AuthenticatedUser>);

#[async_trait]
impl<S> FromRequestParts<S> for OptionalUser
where
    S: Send + Sync,
    Arc<crate::AppState>: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = Arc::<crate::AppState>::from_ref(state);

        let maybe_auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok());

        if let Some(auth_header) = maybe_auth_header {
            if let Some(token) = auth_header.strip_prefix("Bearer ") {
                if let Ok(claims) = verify_jwt(token, &app_state.config.jwt_secret) {
                    return Ok(OptionalUser(Some(AuthenticatedUser {
                        user_id: claims.sub,
                        email: claims.email,
                    })));
                }
            }
        }

        Ok(OptionalUser(None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_create_and_verify() {
        let secret = "test_super_secret_jwt_key_123456789";
        let user_id = "usr_1234567890abcdef";
        let email = "test@findora.app";

        let token = create_jwt(user_id, email, secret, 1).expect("Failed to create JWT");
        assert!(!token.is_empty());

        let claims = verify_jwt(&token, secret).expect("Failed to verify JWT");
        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.email, email);
    }

    #[test]
    fn test_jwt_invalid_secret() {
        let secret = "test_super_secret_jwt_key_123456789";
        let wrong_secret = "wrong_secret_key_987654321";
        let token = create_jwt("usr_1", "test@findora.app", secret, 1).unwrap();

        let result = verify_jwt(&token, wrong_secret);
        assert!(result.is_err());
    }
}
