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
    let rows = sqlx::query("SELECT id, name, name_en, icon, color, type FROM findora.categories ORDER BY type, id")
        .fetch_all(&*state.pool)
        .await
        .map_err(AppError::Database)?;

    let cats = rows
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

    Ok(Json(cats))
}
