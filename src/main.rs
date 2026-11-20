mod audit;
mod auth;
mod config;
mod db;
mod errors;
mod i18n;
mod image_validator;
mod models;
mod routes;
mod storage;

use config::Config;
use dotenvy::dotenv;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub struct AppState {
    pub pool: Arc<sqlx::PgPool>,
    pub config: Config,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    dotenvy::from_filename("backend/.env").ok();
    dotenvy::from_filename(".env").ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "findora_backend=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("🚀 Khởi động Findora High-Performance Backend (Rust + Axum + Tokio)...");

    let config = Config::from_env().map_err(|e| {
        error!("❌ Cấu hình không hợp lệ: {}", e);
        e
    })?;

    // Kết nối Database Pool với connection constraints tối ưu cho Cloud free-tier
    let pool = db::create_pool(&config.database_url).await?;

    // Chạy cơ chế self-healing & migration schema findora
    db::run_schema_and_migrations(&pool).await?;

    let state = Arc::new(AppState {
        pool: Arc::new(pool),
        config: config.clone(),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = routes::create_router(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("🌟 Findora REST API Server đang lắng nghe tại: http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Hỗ trợ Graceful Shutdown khi deploy trên Cloud (Render, Docker, Kubernetes)
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("👋 Findora Backend đã dừng an toàn (Graceful shutdown complete).");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Không thể bắt sự kiện Ctrl+C");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Không thể bắt sự kiện SIGTERM")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("🛑 Nhận tín hiệu dừng (SIGINT/SIGTERM), đang hoàn tất các request còn dở...");
}
