pub mod auth;
pub mod categories;
pub mod health;
pub mod reports;
pub mod transactions;
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
        .route("/api/auth/me", get(auth::me_handler))
        // User Profile & Avatar Upload (Multipart Form-Data)
        .route("/api/users/profile", get(users::get_profile_handler).put(users::update_profile_handler))
        .route("/api/users/avatar", post(users::upload_avatar_handler))
        // Categories
        .route("/api/categories", get(categories::categories_handler))
        // Wallets
        .route("/api/wallets", get(wallets::list_wallets_handler).post(wallets::create_wallet_handler))
        // Transactions
        .route("/api/transactions", get(transactions::list_transactions_handler).post(transactions::create_transaction_handler))
        // Reports
        .route("/api/reports/monthly", get(reports::monthly_report_handler))
        // Audit Middleware: Ghi nhận IP, Location, Device Name, OS Version cho mọi request
        .layer(axum::middleware::from_fn(crate::audit::audit_log_middleware))
        .with_state(state)
}
