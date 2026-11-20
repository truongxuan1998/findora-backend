use crate::auth::AuthenticatedUser;
use crate::errors::{AppError, AppResult};
use crate::i18n::{self, Language};
use crate::models::transaction::{CreateTransactionRequest, TransactionItem, TransactionQuery};
use crate::AppState;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

pub async fn list_transactions_handler(
    State(state): State<Arc<AppState>>,
    auth: AuthenticatedUser,
    Query(query): Query<TransactionQuery>,
) -> AppResult<Json<Vec<TransactionItem>>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);

    let rows = sqlx::query(
        "SELECT t.id, t.wallet_id, t.amount, t.currency, t.tx_type, t.category_id, 
                c.name as category_name, t.timestamp, t.note, t.created_at, t.updated_at, t.deleted_at
         FROM findora.transactions t
         INNER JOIN findora.wallets w ON t.wallet_id = w.id
         LEFT JOIN findora.categories c ON t.category_id = c.id
         WHERE w.user_id = $1 
           AND t.deleted_at IS NULL
           AND ($2::TEXT IS NULL OR t.wallet_id = $2)
           AND ($3::TEXT IS NULL OR t.category_id = $3)
         ORDER BY t.timestamp DESC
         LIMIT $4 OFFSET $5"
    )
    .bind(&auth.user_id)
    .bind(&query.wallet_id)
    .bind(&query.category_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&*state.pool)
    .await
    .map_err(AppError::Database)?;

    let txs = rows
        .into_iter()
        .map(|r| TransactionItem {
            id: r.get("id"),
            wallet_id: r.get("wallet_id"),
            amount: r.get("amount"),
            currency: r.get("currency"),
            tx_type: r.get("tx_type"),
            category_id: r.get("category_id"),
            category_name: r.get("category_name"),
            timestamp: r.get("timestamp"),
            note: r.get("note"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            deleted_at: r.get("deleted_at"),
        })
        .collect();

    Ok(Json(txs))
}

pub async fn create_transaction_handler(
    State(state): State<Arc<AppState>>,
    lang: Language,
    auth: AuthenticatedUser,
    Json(payload): Json<CreateTransactionRequest>,
) -> AppResult<(StatusCode, Json<TransactionItem>)> {
    if payload.amount <= 0 {
        return Err(AppError::BadRequest(i18n::msg_invalid_amount(lang).into()));
    }

    let tx_type = payload.tx_type.to_uppercase();
    if !["EXPENSE", "INCOME", "TRANSFER"].contains(&tx_type.as_str()) {
        return Err(AppError::BadRequest(i18n::msg_invalid_tx_type(lang).into()));
    }

    let mut tx = state.pool.begin().await.map_err(AppError::Database)?;

    // 1. Xác định và kiểm tra quyền sở hữu wallet_id của người dùng
    let wallet_id = match payload.wallet_id {
        Some(wid) => {
            let exists = sqlx::query("SELECT id FROM findora.wallets WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL")
                .bind(&wid)
                .bind(&auth.user_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(AppError::Database)?;

            if exists.is_none() {
                return Err(AppError::NotFound(i18n::msg_wallet_not_found(lang).into()));
            }
            wid
        }
        None => {
            // Lấy ví mặc định đầu tiên của user
            let first_wallet = sqlx::query("SELECT id FROM findora.wallets WHERE user_id = $1 AND deleted_at IS NULL ORDER BY created_at ASC LIMIT 1")
                .bind(&auth.user_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(AppError::Database)?;

            match first_wallet {
                Some(w) => w.get("id"),
                None => return Err(AppError::BadRequest(i18n::msg_no_wallet_available(lang).into())),
            }
        }
    };

    let now_millis = chrono::Utc::now().timestamp_millis();
    let timestamp = payload.timestamp.unwrap_or(now_millis);
    let tx_id = format!("tx_{}", Uuid::new_v4().simple());
    let note = payload.note.unwrap_or_default();

    // 2. Insert transaction
    sqlx::query(
        "INSERT INTO findora.transactions (id, wallet_id, amount, currency, tx_type, category_id, timestamp, note, created_at, updated_at, created_at_tz)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9, to_timestamp($7 / 1000.0))"
    )
    .bind(&tx_id)
    .bind(&wallet_id)
    .bind(payload.amount)
    .bind(&payload.currency)
    .bind(&tx_type)
    .bind(&payload.category_id)
    .bind(timestamp)
    .bind(&note)
    .bind(now_millis)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Database)?;

    // 3. Tự động cập nhật current_balance của ví trong cùng 1 ACID transaction
    let balance_delta = match tx_type.as_str() {
        "INCOME" => payload.amount,
        "EXPENSE" => -payload.amount,
        _ => 0,
    };

    if balance_delta != 0 {
        sqlx::query("UPDATE findora.wallets SET current_balance = current_balance + $1, updated_at = NOW() WHERE id = $2")
            .bind(balance_delta)
            .bind(&wallet_id)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;
    }

    tx.commit().await.map_err(AppError::Database)?;

    // Xóa Cache danh sách ví của user trong Redis vì số dư ví (current_balance) vừa thay đổi
    state
        .redis
        .del(&format!("findora:cache:wallets:{}", auth.user_id))
        .await;

    let item = TransactionItem {
        id: tx_id,
        wallet_id,
        amount: payload.amount,
        currency: payload.currency,
        tx_type,
        category_id: payload.category_id,
        category_name: None,
        timestamp,
        note,
        created_at: now_millis,
        updated_at: now_millis,
        deleted_at: None,
    };

    Ok((StatusCode::CREATED, Json(item)))
}
