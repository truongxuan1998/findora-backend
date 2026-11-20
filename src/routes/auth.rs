use crate::auth::{create_jwt, AuthenticatedUser};
use crate::errors::{AppError, AppResult};
use crate::models::user::{AuthResponse, LoginRequest, RegisterRequest, UserResponse};
use crate::AppState;
use axum::{extract::State, http::StatusCode, Json};
use bcrypt::{hash, verify, DEFAULT_COST};
use sqlx::Row;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

pub async fn register_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RegisterRequest>,
) -> AppResult<(StatusCode, Json<AuthResponse>)> {
    let email = payload.email.trim().to_lowercase();
    let name = payload.full_name.trim();

    if email.is_empty() || !email.contains('@') {
        return Err(AppError::BadRequest("Email không đúng định dạng".into()));
    }

    if name.is_empty() {
        return Err(AppError::BadRequest("Họ và tên không được để trống".into()));
    }

    if payload.password.len() < 6 {
        return Err(AppError::BadRequest("Mật khẩu phải chứa ít nhất 6 ký tự".into()));
    }

    if payload.password.len() > 72 {
        return Err(AppError::BadRequest("Mật khẩu không được vượt quá 72 ký tự".into()));
    }

    let hashed_pw = hash(&payload.password, DEFAULT_COST)
        .map_err(|e| AppError::Internal(format!("Lỗi mã hóa mật khẩu: {}", e)))?;

    let user_id = format!("usr_{}", Uuid::new_v4().simple());
    let phone = payload.phone_number.unwrap_or_default().trim().to_string();

    // Thực hiện trong một Database Transaction để đảm bảo tính toàn vẹn (ACID)
    let mut tx = state.pool.begin().await.map_err(AppError::Database)?;

    // 1. Lưu thông tin người dùng vào findora.users
    let insert_res = sqlx::query(
        "INSERT INTO findora.users (id, email, full_name, phone_number, password_hash)
         VALUES ($1, $2, $3, $4, $5)"
    )
    .bind(&user_id)
    .bind(&email)
    .bind(name)
    .bind(&phone)
    .bind(&hashed_pw)
    .execute(&mut *tx)
    .await;

    if let Err(e) = insert_res {
        if let sqlx::Error::Database(ref db_err) = e {
            if db_err.code().as_deref() == Some("23505") {
                return Err(AppError::Conflict("Email này đã được đăng ký tài khoản Findora".into()));
            }
        }
        return Err(AppError::Database(e));
    }

    // 2. Tạo ví mặc định liên kết chính xác với user_id mới tạo (Data Integrity)
    let wallet_id = format!("wal_{}", Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO findora.wallets (id, user_id, name, currency, initial_balance, current_balance)
         VALUES ($1, $2, 'Ví tiền mặt', 'VND', 0, 0)"
    )
    .bind(&wallet_id)
    .bind(&user_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Database)?;

    tx.commit().await.map_err(AppError::Database)?;

    info!(user_id = %user_id, email = %email, "🎉 Đăng ký tài khoản và khởi tạo ví thành công");

    // Tạo token JWT có thời hạn 30 ngày
    let token = create_jwt(&user_id, &email, &state.config.jwt_secret, 30)?;

    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            success: true,
            message: "Đăng ký tài khoản thành công!".to_string(),
            token: Some(token),
            user: Some(UserResponse {
                id: user_id,
                email,
                full_name: name.to_string(),
                phone_number: phone,
            }),
        }),
    ))
}

pub async fn login_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> AppResult<Json<AuthResponse>> {
    let email = payload.email.trim().to_lowercase();

    let user_row = sqlx::query(
        "SELECT id, email, full_name, phone_number, password_hash 
         FROM findora.users 
         WHERE email = $1"
    )
    .bind(&email)
    .fetch_optional(&*state.pool)
    .await
    .map_err(AppError::Database)?;

    let row = match user_row {
        Some(r) => r,
        None => {
            warn!(email = %email, "Đăng nhập thất bại: Tài khoản không tồn tại");
            // Tránh user enumeration attack: trả về thông điệp chung
            return Err(AppError::Unauthorized("Email hoặc mật khẩu không chính xác".into()));
        }
    };

    let pw_hash: String = row.get("password_hash");
    let is_valid = verify(&payload.password, &pw_hash).unwrap_or(false);

    if !is_valid {
        warn!(email = %email, "Đăng nhập thất bại: Sai mật khẩu");
        return Err(AppError::Unauthorized("Email hoặc mật khẩu không chính xác".into()));
    }

    let user_id: String = row.get("id");
    let full_name: String = row.get("full_name");
    let phone_number: String = row.get("phone_number");

    info!(user_id = %user_id, email = %email, "🔐 Đăng nhập thành công, đang cấp JWT token");

    let token = create_jwt(&user_id, &email, &state.config.jwt_secret, 30)?;

    Ok(Json(AuthResponse {
        success: true,
        message: "Đăng nhập thành công".to_string(),
        token: Some(token),
        user: Some(UserResponse {
            id: user_id,
            email,
            full_name,
            phone_number,
        }),
    }))
}

pub async fn me_handler(
    State(state): State<Arc<AppState>>,
    auth: AuthenticatedUser,
) -> AppResult<Json<UserResponse>> {
    let row = sqlx::query(
        "SELECT id, email, full_name, phone_number 
         FROM findora.users 
         WHERE id = $1"
    )
    .bind(&auth.user_id)
    .fetch_optional(&*state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound("Người dùng không tồn tại".into()))?;

    Ok(Json(UserResponse {
        id: row.get("id"),
        email: row.get("email"),
        full_name: row.get("full_name"),
        phone_number: row.get("phone_number"),
    }))
}
