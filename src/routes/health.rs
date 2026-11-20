use crate::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
use sqlx::Row;
use std::sync::Arc;

pub async fn root_handler() -> impl IntoResponse {
    Json(json!({
        "service": "Findora High-Performance Backend",
        "runtime": "Rust (Axum + Tokio + SQLx)",
        "database": "Supabase PostgreSQL (Schema: findora)",
        "version": "1.0.0",
        "status": "online"
    }))
}

pub async fn health_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let redis_status = if state.redis.is_enabled() {
        if state.redis.ping().await {
            "connected"
        } else {
            "unreachable"
        }
    } else {
        "disabled"
    };

    match sqlx::query("SELECT count(*) as cnt FROM findora.categories").fetch_one(&*state.pool).await {
        Ok(row) => {
            let cnt: i64 = row.get("cnt");
            (
                StatusCode::OK,
                Json(json!({
                    "status": "ok",
                    "database": "connected",
                    "redis": redis_status,
                    "categories_count": cnt
                })),
            )
        }
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "error",
                "database_error": e.to_string(),
                "redis": redis_status
            })),
        ),
    }
}
