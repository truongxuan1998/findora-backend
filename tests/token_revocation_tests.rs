use axum::extract::State;
use dotenvy::dotenv;
use findora_backend::auth::{
    check_token_version, create_versioned_jwt, fetch_token_version, verify_jwt, AuthenticatedUser,
    TOKEN_VALID_DAYS,
};
use findora_backend::config::Config;
use findora_backend::db;
use findora_backend::i18n::Language;
use findora_backend::routes::auth::{logout_all_handler, refresh_token_handler};
use findora_backend::AppState;
use std::env;
use std::sync::Arc;

/// Thu hồi phiên bằng token_version trên DB thật: logout-all làm token cũ bị từ chối,
/// refresh cấp token mới mang version hiện tại.
#[tokio::test]
async fn test_logout_all_revokes_old_tokens_and_refresh_issues_current_version() {
    dotenv().ok();
    dotenvy::from_filename(".env").ok();

    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = db::create_pool(&db_url).await.expect("Failed to create pool");
    db::run_schema_and_migrations(&pool).await.expect("Migration failed");

    let config = Config::from_env().expect("Config must be valid");
    let secret = config.jwt_secret.clone();
    let redis_service = Arc::new(findora_backend::redis_client::RedisService::disabled());
    let state = Arc::new(AppState {
        pool: Arc::new(pool.clone()),
        config,
        redis: redis_service.clone(),
        email: Arc::new(findora_backend::email::EmailService::disabled()),
        translation: Arc::new(findora_backend::translation::TranslationService::new(redis_service)),
    });

    let user_id = "usr_token_revocation_test";
    let email = "token_revocation_test@findora.app";

    sqlx::query(
        "INSERT INTO findora.users (id, email, full_name, password_hash)
         VALUES ($1, $2, 'Token Revocation User', 'hashed_pass')
         ON CONFLICT (id) DO UPDATE SET token_version = 0",
    )
    .bind(user_id)
    .bind(email)
    .execute(&pool)
    .await
    .unwrap();

    // 1. User mới: version 0, token version 0 hợp lệ
    assert_eq!(fetch_token_version(&pool, user_id).await.unwrap(), Some(0));
    let old_token = create_versioned_jwt(user_id, email, 0, &secret, TOKEN_VALID_DAYS).unwrap();
    let old_claims = verify_jwt(&old_token, &secret).unwrap();
    assert!(check_token_version(&old_claims, Some(0)).is_ok());

    let auth = AuthenticatedUser {
        user_id: user_id.to_string(),
        email: email.to_string(),
    };

    // 2. Đăng xuất mọi thiết bị → version tăng, token cũ bị từ chối
    let logout_all = logout_all_handler(State(state.clone()), Language::Vi, auth.clone())
        .await
        .expect("logout-all must succeed");
    assert_eq!(logout_all.0["success"], true);
    let current = fetch_token_version(&pool, user_id).await.unwrap();
    assert_eq!(current, Some(1));
    assert!(check_token_version(&old_claims, current).is_err(), "token cũ phải bị thu hồi");

    // 3. Refresh cấp token mới mang version hiện tại
    let refreshed = refresh_token_handler(State(state.clone()), Language::Vi, auth)
        .await
        .expect("refresh must succeed");
    let new_token = refreshed.0.token.clone().expect("refresh must return a token");
    let new_claims = verify_jwt(&new_token, &secret).unwrap();
    assert_eq!(new_claims.ver, 1);
    assert!(check_token_version(&new_claims, current).is_ok());

    // 4. User bị xoá → mọi token bị từ chối
    sqlx::query("DELETE FROM findora.users WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let after_delete = fetch_token_version(&pool, user_id).await.unwrap();
    assert_eq!(after_delete, None);
    assert!(check_token_version(&new_claims, after_delete).is_err());
}
