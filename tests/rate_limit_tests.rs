use axum::extract::State;
use axum::Json;
use dotenvy::dotenv;
use findora_backend::config::Config;
use findora_backend::db;
use findora_backend::errors::AppError;
use findora_backend::i18n::Language;
use findora_backend::models::user::LoginRequest;
use findora_backend::routes::auth::login_handler;
use findora_backend::AppState;
use std::env;
use std::sync::Arc;

#[tokio::test]
async fn test_rate_limit_lockout_workflow() {
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

    let test_email = "rate_limit_test@findora.app";
    let valid_pw = "Password123!";
    let hashed = bcrypt::hash(valid_pw, bcrypt::DEFAULT_COST).unwrap();

    // Reset or insert test user
    let _ = sqlx::query(
        "INSERT INTO findora.users (id, email, full_name, password_hash, failed_login_attempts, locked_until, last_failed_at)
         VALUES ('usr_rate_limit_test', $1, 'Rate Limit Tester', $2, 0, NULL, NULL)
         ON CONFLICT (email) DO UPDATE 
         SET password_hash = $2, failed_login_attempts = 0, locked_until = NULL, last_failed_at = NULL"
    )
    .bind(test_email)
    .bind(&hashed)
    .execute(&pool)
    .await
    .unwrap();

    // 1. Nhập sai 7 lần liên tiếp
    for i in 1..=7 {
        let req = LoginRequest {
            email: test_email.to_string(),
            password: "WrongPassword!".to_string(),
        };
        let res = login_handler(State(state.clone()), Language::Vi, Json(req)).await;
        assert!(res.is_err(), "Lần {} phải thất bại", i);
        match res.unwrap_err() {
            AppError::Unauthorized(msg) => {
                let expected_remaining = 8 - i;
                assert!(
                    msg.contains(&format!("còn {} lần thử", expected_remaining)),
                    "Thông báo phải báo còn {} lần thử, thực tế: {}",
                    expected_remaining,
                    msg
                );
            }
            other => panic!("Lần {} phải trả về Unauthorized, nhận được: {:?}", i, other),
        }
    }

    // 2. Nhập sai lần thứ 8 -> Phải bị khóa tài khoản (TooManyRequests / 429)
    let req8 = LoginRequest {
        email: test_email.to_string(),
        password: "WrongPassword!".to_string(),
    };
    let res8 = login_handler(State(state.clone()), Language::Vi, Json(req8)).await;
    assert!(res8.is_err(), "Lần 8 phải thất bại");
    match res8.unwrap_err() {
        AppError::TooManyRequests(msg) => {
            assert!(
                msg.contains("tạm khóa") && msg.contains("ngày mai"),
                "Thông báo phải báo bị khóa đến ngày mai: {}",
                msg
            );
        }
        other => panic!("Lần 8 phải trả về TooManyRequests, nhận được: {:?}", other),
    }

    // 3. Nhập lần thứ 9 (kể cả đúng mật khẩu lúc này) -> Vẫn phải bị từ chối vì đang bị khóa
    let req9 = LoginRequest {
        email: test_email.to_string(),
        password: valid_pw.to_string(),
    };
    let res9 = login_handler(State(state.clone()), Language::Vi, Json(req9)).await;
    assert!(res9.is_err(), "Khi đang bị khóa thì nhập đúng password cũng phải bị từ chối");
    match res9.unwrap_err() {
        AppError::TooManyRequests(msg) => {
            assert!(msg.contains("tạm khóa"));
        }
        other => panic!("Đang khóa phải trả về TooManyRequests, nhận được: {:?}", other),
    }

    // 4. Mở khóa bằng cách reset lại DB và kiểm tra đăng nhập thành công
    let _ = sqlx::query(
        "UPDATE findora.users 
         SET failed_login_attempts = 0, locked_until = NULL, last_failed_at = NULL 
         WHERE email = $1"
    )
    .bind(test_email)
    .execute(&pool)
    .await
    .unwrap();

    let req_ok = LoginRequest {
        email: test_email.to_string(),
        password: valid_pw.to_string(),
    };
    let res_ok = login_handler(State(state.clone()), Language::Vi, Json(req_ok)).await;
    assert!(res_ok.is_ok(), "Đăng nhập với mật khẩu đúng sau khi mở khóa phải thành công");
}
