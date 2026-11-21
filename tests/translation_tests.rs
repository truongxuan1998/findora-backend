use axum::extract::{Query, State};
use axum::Json;
use dotenvy::dotenv;
use findora_backend::auth::AuthenticatedUser;
use findora_backend::config::Config;
use findora_backend::db;
use findora_backend::models::transaction::{CreateTransactionRequest, TransactionQuery};
use findora_backend::models::translation::{
    TranslateQuery, TranslateRequest, TranslateTransactionsRequest,
};
use findora_backend::routes::transactions::{create_transaction_handler, list_transactions_handler};
use findora_backend::routes::translate::{
    translate_get_handler, translate_handler, translate_transactions_handler,
};
use findora_backend::translation::TranslationService;
use findora_backend::AppState;
use std::env;
use std::sync::Arc;

#[tokio::test]
async fn test_translation_service_dictionary_and_detection() {
    let redis_service = Arc::new(findora_backend::redis_client::RedisService::disabled());
    let service = TranslationService::new(redis_service);

    // 1. Kiểm tra phát hiện ngôn ngữ tự động
    let (src1, tgt1) = service.detect_languages("Ăn trưa với đồng nghiệp", None);
    assert_eq!(src1, "vi");
    assert_eq!(tgt1, "en");

    let (src2, tgt2) = service.detect_languages("Coffee with team", None);
    assert_eq!(src2, "en");
    assert_eq!(tgt2, "vi");

    // 2. Dịch các từ vựng thu chi tài chính thông dụng (Việt -> Anh)
    let trans_lunch = service.translate("ăn trưa", "en", None).await;
    assert_eq!(trans_lunch.to_lowercase(), "lunch");

    let trans_salary = service.translate("tiền lương", "en", None).await;
    assert_eq!(trans_salary.to_lowercase(), "salary");

    let trans_rent = service.translate("tiền nhà", "en", None).await;
    assert_eq!(trans_rent.to_lowercase(), "rent");

    let trans_coffee = service.translate("cà phê", "en", None).await;
    assert_eq!(trans_coffee.to_lowercase(), "coffee");

    let trans_fuel = service.translate("xăng xe", "en", None).await;
    assert_eq!(trans_fuel.to_lowercase(), "fuel");

    // 3. Dịch các từ vựng thu chi (Anh -> Việt)
    let trans_salary_vi = service.translate("salary", "vi", None).await;
    assert!(trans_salary_vi.to_lowercase().contains("lương"));

    let trans_lunch_vi = service.translate("lunch", "vi", None).await;
    assert_eq!(trans_lunch_vi.to_lowercase(), "ăn trưa");

    let trans_coffee_vi = service.translate("coffee", "vi", None).await;
    assert_eq!(trans_coffee_vi.to_lowercase(), "cà phê");

    // 4. Khi ngôn ngữ nguồn và đích giống nhau -> giữ nguyên
    let same = service.translate("Tiền điện", "vi", Some("vi")).await;
    assert_eq!(same, "Tiền điện");
}

