use axum::extract::{Query, State};
use dotenvy::dotenv;
use findora_backend::auth::AuthenticatedUser;
use findora_backend::config::Config;
use findora_backend::db;
use findora_backend::i18n::Language;
use findora_backend::models::report::ReportSummaryQuery;
use findora_backend::routes::reports::summary_report_handler;
use findora_backend::AppState;
use std::env;
use std::sync::Arc;

#[tokio::test]
async fn test_financial_report_summary_week_month_year() {
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

    let test_user_id = "usr_stat_test_1";
    let test_email = "stat_test@findora.app";
    let test_wallet_id = "wal_stat_test_1";

    let _ = sqlx::query(
        "INSERT INTO findora.users (id, email, full_name, password_hash)
         VALUES ($1, $2, 'Stat Test User', 'hashed_pass')
         ON CONFLICT (id) DO NOTHING"
    )
    .bind(test_user_id)
    .bind(test_email)
    .execute(&pool)
    .await
    .unwrap();

    let _ = sqlx::query(
        "INSERT INTO findora.wallets (id, user_id, name, currency, initial_balance, current_balance)
         VALUES ($1, $2, 'Ví Thống Kê', 'VND', 5000000, 5000000)
         ON CONFLICT (id) DO NOTHING"
    )
    .bind(test_wallet_id)
    .bind(test_user_id)
    .execute(&pool)
    .await
    .unwrap();

    // Dọn dẹp dữ liệu cũ của user test
    let _ = sqlx::query("DELETE FROM findora.transactions WHERE wallet_id = $1")
        .bind(test_wallet_id)
        .execute(&pool)
        .await;

    // Giả sử mốc thời gian kiểm thử: 2026-11-21 (Thứ 7)
    // 2026-11-21 10:00:00 UTC+7 -> millis
    let vn_offset = chrono::FixedOffset::east_opt(7 * 3600).unwrap();
    let ts_nov_21 = chrono::NaiveDate::from_ymd_opt(2026, 11, 21)
        .unwrap()
        .and_hms_opt(10, 0, 0)
        .unwrap()
        .and_local_timezone(vn_offset)
        .unwrap()
        .timestamp_millis();

    // 2026-11-05 10:00:00 UTC+7 (Cùng tháng 11, nhưng tuần trước)
    let ts_nov_05 = chrono::NaiveDate::from_ymd_opt(2026, 11, 5)
        .unwrap()
        .and_hms_opt(10, 0, 0)
        .unwrap()
        .and_local_timezone(vn_offset)
        .unwrap()
        .timestamp_millis();

    // 2026-05-15 10:00:00 UTC+7 (Cùng năm 2026, nhưng tháng 5)
    let ts_may_15 = chrono::NaiveDate::from_ymd_opt(2026, 5, 15)
        .unwrap()
        .and_hms_opt(10, 0, 0)
        .unwrap()
        .and_local_timezone(vn_offset)
        .unwrap()
        .timestamp_millis();

    // Insert 1: Chi tiêu hôm nay (Tuần này, Tháng 11, Năm 2026): 50,000 VND
    sqlx::query(
        "INSERT INTO findora.transactions (id, wallet_id, amount, currency, tx_type, category_id, timestamp, note, created_at, updated_at, created_at_tz)
         VALUES ('tx_stat_1', $1, 50000, 'VND', 'EXPENSE', 'cat_food', $2, 'Cà phê sáng', $2, $2, to_timestamp($2 / 1000.0))"
    )
    .bind(test_wallet_id)
    .bind(ts_nov_21)
    .execute(&pool)
    .await
    .unwrap();

    // Insert 2: Thu nhập hôm nay: 200,000 VND
    sqlx::query(
        "INSERT INTO findora.transactions (id, wallet_id, amount, currency, tx_type, category_id, timestamp, note, created_at, updated_at, created_at_tz)
         VALUES ('tx_stat_2', $1, 200000, 'VND', 'INCOME', 'cat_bonus', $2, 'Thưởng dự án', $2, $2, to_timestamp($2 / 1000.0))"
    )
    .bind(test_wallet_id)
    .bind(ts_nov_21)
    .execute(&pool)
    .await
    .unwrap();

    // Insert 3: Chi tiêu ngày 05/11 (Tuần 1 tháng 11): 150,000 VND
    sqlx::query(
        "INSERT INTO findora.transactions (id, wallet_id, amount, currency, tx_type, category_id, timestamp, note, created_at, updated_at, created_at_tz)
         VALUES ('tx_stat_3', $1, 150000, 'VND', 'EXPENSE', 'cat_shopping', $2, 'Mua áo', $2, $2, to_timestamp($2 / 1000.0))"
    )
    .bind(test_wallet_id)
    .bind(ts_nov_05)
    .execute(&pool)
    .await
    .unwrap();

    // Insert 4: Thu nhập tháng 5: 10,000,000 VND
    sqlx::query(
        "INSERT INTO findora.transactions (id, wallet_id, amount, currency, tx_type, category_id, timestamp, note, created_at, updated_at, created_at_tz)
         VALUES ('tx_stat_4', $1, 10000000, 'VND', 'INCOME', 'cat_salary', $2, 'Lương tháng 5', $2, $2, to_timestamp($2 / 1000.0))"
    )
    .bind(test_wallet_id)
    .bind(ts_may_15)
    .execute(&pool)
    .await
    .unwrap();

    let auth_user = AuthenticatedUser {
        user_id: test_user_id.to_string(),
        email: test_email.to_string(),
    };

    // 1. Thống kê theo TUẦN (Week)
    let week_query = ReportSummaryQuery {
        period: Some("week".into()),
        year: Some(2026),
        month: Some(11),
        week: None,
        date: Some("2026-11-21".into()),
        wallet_id: Some(test_wallet_id.into()),
    };
    let week_res = summary_report_handler(
        State(state.clone()),
        Language::Vi,
        auth_user.clone(),
        Query(week_query),
    )
    .await
    .expect("Week report failed")
    .0;

    assert_eq!(week_res.period, "week");
    assert_eq!(week_res.total_expense, 50000);
    assert_eq!(week_res.total_income, 200000);
    assert_eq!(week_res.net_balance, 150000);
    assert_eq!(week_res.time_series.len(), 7); // 7 ngày trong tuần

    // 2. Thống kê theo THÁNG (Month)
    let month_query = ReportSummaryQuery {
        period: Some("month".into()),
        year: Some(2026),
        month: Some(11),
        week: None,
        date: Some("2026-11-21".into()),
        wallet_id: Some(test_wallet_id.into()),
    };
    let month_res = summary_report_handler(
        State(state.clone()),
        Language::Vi,
        auth_user.clone(),
        Query(month_query),
    )
    .await
    .expect("Month report failed")
    .0;

    assert_eq!(month_res.period, "month");
    assert_eq!(month_res.total_expense, 200000); // 50k + 150k
    assert_eq!(month_res.total_income, 200000);
    assert_eq!(month_res.net_balance, 0);
    assert!(month_res.time_series.len() >= 4); // 4-5 tuần trong tháng

    // 3. Thống kê theo NĂM (Year)
    let year_query = ReportSummaryQuery {
        period: Some("year".into()),
        year: Some(2026),
        month: None,
        week: None,
        date: Some("2026-11-21".into()),
        wallet_id: Some(test_wallet_id.into()),
    };
    let year_res = summary_report_handler(
        State(state.clone()),
        Language::Vi,
        auth_user,
        Query(year_query),
    )
    .await
    .expect("Year report failed")
    .0;

    assert_eq!(year_res.period, "year");
    assert_eq!(year_res.total_expense, 200000);
    assert_eq!(year_res.total_income, 10200000); // 10tr + 200k
    assert_eq!(year_res.net_balance, 10000000);
    assert_eq!(year_res.time_series.len(), 12); // 12 tháng
}
