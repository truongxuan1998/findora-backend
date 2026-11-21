use crate::auth::AuthenticatedUser;
use crate::errors::{AppError, AppResult};
use crate::i18n::Language;
use crate::models::report::{
    CategoryMonthStat, CategoryReportQuery, CategoryReportResponse, CategorySummary,
    FinancialReportResponse, MonthlyReportQuery, MonthlyReportResponse, ReportSummaryQuery,
    TimeSeriesPoint,
};
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{Datelike, Duration, FixedOffset, NaiveDate, Utc};
use sqlx::Row;
use std::sync::Arc;

struct TimeBucket {
    label: String,
    start_millis: i64,
    end_millis: i64,
}

pub async fn monthly_report_handler(
    State(state): State<Arc<AppState>>,
    lang: Language,
    auth: AuthenticatedUser,
    Query(query): Query<MonthlyReportQuery>,
) -> AppResult<Json<MonthlyReportResponse>> {
    let vn_offset = FixedOffset::east_opt(7 * 3600).unwrap();
    let now_vn = Utc::now().with_timezone(&vn_offset);
    let year = query.year.unwrap_or_else(|| now_vn.year());
    let month = query.month.unwrap_or_else(|| now_vn.month()).clamp(1, 12);

    let start_date = NaiveDate::from_ymd_opt(year, month, 1)
        .ok_or_else(|| AppError::BadRequest("Năm hoặc tháng không hợp lệ".into()))?;
    let start_dt = start_date
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(vn_offset)
        .unwrap();
    let start_millis = start_dt.timestamp_millis();

    let next_month_date = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .unwrap();
    let end_dt = next_month_date
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(vn_offset)
        .unwrap();
    let end_millis = end_dt.timestamp_millis();

    let user_id = Some(auth.user_id);
    let cache_key = format!(
        "findora:cache:report:{}:{}:{}:{}:{}",
        user_id.as_deref().unwrap_or("anon"),
        year,
        month,
        query.wallet_id.as_deref().unwrap_or("all"),
        lang.code()
    );

    if let Some(cached_str) = state.redis.get(&cache_key).await {
        if let Ok(report) = serde_json::from_str::<MonthlyReportResponse>(&cached_str) {
            return Ok(Json(report));
        }
    }

    let (total_income, total_expense, expense_breakdown, income_breakdown, _) =
        fetch_report_aggregates(
            &*state.pool,
            user_id.as_deref(),
            query.wallet_id.as_deref(),
            start_millis,
            end_millis,
            lang,
            false,
        )
        .await?;

    let response = MonthlyReportResponse {
        year,
        month,
        total_income,
        total_expense,
        net_balance: total_income - total_expense,
        expense_breakdown,
        income_breakdown,
    };

    if let Ok(json_str) = serde_json::to_string(&response) {
        state.redis.set_ex(&cache_key, &json_str, 600).await;
    }

    Ok(Json(response))
}

