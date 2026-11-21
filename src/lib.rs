pub mod audit;
pub mod auth;
pub mod config;
pub mod db;
pub mod email;
pub mod errors;
pub mod i18n;
pub mod image_validator;
pub mod models;
pub mod rate_limiter;
pub mod redis_client;
pub mod routes;
pub mod storage;
pub mod translation;

use config::Config;
use email::EmailService;
use redis_client::RedisService;
use std::sync::Arc;
use translation::TranslationService;

pub struct AppState {
    pub pool: Arc<sqlx::PgPool>,
    pub config: Config,
    pub redis: Arc<RedisService>,
    pub email: Arc<EmailService>,
    pub translation: Arc<TranslationService>,
}
