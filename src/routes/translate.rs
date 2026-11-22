use crate::auth::AuthenticatedUser;
use crate::errors::{AppError, AppResult};
use crate::models::translation::{
    TransactionNoteTranslation, TranslateQuery, TranslateRequest, TranslateResponse,
    TranslateTransactionsRequest, TranslateTransactionsResponse, TranslationItemResult,
};
use crate::AppState;
use axum::{
    extract::{Query, State},
    Json,
};
use std::sync::Arc;

/// 1. Dịch văn bản chung (POST /api/translate)
pub async fn translate_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TranslateRequest>,
) -> AppResult<Json<TranslateResponse>> {
    let target = if payload.target.trim().is_empty() {
        "en"
    } else {
        payload.target.trim()
    };
    let source_opt = payload.source.as_deref().filter(|s| !s.trim().is_empty());

    let mut single_original = None;
    let mut single_translated = None;
    let mut translations_vec = Vec::new();

    // Xử lý dịch đơn nếu có trường `text`
    if let Some(ref text) = payload.text {
        if !text.trim().is_empty() {
            let translated = state
                .translation
                .translate(text, target, source_opt)
                .await;
            single_original = Some(text.clone());
            single_translated = Some(translated.clone());
            translations_vec.push(TranslationItemResult {
                original: text.clone(),
                translated,
            });
        }
    }

    // Xử lý dịch hàng loạt nếu có danh sách `texts`
    if let Some(ref list) = payload.texts {
        for item in list {
            if item.trim().is_empty() {
                continue;
            }
            let trans = state
                .translation
                .translate(item, target, source_opt)
                .await;
            translations_vec.push(TranslationItemResult {
                original: item.clone(),
                translated: trans,
            });
        }
    }

    let detected_source = source_opt.unwrap_or_else(|| {
        if target.eq_ignore_ascii_case("vi") {
            "en"
        } else {
            "vi"
        }
    });

    Ok(Json(TranslateResponse {
        original_text: single_original,
        translated_text: single_translated,
        translations: translations_vec,
        source_lang: detected_source.to_string(),
        target_lang: target.to_string(),
    }))
}

/// 2. Dịch nhanh bằng phương thức GET (GET /api/translate?text=...&target=en)
pub async fn translate_get_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TranslateQuery>,
) -> AppResult<Json<TranslateResponse>> {
    let target = query.target.as_deref().unwrap_or("en");
    let source_opt = query.source.as_deref();

    let translated = state
        .translation
        .translate(&query.text, target, source_opt)
        .await;

    let detected_source = source_opt.unwrap_or_else(|| {
        if target.eq_ignore_ascii_case("vi") {
            "en"
        } else {
            "vi"
        }
    });

    Ok(Json(TranslateResponse {
        original_text: Some(query.text.clone()),
        translated_text: Some(translated.clone()),
        translations: vec![TranslationItemResult {
            original: query.text,
            translated,
        }],
        source_lang: detected_source.to_string(),
        target_lang: target.to_string(),
    }))
}

/// 3. Dịch đồng bộ toàn bộ ghi chú giao dịch thu chi của người dùng (POST /api/transactions/translate)
pub async fn translate_transactions_handler(
    State(state): State<Arc<AppState>>,
    auth: AuthenticatedUser,
    Json(payload): Json<TranslateTransactionsRequest>,
) -> AppResult<Json<TranslateTransactionsResponse>> {
    let target = if payload.target_lang.trim().is_empty() {
        "en"
    } else {
        payload.target_lang.trim()
    };
    let limit = payload.limit.unwrap_or(100).clamp(1, 500);

    // Lấy danh sách giao dịch cần dịch
    let rows = if let Some(ref ids) = payload.transaction_ids {
        if ids.is_empty() {
            vec![]
        } else {
            sqlx::query(
                "SELECT t.id, t.note 
                 FROM findora.transactions t
                 INNER JOIN findora.wallets w ON t.wallet_id = w.id
                 WHERE w.user_id = $1 
                   AND t.id = ANY($2)
                   AND t.deleted_at IS NULL
                   AND t.note != ''"
            )
            .bind(&auth.user_id)
            .bind(ids)
            .fetch_all(&*state.pool)
            .await
            .map_err(AppError::Database)?
        }
    } else {
        sqlx::query(
            "SELECT t.id, t.note 
             FROM findora.transactions t
             INNER JOIN findora.wallets w ON t.wallet_id = w.id
             WHERE w.user_id = $1 
               AND t.deleted_at IS NULL
               AND t.note != ''
             ORDER BY t.timestamp DESC
             LIMIT $2"
        )
        .bind(&auth.user_id)
        .bind(limit)
        .fetch_all(&*state.pool)
        .await
        .map_err(AppError::Database)?
    };

    use sqlx::Row;
    let items: Vec<(String, String)> = rows
        .into_iter()
        .map(|r| (r.get::<String, _>("id"), r.get::<String, _>("note")))
        .collect();

    // Dịch song song toàn bộ ghi chú (thay vì tuần tự) để giảm đáng kể thời gian phản hồi
    let notes: Vec<String> = items.iter().map(|(_, note)| note.clone()).collect();
    let translated_notes = state.translation.translate_batch(&notes, target, None).await;

    let translations: Vec<TransactionNoteTranslation> = items
        .into_iter()
        .zip(translated_notes)
        .map(|((id, note), translated)| TransactionNoteTranslation {
            id,
            original_note: note,
            translated_note: translated,
        })
        .collect();

    Ok(Json(TranslateTransactionsResponse {
        success: true,
        target_lang: target.to_string(),
        count: translations.len(),
        translations,
    }))
}