pub async fn summary_report_handler(
    State(state): State<Arc<AppState>>,
    lang: Language,
    auth: AuthenticatedUser,
    Query(query): Query<ReportSummaryQuery>,
) -> AppResult<Json<FinancialReportResponse>> {
    let vn_offset = FixedOffset::east_opt(7 * 3600).unwrap();
    let now_vn = Utc::now().with_timezone(&vn_offset);

    let period = query
        .period
        .as_deref()
        .unwrap_or("month")
        .to_lowercase();

    // Determine reference date
    let mut ref_date = if let Some(ref d_str) = query.date {
        NaiveDate::parse_from_str(d_str, "%Y-%m-%d")
            .unwrap_or_else(|_| now_vn.date_naive())
    } else {
        now_vn.date_naive()
    };

    if let Some(y) = query.year {
        ref_date = NaiveDate::from_ymd_opt(y, query.month.unwrap_or(ref_date.month()), ref_date.day().min(28))
            .unwrap_or(ref_date);
    } else if let Some(m) = query.month {
        ref_date = NaiveDate::from_ymd_opt(ref_date.year(), m.clamp(1, 12), ref_date.day().min(28))
            .unwrap_or(ref_date);
    }

    let user_id = Some(auth.user_id);

    let (start_date, end_date, start_millis, end_millis, title, resp_year, resp_month, resp_week, buckets) =
        match period.as_str() {
            "week" => {
                let weekday = ref_date.weekday();
                let days_from_monday = weekday.num_days_from_monday();
                let week_start = ref_date - Duration::days(days_from_monday as i64);
                let week_end = week_start + Duration::days(6);
                let next_week_start = week_start + Duration::days(7);

                let s_dt = week_start.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(vn_offset).unwrap();
                let e_dt = next_week_start.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(vn_offset).unwrap();

                let iso_week = ref_date.iso_week().week();
                let title = match lang {
                    Language::Vi => format!(
                        "Tuần {} ({:02}/{:02} - {:02}/{:02})",
                        iso_week,
                        week_start.day(),
                        week_start.month(),
                        week_end.day(),
                        week_end.month()
                    ),
                    Language::En => format!(
                        "Week {} ({:02}/{:02} - {:02}/{:02})",
                        iso_week,
                        week_start.month(),
                        week_start.day(),
                        week_end.month(),
                        week_end.day()
                    ),
                };

                let labels_vi = ["T2", "T3", "T4", "T5", "T6", "T7", "CN"];
                let labels_en = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

                let mut b_list = Vec::with_capacity(7);
                for i in 0..7 {
                    let d_start = week_start + Duration::days(i);
                    let d_end = d_start + Duration::days(1);
                    let b_s = d_start.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(vn_offset).unwrap().timestamp_millis();
                    let b_e = d_end.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(vn_offset).unwrap().timestamp_millis();
                    let label = match lang {
                        Language::Vi => labels_vi[i as usize].to_string(),
                        Language::En => labels_en[i as usize].to_string(),
                    };
                    b_list.push(TimeBucket {
                        label,
                        start_millis: b_s,
                        end_millis: b_e,
                    });
                }

                (
                    week_start,
                    week_end,
                    s_dt.timestamp_millis(),
                    e_dt.timestamp_millis(),
                    title,
                    ref_date.year(),
                    Some(ref_date.month()),
                    Some(iso_week),
                    b_list,
                )
            }
            "year" => {
                let y = ref_date.year();
                let year_start = NaiveDate::from_ymd_opt(y, 1, 1).unwrap();
                let year_end = NaiveDate::from_ymd_opt(y, 12, 31).unwrap();
                let next_year_start = NaiveDate::from_ymd_opt(y + 1, 1, 1).unwrap();

                let s_dt = year_start.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(vn_offset).unwrap();
                let e_dt = next_year_start.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(vn_offset).unwrap();

                let title = match lang {
                    Language::Vi => format!("Năm {}", y),
                    Language::En => format!("Year {}", y),
                };

                let labels_vi = ["T1", "T2", "T3", "T4", "T5", "T6", "T7", "T8", "T9", "T10", "T11", "T12"];
                let labels_en = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

                let mut b_list = Vec::with_capacity(12);
                for m in 1..=12 {
                    let m_start = NaiveDate::from_ymd_opt(y, m, 1).unwrap();
                    let m_end = if m == 12 {
                        NaiveDate::from_ymd_opt(y + 1, 1, 1).unwrap()
                    } else {
                        NaiveDate::from_ymd_opt(y, m + 1, 1).unwrap()
                    };
                    let b_s = m_start.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(vn_offset).unwrap().timestamp_millis();
                    let b_e = m_end.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(vn_offset).unwrap().timestamp_millis();
                    let label = match lang {
                        Language::Vi => labels_vi[(m - 1) as usize].to_string(),
                        Language::En => labels_en[(m - 1) as usize].to_string(),
                    };
                    b_list.push(TimeBucket {
                        label,
                        start_millis: b_s,
                        end_millis: b_e,
                    });
                }

                (
                    year_start,
                    year_end,
                    s_dt.timestamp_millis(),
                    e_dt.timestamp_millis(),
                    title,
                    y,
                    None,
                    None,
                    b_list,
                )
            }
            _ => {
                // Default: "month"
                let y = ref_date.year();
                let m = ref_date.month();
                let month_start = NaiveDate::from_ymd_opt(y, m, 1).unwrap();
                let next_month_start = if m == 12 {
                    NaiveDate::from_ymd_opt(y + 1, 1, 1).unwrap()
                } else {
                    NaiveDate::from_ymd_opt(y, m + 1, 1).unwrap()
                };
                let month_end = next_month_start - Duration::days(1);

                let s_dt = month_start.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(vn_offset).unwrap();
                let e_dt = next_month_start.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(vn_offset).unwrap();

                let title = match lang {
                    Language::Vi => format!("Tháng {}/{}", m, y),
                    Language::En => format!("{}/{}", m, y),
                };

                let days_in_month = (next_month_start - month_start).num_days();
                let mut b_list = Vec::new();
                let intervals: Vec<(i64, i64, &str, &str)> = vec![
                    (1, 7, "Tuần 1", "Week 1"),
                    (8, 14, "Tuần 2", "Week 2"),
                    (15, 21, "Tuần 3", "Week 3"),
                    (22, 28, "Tuần 4", "Week 4"),
                    (29, days_in_month, "Tuần 5", "Week 5"),
                ];

                for (s_day, e_day, vi_lbl, en_lbl) in intervals {
                    if s_day <= days_in_month {
                        let actual_end_day = e_day.min(days_in_month);
                        let d_s = NaiveDate::from_ymd_opt(y, m, s_day as u32).unwrap();
                        let d_e = if actual_end_day == days_in_month {
                            next_month_start
                        } else {
                            NaiveDate::from_ymd_opt(y, m, (actual_end_day + 1) as u32).unwrap()
                        };

                        let b_s = d_s.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(vn_offset).unwrap().timestamp_millis();
                        let b_e = d_e.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(vn_offset).unwrap().timestamp_millis();
                        let label = match lang {
                            Language::Vi => vi_lbl.to_string(),
                            Language::En => en_lbl.to_string(),
                        };
                        b_list.push(TimeBucket {
                            label,
                            start_millis: b_s,
                            end_millis: b_e,
                        });
                    }
                }

                (
                    month_start,
                    month_end,
                    s_dt.timestamp_millis(),
                    e_dt.timestamp_millis(),
                    title,
                    y,
                    Some(m),
                    None,
                    b_list,
                )
            }
        };

    let cache_key = format!(
        "findora:cache:report:{}:{}:{}:{}:{}:{}:{}",
        user_id.as_deref().unwrap_or("anon"),
        period,
        resp_year,
        resp_month.unwrap_or(0),
        resp_week.unwrap_or(0),
        query.wallet_id.as_deref().unwrap_or("all"),
        lang.code()
    );

    if let Some(cached_str) = state.redis.get(&cache_key).await {
        if let Ok(report) = serde_json::from_str::<FinancialReportResponse>(&cached_str) {
            return Ok(Json(report));
        }
    }

    let (total_income, total_expense, expense_breakdown, income_breakdown, tx_rows) =
        fetch_report_aggregates(
            &*state.pool,
            user_id.as_deref(),
            query.wallet_id.as_deref(),
            start_millis,
            end_millis,
            lang,
            true,
        )
        .await?;

    // Build time series from buckets
    let mut time_series = Vec::with_capacity(buckets.len());
    for b in buckets {
        let mut b_inc: i64 = 0;
        let mut b_exp: i64 = 0;

        for (tx_ts, tx_type, tx_amt) in &tx_rows {
            if *tx_ts >= b.start_millis && *tx_ts < b.end_millis {
                if tx_type == "INCOME" {
                    b_inc += tx_amt;
                } else if tx_type == "EXPENSE" {
                    b_exp += tx_amt;
                }
            }
        }

        time_series.push(TimeSeriesPoint {
            label: b.label,
            income: b_inc,
            expense: b_exp,
        });
    }

    let response = FinancialReportResponse {
        period,
        title,
        start_date: start_date.format("%Y-%m-%d").to_string(),
        end_date: end_date.format("%Y-%m-%d").to_string(),
        year: resp_year,
        month: resp_month,
        week: resp_week,
        total_income,
        total_expense,
        net_balance: total_income - total_expense,
        expense_breakdown,
        income_breakdown,
        time_series,
    };

    if let Ok(json_str) = serde_json::to_string(&response) {
        state.redis.set_ex(&cache_key, &json_str, 600).await;
    }

    Ok(Json(response))
}

