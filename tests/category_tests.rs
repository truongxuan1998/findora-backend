use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use dotenvy::dotenv;
use findora_backend::auth::AuthenticatedUser;
use findora_backend::config::Config;
use findora_backend::db;
use findora_backend::errors::AppError;
use findora_backend::i18n::Language;
use findora_backend::models::category::CreateCategoryRequest;
use findora_backend::models::transaction::{CreateTransactionRequest, TransactionQuery};
use findora_backend::redis_client::RedisService;
use findora_backend::routes::categories::{categories_handler, create_category_handler};
use findora_backend::routes::transactions::{create_transaction_handler, list_transactions_handler};
use findora_backend::AppState;
use std::env;
use std::sync::Arc;

#[tokio::test]
async fn test_user_custom_categories_and_transactions_workflow() {
    dotenv().ok();
    dotenvy::from_filename("backend/.env").ok();
    dotenvy::from_filename(".env").ok();

    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = db::create_pool(&db_url).await.expect("Failed to create pool");

    db::run_schema_and_migrations(&pool)
        .await
        .expect("Migration failed");

    let config = Config::from_env().expect("Config must be valid");
    let redis_service = Arc::new(
        RedisService::new(
            config.redis_url.as_deref(),
            config.upstash_redis_rest_url.as_deref(),
            config.upstash_redis_rest_token.as_deref(),
        )
        .await,
    );
    let email_service = Arc::new(findora_backend::email::EmailService::disabled());
    let translation_service = Arc::new(findora_backend::translation::TranslationService::new(
        redis_service.clone(),
    ));
    let state = Arc::new(AppState {
        pool: Arc::new(pool.clone()),
        config,
        redis: redis_service.clone(),
        email: email_service,
        translation: translation_service,
    });

    let user1_id = "usr_cat_test_1";
    let user1_email = "cat_test_1@findora.app";
    let user2_id = "usr_cat_test_2";
    let user2_email = "cat_test_2@findora.app";
    let wallet1_id = "wal_cat_test_1";

    // 1. Setup 2 test users & 1 test wallet
    for (uid, email, name) in [
        (user1_id, user1_email, "Cat Test User 1"),
        (user2_id, user2_email, "Cat Test User 2"),
    ] {
        sqlx::query(
            "INSERT INTO findora.users (id, email, full_name, password_hash)
             VALUES ($1, $2, $3, 'hashed_pass')
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(uid)
        .bind(email)
        .bind(name)
        .execute(&pool)
        .await
        .unwrap();
    }

    sqlx::query(
        "INSERT INTO findora.wallets (id, user_id, name, currency, initial_balance, current_balance)
         VALUES ($1, $2, 'Ví Danh Mục Test', 'VND', 1000000, 1000000)
         ON CONFLICT (id) DO UPDATE SET current_balance = 1000000, deleted_at = NULL",
    )
    .bind(wallet1_id)
    .bind(user1_id)
    .execute(&pool)
    .await
    .unwrap();

    // Dọn dữ liệu test cũ nếu có
    let _ = sqlx::query("DELETE FROM findora.transactions WHERE wallet_id = $1")
        .bind(wallet1_id)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM findora.categories WHERE user_id IN ($1, $2)")
        .bind(user1_id)
        .bind(user2_id)
        .execute(&pool)
        .await;
    redis_service
        .del_pattern(&format!("findora:cache:categories:{}:*", user1_id))
        .await;
    redis_service
        .del_pattern(&format!("findora:cache:categories:{}:*", user2_id))
        .await;

    let auth_u1 = AuthenticatedUser {
        user_id: user1_id.to_string(),
        email: user1_email.to_string(),
    };
    let auth_u2 = AuthenticatedUser {
        user_id: user2_id.to_string(),
        email: user2_email.to_string(),
    };

    // 2. GET /api/categories ban đầu cho user 1 -> có 9 danh mục mặc định (và populate cache)
    let Json(initial_cats) = categories_handler(State(state.clone()), Language::Vi, auth_u1.clone())
        .await
        .expect("GET categories phải thành công");
    assert_eq!(initial_cats.len(), 9, "Phải có đúng 9 danh mục mặc định");

    // 3. Validate tạo danh mục lỗi (rỗng tên / sai type -> 400 BadRequest)
    let bad_req: CreateCategoryRequest = serde_json::from_value(serde_json::json!({
        "name": "   ",
        "icon": "pawprint.fill",
        "color": "#795548",
        "type": "EXPENSE"
    }))
    .unwrap();
    let err_empty = create_category_handler(
        State(state.clone()),
        Language::Vi,
        auth_u1.clone(),
        Json(bad_req),
    )
    .await
    .expect_err("Tên rỗng phải bị từ chối");
    assert_eq!(err_empty.into_response().status(), StatusCode::BAD_REQUEST);

    let bad_type_req: CreateCategoryRequest = serde_json::from_value(serde_json::json!({
        "name": "Thú cưng",
        "icon": "pawprint.fill",
        "color": "#795548",
        "type": "TRANSFER"
    }))
    .unwrap();
    let err_type = create_category_handler(
        State(state.clone()),
        Language::Vi,
        auth_u1.clone(),
        Json(bad_type_req),
    )
    .await
    .expect_err("Type TRANSFER không hợp lệ cho category");
    assert_eq!(err_type.into_response().status(), StatusCode::BAD_REQUEST);

    // 4. POST /api/categories hợp lệ cho user 1 (kèm alias nameEn) -> 201 Created + invalidate cache
    let create_cat_req: CreateCategoryRequest = serde_json::from_value(serde_json::json!({
        "name": "Thú cưng",
        "nameEn": "Pets",
        "icon": "pawprint.fill",
        "color": "#795548",
        "type": "expense"
    }))
    .unwrap();
    let (cat_status, Json(created_cat)) = create_category_handler(
        State(state.clone()),
        Language::Vi,
        auth_u1.clone(),
        Json(create_cat_req),
    )
    .await
    .expect("POST /api/categories phải thành công");

    assert_eq!(cat_status, StatusCode::CREATED);
    assert!(created_cat.id.starts_with("cat_"));
    assert_eq!(created_cat.name, "Thú cưng");
    assert_eq!(created_cat.name_en.as_deref(), Some("Pets"));
    assert_eq!(created_cat.r#type, "EXPENSE");

    // 5. GET /api/categories cho user 1 -> thấy 9 mặc định + 1 tuỳ chỉnh (cache đã được invalidate)
    let Json(u1_cats) = categories_handler(State(state.clone()), Language::Vi, auth_u1.clone())
        .await
        .expect("GET categories user 1 phải thành công");
    assert_eq!(u1_cats.len(), 10);
    assert!(u1_cats.iter().any(|c| c.id == created_cat.id && c.name == "Thú cưng"));

    // 6. GET /api/categories cho user 2 -> chỉ thấy 9 mặc định, không rò danh mục user 1
    let Json(u2_cats) = categories_handler(State(state.clone()), Language::Vi, auth_u2.clone())
        .await
        .expect("GET categories user 2 phải thành công");
    assert_eq!(u2_cats.len(), 9);
    assert!(!u2_cats.iter().any(|c| c.id == created_cat.id));

    // 7. Tạo giao dịch với danh mục tuỳ chỉnh vừa tạo -> thành công, ví giảm số dư
    let tx_req = CreateTransactionRequest {
        wallet_id: Some(wallet1_id.to_string()),
        to_wallet_id: None,
        amount: 150_000,
        currency: "VND".to_string(),
        tx_type: "EXPENSE".to_string(),
        category_id: created_cat.id.clone(),
        note: Some("Mua hạt cho mèo".to_string()),
        timestamp: None,
    };
    let (tx_status, Json(created_tx)) = create_transaction_handler(
        State(state.clone()),
        Language::Vi,
        auth_u1.clone(),
        Json(tx_req),
    )
    .await
    .expect("Tạo giao dịch với danh mục tuỳ chỉnh phải thành công");
    assert_eq!(tx_status, StatusCode::CREATED);
    assert_eq!(created_tx.category_id, created_cat.id);

    let bal: (i64,) = sqlx::query_as("SELECT current_balance FROM findora.wallets WHERE id = $1")
        .bind(wallet1_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(bal.0, 850_000);

    // Liệt kê giao dịch thấy giao dịch vừa tạo kèm category_name = "Thú cưng"
    let Json(listed_txs) = list_transactions_handler(
        State(state.clone()),
        auth_u1.clone(),
        Query(TransactionQuery {
            wallet_id: Some(wallet1_id.to_string()),
            category_id: None,
            limit: Some(10),
            offset: Some(0),
            translate_to: None,
        }),
    )
    .await
    .expect("Liệt kê giao dịch phải thành công");
    assert!(listed_txs
        .iter()
        .any(|t| t.id == created_tx.id && t.category_name.as_deref() == Some("Thú cưng")));

    // 8. Tạo giao dịch với category_id không tồn tại -> HTTP 400 BadRequest (không phải 500)
    let invalid_tx_req = CreateTransactionRequest {
        wallet_id: Some(wallet1_id.to_string()),
        to_wallet_id: None,
        amount: 50_000,
        currency: "VND".to_string(),
        tx_type: "EXPENSE".to_string(),
        category_id: "cat_khong_ton_tai_999".to_string(),
        note: None,
        timestamp: None,
    };
    let err_invalid_cat = create_transaction_handler(
        State(state.clone()),
        Language::Vi,
        auth_u1.clone(),
        Json(invalid_tx_req),
    )
    .await
    .expect_err("category_id không tồn tại phải trả lỗi");
    assert_eq!(err_invalid_cat.into_response().status(), StatusCode::BAD_REQUEST);

    // 9. User 2 không được phép dùng danh mục riêng của User 1
    let wallet2_id = "wal_cat_test_2";
    sqlx::query(
        "INSERT INTO findora.wallets (id, user_id, name, currency, initial_balance, current_balance)
         VALUES ($1, $2, 'Ví User 2', 'VND', 500000, 500000)
         ON CONFLICT (id) DO UPDATE SET current_balance = 500000, deleted_at = NULL",
    )
    .bind(wallet2_id)
    .bind(user2_id)
    .execute(&pool)
    .await
    .unwrap();

    let hijack_tx_req = CreateTransactionRequest {
        wallet_id: Some(wallet2_id.to_string()),
        to_wallet_id: None,
        amount: 50_000,
        currency: "VND".to_string(),
        tx_type: "EXPENSE".to_string(),
        category_id: created_cat.id.clone(),
        note: None,
        timestamp: None,
    };
    let err_hijack = create_transaction_handler(
        State(state.clone()),
        Language::Vi,
        auth_u2.clone(),
        Json(hijack_tx_req),
    )
    .await
    .expect_err("Không được dùng danh mục riêng của user khác");
    assert_eq!(err_hijack.into_response().status(), StatusCode::BAD_REQUEST);

    // 10. Kiểm tra mapping SQLSTATE 23503 -> BadRequest trong AppError::from
    let fk_err = sqlx::query(
        "INSERT INTO findora.transactions (id, wallet_id, amount, currency, tx_type, category_id, timestamp, created_at, updated_at)
         VALUES ('tx_fk_test_bad', $1, 1000, 'VND', 'EXPENSE', 'cat_nonexistent_fk', 1000, 1000, 1000)",
    )
    .bind(wallet1_id)
    .execute(&pool)
    .await
    .expect_err("Insert trực tiếp với category_id sai phải vi phạm FK")
    .into();
    let app_err: AppError = fk_err;
    assert_eq!(app_err.into_response().status(), StatusCode::BAD_REQUEST);

    // Dọn dẹp dữ liệu test
    let _ = sqlx::query("DELETE FROM findora.transactions WHERE wallet_id IN ($1, $2)")
        .bind(wallet1_id)
        .bind(wallet2_id)
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM findora.categories WHERE user_id IN ($1, $2)")
        .bind(user1_id)
        .bind(user2_id)
        .execute(&pool)
        .await;
    redis_service
        .del_pattern(&format!("findora:cache:categories:{}:*", user1_id))
        .await;
    redis_service
        .del_pattern(&format!("findora:cache:categories:{}:*", user2_id))
        .await;
}
