use axum::extract::State;
use axum::Json;
use dotenvy::dotenv;
use findora_backend::config::Config;
use findora_backend::db;
use findora_backend::errors::AppError;
use findora_backend::i18n::Language;
use findora_backend::models::user::{
    ForgotPasswordRequest, LoginRequest, ResetPasswordRequest,
};
use findora_backend::routes::auth::{
    forgot_password_handler, login_handler, reset_password_handler,
};
use findora_backend::AppState;
use std::env;
use std::sync::Arc;

#[tokio::test]
async fn test_forgot_and_reset_password_workflow() {
    dotenv().ok();
    dotenvy::from_filename("backend/.env").ok();
    dotenvy::from_filename(".env").ok();

    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = db::create_pool(&db_url).await.expect("Failed to create pool");

    db::run_schema_and_migrations(&pool)
        .await
        .expect("Migration failed");

    let config = Config::from_env().expect("Config must be valid");
    let redis_service = Arc::new(findora_backend::redis_client::RedisService::disabled());
    let email_service = Arc::new(findora_backend::email::EmailService::disabled());
    let state = Arc::new(AppState {
        pool: Arc::new(pool.clone()),
        config,
        redis: redis_service,
        email: email_service,
    });

    let test_email = "forgot_pwd_test@findora.app";
    let old_pw = "OldPassword123!";
    let new_pw = "NewSecurePassword2026@";
    let hashed = bcrypt::hash(old_pw, bcrypt::DEFAULT_COST).unwrap();

    // 1. Tạo user test bị khóa do nhập sai mật khẩu trước đó
    let _ = sqlx::query("DELETE FROM findora.password_resets WHERE email = $1")
        .bind(test_email)
        .execute(&pool)
        .await;

    let _ = sqlx::query(
        "INSERT INTO findora.users (id, email, full_name, password_hash, failed_login_attempts, locked_until, last_failed_at)
         VALUES ('usr_forgot_pwd_test', $1, 'Forgot Pwd User', $2, 8, NOW() + INTERVAL '1 day', NOW())
         ON CONFLICT (email) DO UPDATE 
         SET password_hash = $2, failed_login_attempts = 8, locked_until = NOW() + INTERVAL '1 day', last_failed_at = NOW()"
    )
    .bind(test_email)
    .bind(&hashed)
    .execute(&pool)
    .await
    .unwrap();

    // 2. Yêu cầu mã quên mật khẩu (POST /api/auth/forgot-password)
    let forgot_req = ForgotPasswordRequest {
        email: test_email.to_string(),
    };
    let forgot_res = forgot_password_handler(State(state.clone()), Language::Vi, Json(forgot_req))
        .await
        .expect("Yêu cầu quên mật khẩu phải thành công");

    // Chuẩn Production: API response tuyệt đối KHÔNG chứa reset_code nhưng trả expires_in 120s để client đếm ngược
    assert_eq!(forgot_res.0.expires_in, 120, "Hạn mã xác thực phải là 120 giây");
    assert!(
        forgot_res.0.reset_code.is_none(),
        "Production API response không được tiết lộ mã xác thực"
    );

    // Lấy mã xác thực được lưu bảo mật trong database (mô phỏng người dùng nhận qua email)
    let record: (String,) = sqlx::query_as(
        "SELECT code FROM findora.password_resets WHERE email = $1 ORDER BY created_at DESC LIMIT 1"
    )
    .bind(test_email)
    .fetch_one(&pool)
    .await
    .expect("Mã xác thực phải được lưu trong database");
    let code = record.0;
    assert_eq!(code.len(), 6, "Mã xác thực phải gồm đúng 6 chữ số");

    // 3. Đặt lại mật khẩu với mã SAI -> Phải bị từ chối
    let bad_reset_req = ResetPasswordRequest {
        email: test_email.to_string(),
        code: "000000".to_string(),
        new_password: new_pw.to_string(),
        confirm_password: Some(new_pw.to_string()),
    };
    let bad_res = reset_password_handler(State(state.clone()), Language::Vi, Json(bad_reset_req)).await;
    assert!(bad_res.is_err(), "Mã sai phải bị từ chối");
    match bad_res.unwrap_err() {
        AppError::BadRequest(msg) => assert!(msg.contains("không chính xác") || msg.contains("hết hạn")),
        other => panic!("Phải trả về BadRequest, nhận được: {:?}", other),
    }

    // 4. Đặt lại mật khẩu với mật khẩu mới quá yếu (thiếu ký tự đặc biệt) -> Phải bị từ chối bởi validate_password
    let weak_pw_req = ResetPasswordRequest {
        email: test_email.to_string(),
        code: code.clone(),
        new_password: "WeakPassword123".to_string(),
        confirm_password: Some("WeakPassword123".to_string()),
    };
    let weak_res = reset_password_handler(State(state.clone()), Language::Vi, Json(weak_pw_req)).await;
    assert!(weak_res.is_err(), "Mật khẩu yếu phải bị từ chối");

    // 5. Đặt lại mật khẩu với mã ĐÚNG và mật khẩu mạnh hợp lệ
    let valid_reset_req = ResetPasswordRequest {
        email: test_email.to_string(),
        code: code.clone(),
        new_password: new_pw.to_string(),
        confirm_password: Some(new_pw.to_string()),
    };
    let reset_res = reset_password_handler(State(state.clone()), Language::Vi, Json(valid_reset_req))
        .await
        .expect("Đặt lại mật khẩu với mã hợp lệ phải thành công");

    assert!(reset_res.0.token.is_some(), "Reset thành công phải cấp ngay JWT token");

    // 6. Mã đó không được dùng lại lần 2 (Replay Attack Defense)
    let replay_req = ResetPasswordRequest {
        email: test_email.to_string(),
        code: code.clone(),
        new_password: new_pw.to_string(),
        confirm_password: Some(new_pw.to_string()),
    };
    let replay_res = reset_password_handler(State(state.clone()), Language::Vi, Json(replay_req)).await;
    assert!(replay_res.is_err(), "Mã đã dùng không thể tái sử dụng");

    // 7. Đăng nhập bằng mật khẩu MỚI -> Phải thành công và tài khoản đã được tự động mở khóa
    let login_req = LoginRequest {
        email: test_email.to_string(),
        password: new_pw.to_string(),
    };
    let login_res = login_handler(State(state.clone()), Language::Vi, Json(login_req))
        .await
        .expect("Đăng nhập bằng mật khẩu mới phải thành công");

    assert!(login_res.0.token.is_some());

    // 8. Kiểm tra cơ chế timeout 120 giây: Mã hết hạn (> 120s) phải bị từ chối
    let expired_code = "888888";
    let expired_id = format!("rst_exp_{}", uuid::Uuid::new_v4().simple());
    let _ = sqlx::query(
        "INSERT INTO findora.password_resets (id, email, code, expires_at, created_at)
         VALUES ($1, $2, $3, NOW() - INTERVAL '1 second', NOW() - INTERVAL '121 seconds')"
    )
    .bind(&expired_id)
    .bind(test_email)
    .bind(expired_code)
    .execute(&pool)
    .await
    .unwrap();

    let timeout_req = ResetPasswordRequest {
        email: test_email.to_string(),
        code: expired_code.to_string(),
        new_password: "AnotherPassword2026!".to_string(),
        confirm_password: Some("AnotherPassword2026!".to_string()),
    };
    let timeout_res = reset_password_handler(State(state.clone()), Language::Vi, Json(timeout_req)).await;
    assert!(timeout_res.is_err(), "Mã hết hạn 120s phải bị từ chối");
}