async fn fetch_report_aggregates(
    pool: &sqlx::PgPool,
    user_id: Option<&str>,
    wallet_id: Option<&str>,
    start_millis: i64,
    end_millis: i64,
    _lang: Language,
    fetch_tx_points: bool,
) -> AppResult<(i64, i64, Vec<CategorySummary>, Vec<CategorySummary>, Vec<(i64, String, i64)>)> {
    // 1. Totals
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
    .bind(user_id)
    .bind(wallet_id)
    .fetch_all(pool)
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

    // 2. Expense Breakdown
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
    .bind(user_id)
    .bind(wallet_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::Database)?;

    let expense_breakdown: Vec<CategorySummary> = expense_rows
        .into_iter()
        .map(|r| {
            let total: i64 = r.get("total_cat");
            let name_vi: String = r.get("cat_name");
            let name_en: Option<String> = r.get("cat_name_en");

            let percentage = if total_expense > 0 {
                ((total as f64 / total_expense as f64) * 1000.0).round() / 10.0
            } else {
                0.0
            };

            CategorySummary {
                category_id: r.get("cat_id"),
                category_name: name_vi,
                category_name_en: name_en,
                total_amount: total,
                percentage,
            }
        })
        .collect();

    // 3. Income Breakdown
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
    .bind(user_id)
    .bind(wallet_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::Database)?;

    let income_breakdown: Vec<CategorySummary> = income_rows
        .into_iter()
        .map(|r| {
            let total: i64 = r.get("total_cat");
            let name_vi: String = r.get("cat_name");
            let name_en: Option<String> = r.get("cat_name_en");

            let percentage = if total_income > 0 {
                ((total as f64 / total_income as f64) * 1000.0).round() / 10.0
            } else {
                0.0
            };

            CategorySummary {
                category_id: r.get("cat_id"),
                category_name: name_vi,
                category_name_en: name_en,
                total_amount: total,
                percentage,
            }
        })
        .collect();

    // 4. Time series transactions (if requested)
    let mut tx_points = Vec::new();
    if fetch_tx_points {
        let rows = sqlx::query(
            "SELECT t.timestamp, t.tx_type, t.amount
             FROM findora.transactions t
             INNER JOIN findora.wallets w ON t.wallet_id = w.id
             WHERE t.deleted_at IS NULL
               AND t.timestamp >= $1 
               AND t.timestamp < $2
               AND ($3::TEXT IS NULL OR w.user_id = $3)
               AND ($4::TEXT IS NULL OR t.wallet_id = $4)"
        )
        .bind(start_millis)
        .bind(end_millis)
        .bind(user_id)
        .bind(wallet_id)
        .fetch_all(pool)
        .await
        .map_err(AppError::Database)?;

        for r in rows {
            let ts: i64 = r.get("timestamp");
            let tt: String = r.get("tx_type");
            let amt: i64 = r.get("amount");
            tx_points.push((ts, tt, amt));
        }
    }

    Ok((total_income, total_expense, expense_breakdown, income_breakdown, tx_points))
}