#[tokio::test]
async fn test_translate_api_endpoints_and_transaction_sync() {
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
    let translation_service =
        Arc::new(TranslationService::new(redis_service.clone()));
    let state = Arc::new(AppState {
        pool: Arc::new(pool.clone()),
        config,
        redis: redis_service,
        email: email_service,
        translation: translation_service,
    });

    // 1. Kiểm tra POST /api/translate (Dịch đơn)
    let single_req = TranslateRequest {
        text: Some("Ăn tối".to_string()),
        texts: None,
        source: Some("vi".to_string()),
        target: "en".to_string(),
    };
    let single_res = translate_handler(State(state.clone()), Json(single_req))
        .await
        .expect("POST /api/translate phải thành công");
    assert_eq!(
        single_res.0.translated_text.as_deref().unwrap().to_lowercase(),
        "dinner"
    );

    // 2. Kiểm tra POST /api/translate (Dịch hàng loạt danh sách texts)
    let batch_req = TranslateRequest {
        text: None,
        texts: Some(vec![
            "Ăn sáng".to_string(),
            "Cà phê".to_string(),
            "Tiền điện".to_string(),
        ]),
        source: None,
        target: "en".to_string(),
    };
    let batch_res = translate_handler(State(state.clone()), Json(batch_req))
        .await
        .expect("POST /api/translate batch phải thành công");
    assert_eq!(batch_res.0.translations.len(), 3);
    assert_eq!(batch_res.0.translations[0].translated.to_lowercase(), "breakfast");
    assert_eq!(batch_res.0.translations[1].translated.to_lowercase(), "coffee");
    assert_eq!(batch_res.0.translations[2].translated.to_lowercase(), "electricity bill");

    // 3. Kiểm tra GET /api/translate?text=...&target=en
    let get_query = TranslateQuery {
        text: "Tiền nước".to_string(),
        source: None,
        target: Some("en".to_string()),
    };
    let get_res = translate_get_handler(State(state.clone()), Query(get_query))
        .await
        .expect("GET /api/translate phải thành công");
    assert_eq!(
        get_res.0.translated_text.as_deref().unwrap().to_lowercase(),
        "water bill"
    );

    // 4. Chuẩn bị User, Ví và Giao dịch để kiểm tra đồng bộ dịch thu chi
    let test_user_id = "usr_trans_test_1";
    let test_email = "trans_test@findora.app";
    let test_wallet_id = "wal_trans_test_1";

    let _ = sqlx::query(
        "INSERT INTO findora.users (id, email, full_name, password_hash)
         VALUES ($1, $2, 'Translation Tester', 'hashed')
         ON CONFLICT (id) DO NOTHING"
    )
    .bind(test_user_id)
    .bind(test_email)
    .execute(&pool)
    .await
    .unwrap();

    let _ = sqlx::query(
        "INSERT INTO findora.wallets (id, user_id, name, currency, initial_balance, current_balance)
         VALUES ($1, $2, 'Ví Tiêu Dùng', 'VND', 5000000, 5000000)
         ON CONFLICT (id) DO NOTHING"
    )
    .bind(test_wallet_id)
    .bind(test_user_id)
    .execute(&pool)
    .await
    .unwrap();

    let auth = AuthenticatedUser {
        user_id: test_user_id.to_string(),
        email: test_email.to_string(),
    };

    // Tạo giao dịch có ghi chú tiếng Việt: "Ăn trưa"
    let create_req = CreateTransactionRequest {
        wallet_id: Some(test_wallet_id.to_string()),
        amount: 50_000,
        currency: "VND".to_string(),
        tx_type: "EXPENSE".to_string(),
        category_id: "cat_food".to_string(),
        note: Some("Ăn trưa".to_string()),
        timestamp: Some(1732170000000),
    };
    let (_, tx_created) = create_transaction_handler(
        State(state.clone()),
        findora_backend::i18n::Language::Vi,
        auth.clone(),
        Json(create_req),
    )
    .await
    .expect("Tạo giao dịch phải thành công");

    // 5. Kiểm tra POST /api/transactions/translate
    let sync_req = TranslateTransactionsRequest {
        target_lang: "en".to_string(),
        transaction_ids: Some(vec![tx_created.0.id.clone()]),
        limit: Some(10),
    };
    let sync_res = translate_transactions_handler(
        State(state.clone()),
        auth.clone(),
        Json(sync_req),
    )
    .await
    .expect("Dịch giao dịch phải thành công");

    assert!(sync_res.0.success);
    assert!(!sync_res.0.translations.is_empty());
    let item = &sync_res.0.translations[0];
    assert_eq!(item.original_note, "Ăn trưa");
    assert_eq!(item.translated_note.to_lowercase(), "lunch");

    // 6. Kiểm tra GET /api/transactions?translate_to=en
    let list_query = TransactionQuery {
        wallet_id: Some(test_wallet_id.to_string()),
        category_id: None,
        translate_to: Some("en".to_string()),
        limit: Some(10),
        offset: Some(0),
    };
    let list_res = list_transactions_handler(
        State(state.clone()),
        auth,
        Query(list_query),
    )
    .await
    .expect("Lấy danh sách giao dịch có translate_to=en phải thành công");

    let found_tx = list_res.0.iter().find(|t| t.id == tx_created.0.id);
    assert!(found_tx.is_some());
    let t = found_tx.unwrap();
    assert_eq!(t.note, "Ăn trưa");
    assert_eq!(t.translated_note.as_deref().unwrap().to_lowercase(), "lunch");
}
