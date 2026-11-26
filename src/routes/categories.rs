use crate::auth::AuthenticatedUser;
use crate::errors::{AppError, AppResult};
use crate::i18n::Language;
use crate::models::category::{CategoryItem, CreateCategoryRequest};
use crate::AppState;
use axum::{extract::State, http::StatusCode, Json};
use sqlx::{postgres::PgRow, Row};
use std::sync::Arc;
use uuid::Uuid;

fn category_from_row(r: &PgRow) -> CategoryItem {
    CategoryItem {
        id: r.get("id"),
        name: r.get("name"),
        name_en: r.get("name_en"),
        icon: r.get("icon"),
        color: r.get("color"),
        r#type: r.get("type"),
    }
}

pub async fn categories_handler(
    State(state): State<Arc<AppState>>,
    lang: Language,
    auth: AuthenticatedUser,
) -> AppResult<Json<Vec<CategoryItem>>> {
    let cache_key = format!("findora:cache:categories:{}:{}", auth.user_id, lang.code());

    // 1. Thử lấy từ Redis Cache (Cache Hit -> ~1ms)
    if let Some(cached_str) = state.redis.get(&cache_key).await {
        if let Ok(cats) = serde_json::from_str::<Vec<CategoryItem>>(&cached_str) {
            return Ok(Json(cats));
        }
    }

    // 2. Cache Miss -> Truy vấn PostgreSQL (mặc định global + danh mục riêng của user)
    let rows = sqlx::query(
        "SELECT id, name, name_en, icon, color, type
         FROM findora.categories
         WHERE user_id IS NULL OR user_id = $1
         ORDER BY type, id",
    )
    .bind(&auth.user_id)
    .fetch_all(&*state.pool)
    .await
    .map_err(AppError::Database)?;

    let cats: Vec<CategoryItem> = rows.iter().map(category_from_row).collect();

    // 3. Ghi vào Redis Cache với TTL 1 giờ (3600 giây)
    if let Ok(json_str) = serde_json::to_string(&cats) {
        state.redis.set_ex(&cache_key, &json_str, 3600).await;
    }

    Ok(Json(cats))
}

pub async fn create_category_handler(
    State(state): State<Arc<AppState>>,
    lang: Language,
    auth: AuthenticatedUser,
    Json(payload): Json<CreateCategoryRequest>,
) -> AppResult<(StatusCode, Json<CategoryItem>)> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest(match lang {
            Language::Vi => "Tên danh mục không được để trống".to_string(),
            Language::En => "Category name cannot be empty".to_string(),
        }));
    }

    let icon = payload.icon.trim();
    if icon.is_empty() {
        return Err(AppError::BadRequest(match lang {
            Language::Vi => "Icon danh mục không được để trống".to_string(),
            Language::En => "Category icon cannot be empty".to_string(),
        }));
    }

    let color = payload.color.trim();
    if color.is_empty() {
        return Err(AppError::BadRequest(match lang {
            Language::Vi => "Màu sắc danh mục không được để trống".to_string(),
            Language::En => "Category color cannot be empty".to_string(),
        }));
    }

    let cat_type = payload.r#type.trim().to_uppercase();
    if cat_type != "EXPENSE" && cat_type != "INCOME" {
        return Err(AppError::BadRequest(match lang {
            Language::Vi => "Loại danh mục không hợp lệ ('EXPENSE' hoặc 'INCOME')".to_string(),
            Language::En => "Invalid category type ('EXPENSE' or 'INCOME')".to_string(),
        }));
    }

    let name_en = payload
        .name_en
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let id = format!("cat_{}", Uuid::new_v4().simple());

    let row = sqlx::query(
        "INSERT INTO findora.categories (id, name, name_en, icon, color, type, user_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id, name, name_en, icon, color, type",
    )
    .bind(&id)
    .bind(name)
    .bind(&name_en)
    .bind(icon)
    .bind(color)
    .bind(&cat_type)
    .bind(&auth.user_id)
    .fetch_one(&*state.pool)
    .await
    .map_err(AppError::from)?;

    state
        .redis
        .del_pattern(&format!("findora:cache:categories:{}:*", auth.user_id))
        .await;

    Ok((StatusCode::CREATED, Json(category_from_row(&row))))
}
