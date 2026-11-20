use crate::errors::{AppError, AppResult};
use crate::i18n::Language;
use crate::models::category::CategoryItem;
use crate::AppState;
use axum::{extract::State, Json};
use sqlx::Row;
use std::sync::Arc;

pub async fn categories_handler(
    State(state): State<Arc<AppState>>,
    lang: Language,
) -> AppResult<Json<Vec<CategoryItem>>> {
    let cache_key = format!("findora:cache:categories:{}", lang.code());

    // 1. Thử lấy từ Redis Cache (Cache Hit -> ~1ms)
    if let Some(cached_str) = state.redis.get(&cache_key).await {
        if let Ok(cats) = serde_json::from_str::<Vec<CategoryItem>>(&cached_str) {
            return Ok(Json(cats));
        }
    }

    // 2. Cache Miss -> Truy vấn PostgreSQL
    let rows = sqlx::query("SELECT id, name, name_en, icon, color, type FROM findora.categories ORDER BY type, id")
        .fetch_all(&*state.pool)
        .await
        .map_err(AppError::Database)?;

    let cats: Vec<CategoryItem> = rows
        .into_iter()
        .map(|r| {
            let name_vi: String = r.get("name");
            let name_en: Option<String> = r.get("name_en");

            // Tự động chọn tên danh mục phù hợp theo ngôn ngữ của Client
            let display_name = match lang {
                Language::En => name_en.clone().unwrap_or_else(|| name_vi.clone()),
                Language::Vi => name_vi.clone(),
            };

            CategoryItem {
                id: r.get("id"),
                name: display_name,
                name_en,
                icon: r.get("icon"),
                color: r.get("color"),
                r#type: r.get("type"),
            }
        })
        .collect();

    // 3. Ghi vào Redis Cache với TTL 1 giờ (3600 giây)
    if let Ok(json_str) = serde_json::to_string(&cats) {
        state.redis.set_ex(&cache_key, &json_str, 3600).await;
    }

    Ok(Json(cats))
}
