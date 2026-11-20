pub mod audit;
pub mod auth;
pub mod config;
pub mod db;
pub mod errors;
pub mod i18n;
pub mod image_validator;
pub mod models;
pub mod rate_limiter;
pub mod routes;
pub mod storage;

use config::Config;
use std::sync::Arc;

pub struct AppState {
    pub pool: Arc<sqlx::PgPool>,
    pub config: Config,
}
