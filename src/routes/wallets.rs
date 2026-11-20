use crate::auth::AuthenticatedUser;
use crate::errors::{AppError, AppResult};
use crate::models::wallet::{CreateWalletRequest, WalletItem};
use crate::AppState;
use axum::{extract::State, http::StatusCode, Json};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

pub async fn list_wallets_handler(
    State(state): State<Arc<AppState>>,
    auth: AuthenticatedUser,
) -> AppResult<Json<Vec<WalletItem>>> {
    let rows = sqlx::query(
        "SELECT id, user_id, name, currency, initial_balance, current_balance, created_at, updated_at
         FROM findora.wallets
         WHERE user_id = $1 AND deleted_at IS NULL
         ORDER BY created_at ASC"
    )
    .bind(&auth.user_id)
    .fetch_all(&*state.pool)
    .await
    .map_err(AppError::Database)?;

    let wallets = rows
        .into_iter()
        .map(|r| WalletItem {
            id: r.get("id"),
            user_id: r.get("user_id"),
            name: r.get("name"),
            currency: r.get("currency"),
            initial_balance: r.get("initial_balance"),
            current_balance: r.get("current_balance"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .collect();

    Ok(Json(wallets))
}

pub async fn create_wallet_handler(
    State(state): State<Arc<AppState>>,
    auth: AuthenticatedUser,
    Json(payload): Json<CreateWalletRequest>,
) -> AppResult<(StatusCode, Json<WalletItem>)> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Tên ví không được để trống".into()));
    }

    let wallet_id = format!("wal_{}", Uuid::new_v4().simple());
    let currency = if payload.currency.trim().is_empty() { "VND" } else { payload.currency.trim() };

    let row = sqlx::query(
        "INSERT INTO findora.wallets (id, user_id, name, currency, initial_balance, current_balance)
         VALUES ($1, $2, $3, $4, $5, $5)
         RETURNING id, user_id, name, currency, initial_balance, current_balance, created_at, updated_at"
    )
    .bind(&wallet_id)
    .bind(&auth.user_id)
    .bind(name)
    .bind(currency)
    .bind(payload.initial_balance)
    .fetch_one(&*state.pool)
    .await
    .map_err(AppError::Database)?;

    let wallet = WalletItem {
        id: row.get("id"),
        user_id: row.get("user_id"),
        name: row.get("name"),
        currency: row.get("currency"),
        initial_balance: row.get("initial_balance"),
        current_balance: row.get("current_balance"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    };

    Ok((StatusCode::CREATED, Json(wallet)))
}
