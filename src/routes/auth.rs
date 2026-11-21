use crate::auth::{create_jwt, AuthenticatedUser};
use crate::errors::{AppError, AppResult};
use crate::i18n::{self, Language};
use crate::models::user::{
    AuthResponse, ForgotPasswordRequest, ForgotPasswordResponse, LoginRequest, RegisterRequest,
    ResetPasswordRequest, UserResponse,
};
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
                avatar_url: None,
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
        "SELECT id, email, full_name, phone_number, password_hash, avatar_url,
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

    let avatar_url: Option<String> = row.get("avatar_url");

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

        avatar_url,
        }),
    }))
}

pub async fn me_handler(
    State(state): State<Arc<AppState>>,
    lang: Language,
    auth: AuthenticatedUser,
) -> AppResult<Json<UserResponse>> {
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

    Ok(Json(UserResponse {
        id: row.get("id"),
        email: row.get("email"),
        full_name: row.get("full_name"),
        phone_number: row.get("phone_number"),
        avatar_url: row.get("avatar_url"),
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

/// 4. Yêu cầu mã xác nhận Quên mật khẩu (POST /api/auth/forgot-password)
pub async fn forgot_password_handler(
    State(state): State<Arc<AppState>>,
    lang: Language,
    Json(payload): Json<ForgotPasswordRequest>,
) -> AppResult<Json<ForgotPasswordResponse>> {
    let email = payload.email.trim().to_lowercase();
    const OTP_EXPIRY_SECONDS: u32 = 120;
    if email.is_empty() {
        return Err(AppError::BadRequest("Email không được để trống".into()));
    }

    // Kiểm tra xem người dùng có tồn tại trong hệ thống không
    let user_exists = sqlx::query("SELECT id FROM findora.users WHERE email = $1")
        .bind(&email)
        .fetch_optional(&*state.pool)
        .await
        .map_err(AppError::Database)?;

    if user_exists.is_none() {
        warn!(email = %email, "Yêu cầu quên mật khẩu cho email không tồn tại");
        // Giả lập thành công để tránh tấn công user enumeration
        return Ok(Json(ForgotPasswordResponse {
            success: true,
            message: i18n::msg_forgot_password_sent(lang).to_string(),
            expires_in: OTP_EXPIRY_SECONDS,
            reset_code: None,
        }));
    }

    // Giới hạn bảo mật: 1 ngày 1 tài khoản chỉ được đặt lại mật khẩu tối đa 2 lần
    const MAX_DAILY_PASSWORD_RESETS: i64 = 2;
    let daily_reset_count: i64 = sqlx::query(
        "SELECT COUNT(*) FROM findora.password_resets 
         WHERE email = $1 AND used_at > NOW() - INTERVAL '24 hours'"
    )
    .bind(&email)
    .fetch_one(&*state.pool)
    .await
    .map_err(AppError::Database)?
    .get(0);

    if daily_reset_count >= MAX_DAILY_PASSWORD_RESETS {
        warn!(email = %email, count = daily_reset_count, "Tài khoản đã đạt giới hạn đặt lại mật khẩu tối đa 2 lần trong 1 ngày");
        return Err(AppError::TooManyRequests(
            i18n::msg_daily_reset_password_limit_exceeded(lang).to_string(),
        ));
    }

    // Rate limiting: Tối đa 5 lần yêu cầu mã trong vòng 15 phút
    let recent_count: i64 = sqlx::query(
        "SELECT COUNT(*) FROM findora.password_resets 
         WHERE email = $1 AND created_at > NOW() - INTERVAL '15 minutes'"
    )
    .bind(&email)
    .fetch_one(&*state.pool)
    .await
    .map_err(AppError::Database)?
    .get(0);

    if recent_count >= 5 {
        warn!(email = %email, "Quá nhiều yêu cầu quên mật khẩu trong 15 phút");
        return Err(AppError::TooManyRequests(
            i18n::msg_too_many_reset_requests(lang).to_string(),
        ));
    }

    // Sinh mã xác thực 6 số ngẫu nhiên CSPRNG
    let random_bytes = Uuid::new_v4().as_bytes()[0..4].try_into().unwrap();
    let code_num = (u32::from_le_bytes(random_bytes) % 900_000) + 100_000;
    let code = format!("{:06}", code_num);

    let reset_id = format!("rst_{}", Uuid::new_v4().simple());
    let vn_offset = chrono::FixedOffset::east_opt(7 * 3600).unwrap();
    // Hạn hiệu lực của mã OTP: 120 giây (2 phút)
    let expires_at = (chrono::Utc::now() + chrono::Duration::seconds(OTP_EXPIRY_SECONDS as i64)).with_timezone(&vn_offset);

    sqlx::query(
        "INSERT INTO findora.password_resets (id, email, code, expires_at, created_at)
         VALUES ($1, $2, $3, $4, NOW())"
    )
    .bind(&reset_id)
    .bind(&email)
    .bind(&code)
    .bind(expires_at)
    .execute(&*state.pool)
    .await
    .map_err(AppError::Database)?;

    info!(
        email = %email,
        expires_at = %expires_at,
        "🔑 Đã phát hành mã xác thực quên mật khẩu 6 số (hết hạn trong 120 giây, lưu an toàn trong database)"
    );

    // Gửi email chứa mã OTP qua Brevo Service trong background task (non-blocking)
    let email_service = state.email.clone();
    let recipient_email = email.clone();
    let otp_code = code.clone();
    tokio::spawn(async move {
        if let Err(e) = email_service.send_reset_password_otp(&recipient_email, &otp_code, lang).await {
            tracing::error!(email = %recipient_email, error = %e, "Lỗi khi gửi email OTP qua Brevo");
        }
    });

    // Chuẩn Production: Trả về expires_in (120s) để client app hiển thị đồng hồ đếm ngược
    Ok(Json(ForgotPasswordResponse {
        success: true,
        message: i18n::msg_forgot_password_sent(lang).to_string(),
        expires_in: OTP_EXPIRY_SECONDS,
        reset_code: None,
    }))
}

/// 5. Đặt lại mật khẩu mới bằng mã xác nhận (POST /api/auth/reset-password)
pub async fn reset_password_handler(
    State(state): State<Arc<AppState>>,
    lang: Language,
    Json(payload): Json<ResetPasswordRequest>,
) -> AppResult<Json<AuthResponse>> {
    let email = payload.email.trim().to_lowercase();
    let code = payload.code.trim();

    if let Some(ref confirm) = payload.confirm_password {
        if &payload.new_password != confirm {
            return Err(AppError::BadRequest(i18n::msg_password_mismatch(lang).into()));
        }
    }

    // 1. Kiểm tra giới hạn: 1 ngày 1 tài khoản chỉ được đặt lại mật khẩu tối đa 2 lần
    const MAX_DAILY_PASSWORD_RESETS: i64 = 2;
    let daily_reset_count: i64 = sqlx::query(
        "SELECT COUNT(*) FROM findora.password_resets 
         WHERE email = $1 AND used_at > NOW() - INTERVAL '24 hours'"
    )
    .bind(&email)
    .fetch_one(&*state.pool)
    .await
    .map_err(AppError::Database)?
    .get(0);

    if daily_reset_count >= MAX_DAILY_PASSWORD_RESETS {
        warn!(email = %email, count = daily_reset_count, "Tài khoản đã đạt giới hạn đặt lại mật khẩu tối đa 2 lần trong 1 ngày");
        return Err(AppError::TooManyRequests(
            i18n::msg_daily_reset_password_limit_exceeded(lang).to_string(),
        ));
    }

    // 2. Kiểm tra chính sách mật khẩu mạnh (ít nhất 8 ký tự, có số, ký tự đặc biệt)
    validate_password(&payload.new_password, lang)?;

    // 2. Kiểm tra mã xác thực trong database (chưa dùng và chưa hết hạn)
    let record = sqlx::query(
        "SELECT id, code, expires_at 
         FROM findora.password_resets 
         WHERE email = $1 AND used_at IS NULL AND expires_at > NOW() 
         ORDER BY created_at DESC LIMIT 1"
    )
    .bind(&email)
    .fetch_optional(&*state.pool)
    .await
    .map_err(AppError::Database)?;

    let reset_row = match record {
        Some(r) => r,
        None => {
            return Err(AppError::BadRequest(
                i18n::msg_invalid_or_expired_code(lang).into(),
            ));
        }
    };

    let stored_code: String = reset_row.get("code");
    let reset_id: String = reset_row.get("id");

    if stored_code != code {
        warn!(email = %email, input_code = %code, "Mã xác thực không trùng khớp");
        return Err(AppError::BadRequest(
            i18n::msg_invalid_or_expired_code(lang).into(),
        ));
    }

    // 3. Hash mật khẩu mới bằng bcrypt
    let hashed_pw = hash(&payload.new_password, DEFAULT_COST)
        .map_err(|e| AppError::Internal(format!("Lỗi mã hóa mật khẩu: {}", e)))?;

    let mut tx = state.pool.begin().await.map_err(AppError::Database)?;

    // 4. Cập nhật mật khẩu mới & Mở khóa tài khoản (reset rate limit attempts)
    let user_row = sqlx::query(
        "UPDATE findora.users 
         SET password_hash = $1, 
             failed_login_attempts = 0, 
             locked_until = NULL, 
             last_failed_at = NULL, 
             updated_at = NOW() 
         WHERE email = $2 
         RETURNING id, full_name, phone_number, avatar_url"
    )
    .bind(&hashed_pw)
    .bind(&email)
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::Database)?;

    let user = match user_row {
        Some(u) => u,
        None => {
            return Err(AppError::NotFound(i18n::msg_user_not_found(lang).into()));
        }
    };

    // Đánh dấu mã đã sử dụng (chống Replay Attack)
    sqlx::query("UPDATE findora.password_resets SET used_at = NOW() WHERE id = $1")
        .bind(&reset_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;

    tx.commit().await.map_err(AppError::Database)?;

    let user_id: String = user.get("id");
    let full_name: String = user.get("full_name");
    let phone_number: String = user.get("phone_number");

    let avatar_url: Option<String> = user.get("avatar_url");

    // Cấp ngay JWT token để người dùng đăng nhập ngay lập tức
    let token = create_jwt(&user_id, &email, &state.config.jwt_secret, 30)?;

    info!(user_id = %user_id, email = %email, "🎉 Đặt lại mật khẩu thành công và tự động mở khóa tài khoản");

    Ok(Json(AuthResponse {
        success: true,
        message: i18n::msg_reset_password_success(lang).to_string(),
        token: Some(token),
        user: Some(UserResponse {
            id: user_id,
            email,
            full_name,
            phone_number,

        avatar_url,
        }),
    }))
}