pub async fn category_report_handler(
    State(state): State<Arc<AppState>>,
    Path(category_id): Path<String>,
    lang: Language,
    auth: AuthenticatedUser,
    Query(query): Query<CategoryReportQuery>,
) -> AppResult<Json<CategoryReportResponse>> {
    let vn_offset = FixedOffset::east_opt(7 * 3600).unwrap();
    let now_vn = Utc::now().with_timezone(&vn_offset);

    let year = query.year.unwrap_or_else(|| now_vn.year());
    let month = query.month.unwrap_or(12).clamp(1, 12);
    let months_count = query.months_count.unwrap_or(12).clamp(1, 12) as i32;

    let user_id = Some(auth.user_id);

    // Fetch category info
    let cat_row = sqlx::query(
        "SELECT id, name, name_en, icon, color, type FROM findora.categories WHERE id = $1"
    )
    .bind(&category_id)
    .fetch_optional(&*state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound("Không tìm thấy danh mục".into()))?;

    let cat_name: String = cat_row.get("name");
    let cat_name_en: Option<String> = cat_row.get("name_en");
    let cat_icon: String = cat_row.get("icon");
    let cat_color: String = cat_row.get("color");
    let cat_type: String = cat_row.get("type");

    let display_name = match lang {
        Language::En => cat_name_en.clone().unwrap_or_else(|| cat_name.clone()),
        Language::Vi => cat_name.clone(),
    };

    // Build the list of target months (ending with (year, month))
    // e.g. for (2026, 11) with count 5 -> 7, 8, 9, 10, 11
    let mut target_months: Vec<(i32, u32)> = Vec::new();
    let mut cur_y = year;
    let mut cur_m = month as i32;
    for _ in 0..months_count {
        target_months.push((cur_y, cur_m as u32));
        cur_m -= 1;
        if cur_m < 1 {
            cur_m = 12;
            cur_y -= 1;
        }
    }
    target_months.reverse();

    let mut monthly_history = Vec::with_capacity(target_months.len());
    let mut current_month_total: i64 = 0;

    for (y, m) in target_months {
        let m_start = NaiveDate::from_ymd_opt(y, m, 1).unwrap();
        let m_end = if m == 12 {
            NaiveDate::from_ymd_opt(y + 1, 1, 1).unwrap()
        } else {
            NaiveDate::from_ymd_opt(y, m + 1, 1).unwrap()
        };
        let s_dt = m_start.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(vn_offset).unwrap();
        let e_dt = m_end.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(vn_offset).unwrap();

        let row = sqlx::query(
            "SELECT COALESCE(SUM(t.amount), 0)::BIGINT as sum
             FROM findora.transactions t
             INNER JOIN findora.wallets w ON t.wallet_id = w.id
             WHERE t.deleted_at IS NULL
               AND t.category_id = $1
               AND t.timestamp >= $2
               AND t.timestamp < $3
               AND ($4::TEXT IS NULL OR w.user_id = $4)
               AND ($5::TEXT IS NULL OR t.wallet_id = $5)"
        )
        .bind(&category_id)
        .bind(s_dt.timestamp_millis())
        .bind(e_dt.timestamp_millis())
        .bind(&user_id)
        .bind(&query.wallet_id)
        .fetch_one(&*state.pool)
        .await
        .map_err(AppError::Database)?;

        let sum: i64 = row.get("sum");
        let label = match lang {
            Language::Vi => format!("T{}", m),
            Language::En => format!("M{}", m),
        };

        if y == year && m == month {
            current_month_total = sum;
        }

        monthly_history.push(CategoryMonthStat {
            year: y,
            month: m,
            label,
            total: sum,
        });
    }

    Ok(Json(CategoryReportResponse {
        category_id,
        category_name: display_name,
        category_name_en: cat_name_en,
        category_icon: cat_icon,
        category_color: cat_color,
        tx_type: cat_type,
        selected_year: year,
        selected_month: month,
        current_month_total,
        monthly_history,
    }))
}
