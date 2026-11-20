use crate::auth::OptionalUser;
use crate::errors::{AppError, AppResult};
use crate::i18n::Language;
use crate::models::report::{CategorySummary, MonthlyReportQuery, MonthlyReportResponse};
use crate::AppState;
use axum::{
    extract::{Query, State},
    Json,
};
use chrono::{Datelike, NaiveDate};
use sqlx::Row;
use std::sync::Arc;

pub async fn monthly_report_handler(
    State(state): State<Arc<AppState>>,
    lang: Language,
    auth: OptionalUser,
    Query(query): Query<MonthlyReportQuery>,
) -> AppResult<Json<MonthlyReportResponse>> {
    let now = chrono::Utc::now();
    let year = query.year.unwrap_or_else(|| now.year());
    let month = query.month.unwrap_or_else(|| now.month()).clamp(1, 12);

    // Tính mốc thời gian đầu tháng và cuối tháng (Unix timestamp milliseconds)
    let start_date = NaiveDate::from_ymd_opt(year, month, 1)
        .ok_or_else(|| AppError::BadRequest("Năm hoặc tháng không hợp lệ".into()))?;
    let start_dt = start_date.and_hms_opt(0, 0, 0).unwrap().and_utc();
    let start_millis = start_dt.timestamp_millis();

    let next_month_date = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .unwrap();
    let end_dt = next_month_date.and_hms_opt(0, 0, 0).unwrap().and_utc();
    let end_millis = end_dt.timestamp_millis();

    let user_id = auth.0.map(|u| u.user_id);

    // 1. Tính tổng Thu và Chi chuẩn xác theo khoảng thời gian và user/wallet
    let totals_rows = sqlx::query(
        "SELECT t.tx_type, COALESCE(SUM(t.amount), 0)::BIGINT as sum
         FROM findora.transactions t
         INNER JOIN findora.wallets w ON t.wallet_id = w.id
         WHERE t.deleted_at IS NULL
           AND t.timestamp >= $1 
           AND t.timestamp < $2
           AND ($3::TEXT IS NULL OR w.user_id = $3)
           AND ($4::TEXT IS NULL OR t.wallet_id = $4)
         GROUP BY t.tx_type"
    )
    .bind(start_millis)
    .bind(end_millis)
    .bind(&user_id)
    .bind(&query.wallet_id)
    .fetch_all(&*state.pool)
    .await
    .map_err(AppError::Database)?;

    let mut total_income: i64 = 0;
    let mut total_expense: i64 = 0;

    for r in totals_rows {
        let t: String = r.get("tx_type");
        let s: i64 = r.get("sum");
        match t.as_str() {
            "INCOME" => total_income = s,
            "EXPENSE" => total_expense = s,
            _ => {}
        }
    }

    // 2. Breakdown theo danh mục (Chi tiêu - Hỗ trợ Song ngữ)
    let expense_rows = sqlx::query(
        "SELECT c.id as cat_id, c.name as cat_name, c.name_en as cat_name_en, COALESCE(SUM(t.amount), 0)::BIGINT as total_cat
         FROM findora.transactions t
         INNER JOIN findora.wallets w ON t.wallet_id = w.id
         INNER JOIN findora.categories c ON t.category_id = c.id
         WHERE t.deleted_at IS NULL
           AND t.tx_type = 'EXPENSE'
           AND t.timestamp >= $1 
           AND t.timestamp < $2
           AND ($3::TEXT IS NULL OR w.user_id = $3)
           AND ($4::TEXT IS NULL OR t.wallet_id = $4)
         GROUP BY c.id, c.name, c.name_en
         ORDER BY total_cat DESC"
    )
    .bind(start_millis)
    .bind(end_millis)
    .bind(&user_id)
    .bind(&query.wallet_id)
    .fetch_all(&*state.pool)
    .await
    .map_err(AppError::Database)?;

    let expense_breakdown: Vec<CategorySummary> = expense_rows
        .into_iter()
        .map(|r| {
            let total: i64 = r.get("total_cat");
            let name_vi: String = r.get("cat_name");
            let name_en: Option<String> = r.get("cat_name_en");

            let display_name = match lang {
                Language::En => name_en.clone().unwrap_or_else(|| name_vi.clone()),
                Language::Vi => name_vi,
            };

            let percentage = if total_expense > 0 {
                ((total as f64 / total_expense as f64) * 1000.0).round() / 10.0
            } else {
                0.0
            };
            CategorySummary {
                category_id: r.get("cat_id"),
                category_name: display_name,
                category_name_en: name_en,
                total_amount: total,
                percentage,
            }
        })
        .collect();

    // 3. Breakdown theo danh mục (Thu nhập - Hỗ trợ Song ngữ)
    let income_rows = sqlx::query(
        "SELECT c.id as cat_id, c.name as cat_name, c.name_en as cat_name_en, COALESCE(SUM(t.amount), 0)::BIGINT as total_cat
         FROM findora.transactions t
         INNER JOIN findora.wallets w ON t.wallet_id = w.id
         INNER JOIN findora.categories c ON t.category_id = c.id
         WHERE t.deleted_at IS NULL
           AND t.tx_type = 'INCOME'
           AND t.timestamp >= $1 
           AND t.timestamp < $2
           AND ($3::TEXT IS NULL OR w.user_id = $3)
           AND ($4::TEXT IS NULL OR t.wallet_id = $4)
         GROUP BY c.id, c.name, c.name_en
         ORDER BY total_cat DESC"
    )
    .bind(start_millis)
    .bind(end_millis)
    .bind(&user_id)
    .bind(&query.wallet_id)
    .fetch_all(&*state.pool)
    .await
    .map_err(AppError::Database)?;

    let income_breakdown: Vec<CategorySummary> = income_rows
        .into_iter()
        .map(|r| {
            let total: i64 = r.get("total_cat");
            let name_vi: String = r.get("cat_name");
            let name_en: Option<String> = r.get("cat_name_en");

            let display_name = match lang {
                Language::En => name_en.clone().unwrap_or_else(|| name_vi.clone()),
                Language::Vi => name_vi,
            };

            let percentage = if total_income > 0 {
                ((total as f64 / total_income as f64) * 1000.0).round() / 10.0
            } else {
                0.0
            };
            CategorySummary {
                category_id: r.get("cat_id"),
                category_name: display_name,
                category_name_en: name_en,
                total_amount: total,
                percentage,
            }
        })
        .collect();

    Ok(Json(MonthlyReportResponse {
        year,
        month,
        total_income,
        total_expense,
        net_balance: total_income - total_expense,
        expense_breakdown,
        income_breakdown,
    }))
}
