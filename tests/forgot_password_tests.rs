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

    // Chuẩn Production: API response tuyệt đối KHÔNG chứa reset_code
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
}