#[tokio::test]
async fn test_daily_password_reset_limit_max_2_times() {
    dotenv().ok();
    dotenvy::from_filename("backend/.env").ok();
    dotenvy::from_filename(".env").ok();

    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = db::create_pool(&db_url).await.expect("Failed to create pool");

    db::run_schema_and_migrations(&pool)
        .await
        .expect("Migration failed");

    let config = Config::from_env().expect("Config must be valid");
    let redis_service = Arc::new(findora_backend::redis_client::RedisService::disabled());
    let email_service = Arc::new(findora_backend::email::EmailService::disabled());
    let state = Arc::new(AppState {
        pool: Arc::new(pool.clone()),
        config,
        redis: redis_service,
        email: email_service,
    });

    let test_email = "daily_reset_limit_test@findora.app";
    let pw = "InitialPassword123!";
    let hashed = bcrypt::hash(pw, bcrypt::DEFAULT_COST).unwrap();

    // 1. Tạo user test
    let _ = sqlx::query(
        "INSERT INTO findora.users (id, email, full_name, password_hash)
         VALUES ('usr_daily_limit_test', $1, 'Daily Limit User', $2)
         ON CONFLICT (email) DO UPDATE SET password_hash = $2"
    )
    .bind(test_email)
    .bind(&hashed)
    .execute(&pool)
    .await
    .unwrap();

    // Dọn dẹp dữ liệu cũ của test này
    sqlx::query("DELETE FROM findora.password_resets WHERE email = $1")
        .bind(test_email)
        .execute(&pool)
        .await
        .unwrap();

    // 2. Chèn 2 lần reset thành công trong vòng 24 giờ qua (used_at != NULL)
    sqlx::query(
        "INSERT INTO findora.password_resets (id, email, code, expires_at, used_at, created_at)
         VALUES 
         ('rst_daily_1', $1, '111111', NOW() + INTERVAL '1 hour', NOW() - INTERVAL '3 hours', NOW() - INTERVAL '3 hours'),
         ('rst_daily_2', $1, '222222', NOW() + INTERVAL '1 hour', NOW() - INTERVAL '1 hour', NOW() - INTERVAL '1 hour')"
    )
    .bind(test_email)
    .execute(&pool)
    .await
    .unwrap();

    // 3. Yêu cầu mã mới qua forgot_password_handler -> Phải bị từ chối vì đã đạt 2 lần/ngày
    let forgot_req = ForgotPasswordRequest {
        email: test_email.to_string(),
    };
    let forgot_res = forgot_password_handler(State(state.clone()), Language::Vi, Json(forgot_req)).await;
    assert!(forgot_res.is_err(), "Yêu cầu mã khi đã đạt 2 lần reset/ngày phải bị từ chối");
    match forgot_res.unwrap_err() {
        AppError::TooManyRequests(msg) => {
            assert!(
                msg.contains("tối đa 2 lần trong 1 ngày"),
                "Thông báo phải chứa thông tin giới hạn 2 lần: {}",
                msg
            );
        }
        other => panic!("Phải trả về TooManyRequests, nhận được: {:?}", other),
    }

    // 4. Thử gọi reset_password_handler trực tiếp -> Phải bị chặn với cùng thông báo
    let reset_req = ResetPasswordRequest {
        email: test_email.to_string(),
        code: "111111".to_string(),
        new_password: "NewPassword2026@!".to_string(),
        confirm_password: Some("NewPassword2026@!".to_string()),
    };
    let reset_res = reset_password_handler(State(state.clone()), Language::Vi, Json(reset_req)).await;
    assert!(reset_res.is_err(), "Reset khi đã đạt 2 lần/ngày phải bị từ chối");
    match reset_res.unwrap_err() {
        AppError::TooManyRequests(msg) => {
            assert!(
                msg.contains("tối đa 2 lần trong 1 ngày"),
                "Thông báo phải chứa thông tin giới hạn 2 lần: {}",
                msg
            );
        }
        other => panic!("Phải trả về TooManyRequests, nhận được: {:?}", other),
    }

    // 5. Thử với tiếng Anh (Language::En) -> Thông báo đa ngữ chuẩn
    let forgot_en = forgot_password_handler(
        State(state.clone()),
        Language::En,
        Json(ForgotPasswordRequest { email: test_email.to_string() }),
    ).await;
    match forgot_en.unwrap_err() {
        AppError::TooManyRequests(msg) => {
            assert!(
                msg.contains("daily limit of 2 password resets"),
                "Thông báo tiếng Anh phải đúng chuẩn: {}",
                msg
            );
        }
        other => panic!("Phải trả về TooManyRequests, nhận được: {:?}", other),
    }

    // 6. Mô phỏng 1 lần reset cũ xảy ra cách đây hơn 24 giờ (NOW() - INTERVAL '25 hours')
    sqlx::query(
        "UPDATE findora.password_resets 
         SET used_at = NOW() - INTERVAL '25 hours' 
         WHERE id = 'rst_daily_1'"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Lúc này trong 24h chỉ còn 1 lần reset -> Yêu cầu mã phải thành công!
    let forgot_ok = forgot_password_handler(
        State(state.clone()),
        Language::Vi,
        Json(ForgotPasswordRequest { email: test_email.to_string() }),
    )
    .await
    .expect("Khi mới chỉ reset 1 lần trong 24h, phải được phép yêu cầu mã");
    assert!(forgot_ok.0.success);
}

