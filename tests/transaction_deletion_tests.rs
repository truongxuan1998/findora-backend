use axum::extract::{Path, Query, State};
use axum::Json;
use dotenvy::dotenv;
use findora_backend::auth::AuthenticatedUser;
use findora_backend::config::Config;
use findora_backend::db;
use findora_backend::i18n::Language;
use findora_backend::models::transaction::{CreateTransactionRequest, TransactionQuery};
use findora_backend::routes::transactions::{
    clear_all_transactions_handler, create_transaction_handler, delete_transaction_handler,
    list_transactions_handler, update_transaction_note_handler, UpdateNoteRequest,
};
use findora_backend::AppState;
use std::env;
use std::sync::Arc;

#[tokio::test]
async fn test_transaction_delete_and_note_management() {
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
    let translation_service = Arc::new(findora_backend::translation::TranslationService::new(redis_service.clone()));
    let state = Arc::new(AppState {
        pool: Arc::new(pool.clone()),
        config,
        redis: redis_service,
        email: email_service,
        translation: translation_service,
    });

    let test_user_id = "usr_tx_del_test_1";
    let test_email = "tx_del_test@findora.app";
    let test_wallet_id = "wal_tx_del_test_1";

    // 1. Chuẩn bị user & ví test với số dư ban đầu 1,000,000 VND
    let _ = sqlx::query(
        "INSERT INTO findora.users (id, email, full_name, password_hash)
         VALUES ($1, $2, 'Tx Del User', 'hashed_pass')
         ON CONFLICT (id) DO NOTHING"
    )
    .bind(test_user_id)
    .bind(test_email)
    .execute(&pool)
    .await
    .unwrap();

    let _ = sqlx::query(
        "INSERT INTO findora.wallets (id, user_id, name, currency, initial_balance, current_balance)
         VALUES ($1, $2, 'Ví Chính Test', 'VND', 1000000, 1000000)
         ON CONFLICT (id) DO UPDATE SET current_balance = 1000000, deleted_at = NULL"
    )
    .bind(test_wallet_id)
    .bind(test_user_id)
    .execute(&pool)
    .await
    .unwrap();

    let auth_user = AuthenticatedUser {
        user_id: test_user_id.to_string(),
        email: test_email.to_string(),
    };

    // 2. Tạo giao dịch chi tiêu 200,000 VND kèm ghi chú "Cơm trưa văn phòng"
    let create_req = CreateTransactionRequest {
        wallet_id: Some(test_wallet_id.to_string()),
        amount: 200_000,
        currency: "VND".to_string(),
        tx_type: "EXPENSE".to_string(),
        category_id: "cat_food".to_string(),
        note: Some("Cơm trưa văn phòng".to_string()),
        timestamp: None,
    };

    let (status, Json(created_tx)) = create_transaction_handler(
        State(state.clone()),
        Language::Vi,
        auth_user.clone(),
        Json(create_req),
    )
    .await
    .expect("Tạo giao dịch phải thành công");

    assert_eq!(status, axum::http::StatusCode::CREATED);
    assert_eq!(created_tx.note, "Cơm trưa văn phòng");

    // Kiểm tra số dư ví giảm còn 800,000 VND
    let bal: (i64,) = sqlx::query_as("SELECT current_balance FROM findora.wallets WHERE id = $1")
        .bind(test_wallet_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(bal.0, 800_000);

    // 3. Cập nhật / Xóa nội dung ghi chú (PATCH /api/transactions/:id)
    let update_note_req = UpdateNoteRequest {
        note: Some("".to_string()), // Xóa sạch nội dung ghi chú
    };
    let update_res = update_transaction_note_handler(
        State(state.clone()),
        auth_user.clone(),
        Path(created_tx.id.clone()),
        Json(update_note_req),
    )
    .await
    .expect("Xóa nội dung ghi chú phải thành công");

    assert_eq!(update_res.0.get("note").unwrap(), "");

    // 4. Xóa giao dịch (DELETE /api/transactions/:id)
    let del_res = delete_transaction_handler(
        State(state.clone()),
        auth_user.clone(),
        Path(created_tx.id.clone()),
    )
    .await
    .expect("Xóa giao dịch phải thành công");

    assert_eq!(del_res.0.get("success").unwrap(), true);

    // 5. Kiểm tra số dư ví được hoàn trả lại đúng 1,000,000 VND
    let bal_restored: (i64,) = sqlx::query_as("SELECT current_balance FROM findora.wallets WHERE id = $1")
        .bind(test_wallet_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(bal_restored.0, 1_000_000, "Số dư ví phải được hoàn lại đúng 1,000,000 VND khi xóa chi tiêu");

    // 6. Kiểm tra giao dịch đã bị xóa không còn xuất hiện trong danh sách
    let list_req = TransactionQuery {
        wallet_id: Some(test_wallet_id.to_string()),
        category_id: None,
        translate_to: None,
        limit: Some(10),
        offset: Some(0),
    };
    let list_res = list_transactions_handler(State(state.clone()), auth_user.clone(), Query(list_req))
        .await
        .unwrap();
    assert!(list_res.0.iter().all(|t| t.id != created_tx.id), "Giao dịch đã xóa mềm không được xuất hiện trong list");

    // 7. Thử xóa giao dịch bởi user khác -> Phải trả về NotFound
    let hacker_auth = AuthenticatedUser {
        user_id: "usr_hacker_random".to_string(),
        email: "hacker@evil.com".to_string(),
    };
    let unauthorized_del = delete_transaction_handler(
        State(state.clone()),
        hacker_auth,
        Path(created_tx.id.clone()),
    )
    .await;
    assert!(unauthorized_del.is_err(), "Không được phép xóa giao dịch của người khác");

    // 8. Kiểm tra xóa toàn bộ dữ liệu ghi chú (DELETE /api/transactions)
    let clear_all_res = clear_all_transactions_handler(State(state.clone()), auth_user)
        .await
        .expect("Xóa toàn bộ giao dịch phải thành công");
    assert_eq!(clear_all_res.0.get("success").unwrap(), true);
}
