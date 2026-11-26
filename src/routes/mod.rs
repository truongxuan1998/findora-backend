pub mod auth;
pub mod categories;
pub mod health;
pub mod reports;
pub mod transactions;
pub mod translate;
pub mod users;
pub mod wallets;

use crate::AppState;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Health & System Info
        .route("/", get(health::root_handler))
        .route("/health", get(health::health_handler))
        // Authentication
        .route("/api/auth/register", post(auth::register_handler))
        .route("/api/auth/login", post(auth::login_handler))
        .route("/api/auth/logout", post(auth::logout_handler))
        .route("/api/auth/logout-all", post(auth::logout_all_handler))
        .route("/api/auth/refresh", post(auth::refresh_token_handler))
        .route("/api/auth/forgot-password", post(auth::forgot_password_handler))
        .route("/api/auth/reset-password", post(auth::reset_password_handler))
        .route("/api/auth/me", get(auth::me_handler))
        // User Profile & Avatar Upload (Multipart Form-Data)
        .route("/api/users/profile", get(users::get_profile_handler).put(users::update_profile_handler))
        .route("/api/users/avatar", post(users::upload_avatar_handler))
        // Categories
        .route(
            "/api/categories",
            get(categories::categories_handler).post(categories::create_category_handler),
        )
        // Wallets
        .route("/api/wallets", get(wallets::list_wallets_handler).post(wallets::create_wallet_handler))
        // Transactions
        .route(
            "/api/transactions",
            get(transactions::list_transactions_handler)
                .post(transactions::create_transaction_handler)
                .delete(transactions::clear_all_transactions_handler),
        )
        .route(
            "/api/transactions/:id",
            axum::routing::delete(transactions::delete_transaction_handler)
                .patch(transactions::update_transaction_note_handler),
        )
        // Reports
        .route("/api/reports/monthly", get(reports::monthly_report_handler))
        .route("/api/reports/summary", get(reports::summary_report_handler))
        .route("/api/reports/statistics", get(reports::summary_report_handler))
        .route("/api/reports/category/:id", get(reports::category_report_handler))
        // Translation API
        .route(
            "/api/translate",
            post(translate::translate_handler).get(translate::translate_get_handler),
        )
        .route(
            "/api/transactions/translate",
            post(translate::translate_transactions_handler),
        )
        // Rate Limiter DDoS Middleware: Giới hạn tần suất request theo IP (Strict cho Auth/Upload, General cho API)
        // Dùng from_fn_with_state để middleware truy cập được Redis (rate limit phân tán khi scale nhiều instance)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::rate_limiter::rate_limit_middleware,
        ))
        // Audit Middleware: Ghi nhận IP, Location, Device Name, OS Version cho mọi request
        .layer(axum::middleware::from_fn(crate::audit::audit_log_middleware))
        .with_state(state)
}
