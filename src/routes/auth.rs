use crate::auth::{create_jwt, AuthenticatedUser};
use crate::errors::{AppError, AppResult};
use crate::i18n::{self, Language};
use crate::models::user::{AuthResponse, LoginRequest, RegisterRequest, UserResponse};
use crate::AppState;
use axum::{extract::State, http::StatusCode, Json};
use bcrypt::{hash, verify, DEFAULT_COST};
use sqlx::Row;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

/// Kiểm tra độ phức tạp của mật khẩu: ít nhất 8 ký tự, có số, có ký tự đặc biệt, không quá 72 ký tự
pub fn validate_password(password: &str, lang: Language) -> AppResult<()> {
    if password.len() < 8 {
        return Err(AppError::BadRequest(i18n::msg_password_min_length(lang).into()));
    }

    if password.len() > 72 {
        return Err(AppError::BadRequest(i18n::msg_password_max_length(lang).into()));
    }

    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password.chars().any(|c| !c.is_alphanumeric());

    if !has_digit {
        return Err(AppError::BadRequest(i18n::msg_password_need_digit(lang).into()));
    }

    if !has_special {
        return Err(AppError::BadRequest(i18n::msg_password_need_special(lang).into()));
    }

    Ok(())
}

pub async fn register_handler(
    State(state): State<Arc<AppState>>,
    lang: Language,
    Json(payload): Json<RegisterRequest>,
) -> AppResult<(StatusCode, Json<AuthResponse>)> {
    let email = payload.email.trim().to_lowercase();
    let name = payload.full_name.trim();

    if email.is_empty() || !email.contains('@') {
        return Err(AppError::BadRequest(i18n::msg_invalid_email(lang).into()));
    }

    if name.is_empty() {
        return Err(AppError::BadRequest(i18n::msg_empty_name(lang).into()));
    }

    // Nếu client gửi confirm_password, kiểm tra trùng khớp
    if let Some(ref confirm_pw) = payload.confirm_password {
        if confirm_pw != &payload.password {
            return Err(AppError::BadRequest(i18n::msg_password_mismatch(lang).into()));
        }
    }

    validate_password(&payload.password, lang)?;

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
                return Err(AppError::Conflict(i18n::msg_email_exists(lang).into()));
            }
        }
        return Err(AppError::Database(e));
    }

    // 2. Tạo ví mặc định liên kết chính xác với user_id mới tạo (Tên ví bản địa hóa theo ngôn ngữ)
    let wallet_id = format!("wal_{}", Uuid::new_v4().simple());
    let default_wallet_title = i18n::msg_default_wallet_name(lang);

    sqlx::query(
        "INSERT INTO findora.wallets (id, user_id, name, currency, initial_balance, current_balance)
         VALUES ($1, $2, $3, 'VND', 0, 0)"
    )
    .bind(&wallet_id)
    .bind(&user_id)
    .bind(default_wallet_title)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Database)?;

    tx.commit().await.map_err(AppError::Database)?;

    info!(user_id = %user_id, email = %email, lang = %lang.code(), "🎉 Đăng ký tài khoản và khởi tạo ví thành công");

    // Tạo token JWT có thời hạn 30 ngày
    let token = create_jwt(&user_id, &email, &state.config.jwt_secret, 30)?;

    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            success: true,
            message: i18n::msg_register_success(lang).to_string(),
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
    lang: Language,
    Json(payload): Json<LoginRequest>,
) -> AppResult<Json<AuthResponse>> {
    let email = payload.email.trim().to_lowercase();
    let now_utc = chrono::Utc::now();
    let vn_offset = chrono::FixedOffset::east_opt(7 * 3600).unwrap();
    let now_vn = now_utc.with_timezone(&vn_offset);

    let user_row = sqlx::query(
        "SELECT id, email, full_name, phone_number, password_hash, 
                failed_login_attempts, locked_until, last_failed_at
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
            return Err(AppError::Unauthorized(i18n::msg_invalid_credentials(lang).into()));
        }
    };

    let user_id: String = row.get("id");
    let pw_hash: String = row.get("password_hash");
    let mut failed_attempts: i32 = row.get("failed_login_attempts");
    let locked_until: Option<chrono::DateTime<chrono::Utc>> = row.get("locked_until");
    let last_failed_at: Option<chrono::DateTime<chrono::Utc>> = row.get("last_failed_at");

    // 1. Kiểm tra nếu tài khoản đang bị khóa do vượt quá số lần nhập sai
    if let Some(lock_time) = locked_until {
        if now_utc < lock_time {
            warn!(email = %email, locked_until = %lock_time, "Tài khoản bị từ chối đăng nhập do đang trong thời gian khóa");
            return Err(AppError::TooManyRequests(
                i18n::msg_account_locked_until_tomorrow(lang).to_string(),
            ));
        }
        // Nếu đã qua mốc khóa (sang ngày mới), reset số lần vi phạm
        failed_attempts = 0;
    } else if let Some(last_failed) = last_failed_at {
        // Nếu lần nhập sai gần nhất là từ ngày hôm trước (theo giờ VN), reset số lần
        let last_failed_vn = last_failed.with_timezone(&vn_offset);
        if last_failed_vn.date_naive() < now_vn.date_naive() {
            failed_attempts = 0;
        }
    }

    let is_valid = verify(&payload.password, &pw_hash).unwrap_or(false);

    // 2. Trường hợp Mật khẩu SAI
    if !is_valid {
        let new_failed = failed_attempts + 1;
        const MAX_FAILED_ATTEMPTS: i32 = 8;

        if new_failed >= MAX_FAILED_ATTEMPTS {
            // Khóa tài khoản đến 00:00:00 ngày hôm sau theo múi giờ Việt Nam
            let tomorrow_vn = now_vn.date_naive() + chrono::Days::new(1);
            let lock_until_vn = tomorrow_vn
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_local_timezone(vn_offset)
                .unwrap();
            let lock_until_utc = lock_until_vn.with_timezone(&chrono::Utc);

            sqlx::query(
                "UPDATE findora.users 
                 SET failed_login_attempts = $1, 
                     locked_until = $2, 
                     last_failed_at = NOW(), 
                     updated_at = NOW() 
                 WHERE id = $3"
            )
            .bind(new_failed)
            .bind(lock_until_utc)
            .bind(&user_id)
            .execute(&*state.pool)
            .await
            .map_err(AppError::Database)?;

            warn!(
                email = %email,
                failed_attempts = new_failed,
                locked_until = %lock_until_vn,
                "🚨 Tài khoản đã nhập sai mật khẩu 8 lần và bị khóa đến 00:00 ngày hôm sau"
            );

            return Err(AppError::TooManyRequests(
                i18n::msg_account_locked_until_tomorrow(lang).to_string(),
            ));
        } else {
            let remaining = (MAX_FAILED_ATTEMPTS - new_failed) as u32;

            sqlx::query(
                "UPDATE findora.users 
                 SET failed_login_attempts = $1, 
                     last_failed_at = NOW(), 
                     updated_at = NOW() 
                 WHERE id = $2"
            )
            .bind(new_failed)
            .bind(&user_id)
            .execute(&*state.pool)
            .await
            .map_err(AppError::Database)?;

            warn!(
                email = %email,
                failed_attempts = new_failed,
                remaining = remaining,
                "Nhập sai mật khẩu"
            );

            return Err(AppError::Unauthorized(
                i18n::msg_wrong_password_with_remaining(lang, remaining),
            ));
        }
    }

    // 3. Trường hợp Mật khẩu ĐÚNG -> Reset toàn bộ trạng thái lỗi về 0
    if failed_attempts > 0 || locked_until.is_some() || last_failed_at.is_some() {
        sqlx::query(
            "UPDATE findora.users 
             SET failed_login_attempts = 0, 
                 locked_until = NULL, 
                 last_failed_at = NULL, 
                 updated_at = NOW() 
             WHERE id = $1"
        )
        .bind(&user_id)
        .execute(&*state.pool)
        .await
        .map_err(AppError::Database)?;
    }

    let full_name: String = row.get("full_name");
    let phone_number: String = row.get("phone_number");

    info!(user_id = %user_id, email = %email, lang = %lang.code(), "🔐 Đăng nhập thành công, đã reset rate limit attempts");

    let token = create_jwt(&user_id, &email, &state.config.jwt_secret, 30)?;

    Ok(Json(AuthResponse {
        success: true,
        message: i18n::msg_login_success(lang).to_string(),
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
    lang: Language,
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
    .ok_or_else(|| AppError::NotFound(i18n::msg_user_not_found(lang).into()))?;

    Ok(Json(UserResponse {
        id: row.get("id"),
        email: row.get("email"),
        full_name: row.get("full_name"),
        phone_number: row.get("phone_number"),
    }))
}

pub async fn logout_handler(
    lang: Language,
    auth: AuthenticatedUser,
) -> AppResult<Json<serde_json::Value>> {
    info!(user_id = %auth.user_id, email = %auth.email, "🚪 [USER LOGOUT] Người dùng đã đăng xuất an toàn khỏi thiết bị");

    Ok(Json(serde_json::json!({
        "success": true,
        "message": i18n::msg_logout_success(lang)
    })))
}

