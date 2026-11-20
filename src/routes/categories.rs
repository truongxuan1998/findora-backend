use crate::errors::{AppError, AppResult};
use crate::models::category::CategoryItem;
use crate::AppState;
use axum::{extract::State, Json};
use sqlx::Row;
use std::sync::Arc;

pub async fn categories_handler(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<CategoryItem>>> {
    let rows = sqlx::query("SELECT id, name, icon, color, type FROM findora.categories ORDER BY type, id")
        .fetch_all(&*state.pool)
        .await
        .map_err(AppError::Database)?;

    let cats = rows
        .into_iter()
        .map(|r| CategoryItem {
            id: r.get("id"),
            name: r.get("name"),
            icon: r.get("icon"),
            color: r.get("color"),
            r#type: r.get("type"),
        })
        .collect();

    Ok(Json(cats))
}
