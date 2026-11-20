use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use bcrypt::{hash, verify, DEFAULT_COST};
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgConnectOptions;
use sqlx::{PgPool, Row};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub pool: Arc<PgPool>,
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    #[serde(alias = "fullName")]
    pub full_name: String,
    pub password: String,
    #[serde(alias = "phoneNumber", default)]
    pub phone_number: Option<String>,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub full_name: String,
    pub phone_number: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub success: bool,
    pub message: String,
    pub user: Option<UserResponse>,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct CategoryItem {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub color: String,
    pub r#type: String,
}

#[derive(Deserialize)]
pub struct MonthlyReportQuery {
    pub wallet_id: Option<String>,
    pub year: Option<i32>,
    pub month: Option<u32>,
}

#[derive(Serialize)]
pub struct MonthlyReportResponse {
    pub year: i32,
    pub month: u32,
    pub total_income: i64,
    pub total_expense: i64,
    pub net_balance: i64,
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

    info!("🚀 Khởi động Findora Rust Cloud Backend...");

    let db_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL biến môi trường không được bỏ trống (Supabase PostgreSQL)");

    let connect_opts = PgConnectOptions::from_str(&db_url)?
        .statement_cache_capacity(0);

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(15))
        .connect_with(connect_opts)
        .await
        .expect("❌ Không thể kết nối tới Supabase PostgreSQL!");

    info!("✅ Kết nối Supabase PostgreSQL thành công!");

    // Tự động đảm bảo schema và bảng users tồn tại
    ensure_schema(&pool).await?;

    let state = AppState {
        pool: Arc::new(pool),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(root_handler))
        .route("/health", get(health_handler))
        .route("/api/auth/register", post(register_handler))
        .route("/api/auth/login", post(login_handler))
        .route("/api/categories", get(categories_handler))
        .route("/api/reports/monthly", get(monthly_report_handler))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "10000".to_string())
        .parse()
        .unwrap_or(10000);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("🌟 Findora REST API Server đang lắng nghe tại: http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn ensure_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::raw_sql("
        CREATE SCHEMA IF NOT EXISTS findora;

        CREATE TABLE IF NOT EXISTS findora.users (
            id TEXT PRIMARY KEY,
            email TEXT UNIQUE NOT NULL,
            full_name TEXT NOT NULL,
            phone_number TEXT DEFAULT '',
            password_hash TEXT NOT NULL,
            avatar_url TEXT DEFAULT '',
            created_at TIMESTAMPTZ DEFAULT NOW(),
            updated_at TIMESTAMPTZ DEFAULT NOW()
        );

        CREATE INDEX IF NOT EXISTS idx_users_email ON findora.users(email);
    ")
    .execute(pool)
    .await?;

    info!("🎯 Bảng findora.users trên Supabase đã sẵn sàng.");
    Ok(())
}

async fn root_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "service": "Findora High-Performance Backend",
        "runtime": "Rust (Axum + Tokio + SQLx)",
        "database": "Supabase PostgreSQL (Schema: findora)",
        "version": "1.0.0",
        "status": "online"
    }))
}

async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query("SELECT count(*) as cnt FROM findora.categories").fetch_one(&*state.pool).await {
        Ok(row) => {
            let cnt: i64 = row.get("cnt");
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "status": "ok",
                    "database": "connected",
                    "categories_count": cnt
                })),
            )
        }
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "error",
                "database_error": e.to_string()
            })),
        ),
    }
}

async fn register_handler(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> (StatusCode, Json<AuthResponse>) {
    let email = payload.email.trim().to_lowercase();
    let name = payload.full_name.trim();

    if email.is_empty() || !email.contains('@') {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthResponse {
                success: false,
                message: "Email không đúng định dạng".to_string(),
                user: None,
            }),
        );
    }

    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthResponse {
                success: false,
                message: "Họ và tên không được để trống".to_string(),
                user: None,
            }),
        );
    }

    if payload.password.len() < 6 {
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthResponse {
                success: false,
                message: "Mật khẩu phải chứa ít nhất 6 ký tự".to_string(),
                user: None,
            }),
        );
    }

    // Kiểm tra email đã tồn tại chưa
    let existing = sqlx::query("SELECT id FROM findora.users WHERE email = $1")
        .bind(&email)
        .fetch_optional(&*state.pool)
        .await;

    if let Ok(Some(_)) = existing {
        return (
            StatusCode::CONFLICT,
            Json(AuthResponse {
                success: false,
                message: "Email này đã được đăng ký tài khoản Findora".to_string(),
                user: None,
            }),
        );
    }

    // Băm mật khẩu bằng bcrypt
    let hashed_pw = match hash(&payload.password, DEFAULT_COST) {
        Ok(h) => h,
        Err(e) => {
            error!("Lỗi mã hóa mật khẩu: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthResponse {
                    success: false,
                    message: "Lỗi bảo mật khi tạo tài khoản".to_string(),
                    user: None,
                }),
            );
        }
    };

    let user_id = format!("usr_{}", Uuid::new_v4().simple());
    let phone = payload.phone_number.unwrap_or_default();

    // Lưu người dùng vào Supabase
    let insert_res = sqlx::query(
        "INSERT INTO findora.users (id, email, full_name, phone_number, password_hash)
         VALUES ($1, $2, $3, $4, $5)"
    )
    .bind(&user_id)
    .bind(&email)
    .bind(name)
    .bind(&phone)
    .bind(&hashed_pw)
    .execute(&*state.pool)
    .await;

    match insert_res {
        Ok(_) => {
            info!(user_id = %user_id, email = %email, "🎉 Người dùng mới đăng ký thành công trên Supabase!");

            // Tự động tạo ví mặc định cho người dùng mới
            let wallet_id = format!("wal_{}", Uuid::new_v4().simple());
            let _ = sqlx::query(
                "INSERT INTO findora.wallets (id, name, currency, initial_balance, current_balance)
                 VALUES ($1, 'Ví tiền mặt', 'VND', 0, 0)
                 ON CONFLICT DO NOTHING"
            )
            .bind(&wallet_id)
            .execute(&*state.pool)
            .await;

            (
                StatusCode::CREATED,
                Json(AuthResponse {
                    success: true,
                    message: "Đăng ký tài khoản thành công!".to_string(),
                    user: Some(UserResponse {
                        id: user_id,
                        email,
                        full_name: name.to_string(),
                        phone_number: phone,
                    }),
                }),
            )
        }
        Err(e) => {
            error!("Lỗi insert Supabase: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthResponse {
                    success: false,
                    message: format!("Lỗi lưu cơ sở dữ liệu: {}", e),
                    user: None,
                }),
            )
        }
    }
}

async fn login_handler(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> (StatusCode, Json<AuthResponse>) {
    let email = payload.email.trim().to_lowercase();

    let user_row = sqlx::query(
        "SELECT id, email, full_name, phone_number, password_hash 
         FROM findora.users 
         WHERE email = $1"
    )
    .bind(&email)
    .fetch_optional(&*state.pool)
    .await;

    match user_row {
        Ok(Some(row)) => {
            let pw_hash: String = row.get("password_hash");
            let is_valid = verify(&payload.password, &pw_hash).unwrap_or(false);

            if is_valid {
                let user_id: String = row.get("id");
                let full_name: String = row.get("full_name");
                let phone_number: String = row.get("phone_number");

                info!(user_id = %user_id, email = %email, "🔐 Người dùng đăng nhập thành công!");

                (
                    StatusCode::OK,
                    Json(AuthResponse {
                        success: true,
                        message: "Đăng nhập thành công".to_string(),
                        user: Some(UserResponse {
                            id: user_id,
                            email,
                            full_name,
                            phone_number,
                        }),
                    }),
                )
            } else {
                warn!(email = %email, "Mật khẩu không chính xác");
                (
                    StatusCode::UNAUTHORIZED,
                    Json(AuthResponse {
                        success: false,
                        message: "Email hoặc mật khẩu không chính xác".to_string(),
                        user: None,
                    }),
                )
            }
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(AuthResponse {
                success: false,
                message: "Tài khoản không tồn tại trong hệ thống".to_string(),
                user: None,
            }),
        ),
        Err(e) => {
            error!("Lỗi truy vấn đăng nhập: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthResponse {
                    success: false,
                    message: "Lỗi kết nối cơ sở dữ liệu".to_string(),
                    user: None,
                }),
            )
        }
    }
}

async fn categories_handler(State(state): State<AppState>) -> impl IntoResponse {
    let rows = sqlx::query("SELECT id, name, icon, color, type FROM findora.categories ORDER BY type, id")
        .fetch_all(&*state.pool)
        .await;

    match rows {
        Ok(rows) => {
            let cats: Vec<CategoryItem> = rows
                .into_iter()
                .map(|r| CategoryItem {
                    id: r.get("id"),
                    name: r.get("name"),
                    icon: r.get("icon"),
                    color: r.get("color"),
                    r#type: r.get("type"),
                })
                .collect();
            (StatusCode::OK, Json(cats))
        }
        Err(e) => {
            error!("Lỗi lấy danh mục: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(vec![]))
        }
    }
}

async fn monthly_report_handler(
    State(state): State<AppState>,
    Query(query): Query<MonthlyReportQuery>,
) -> impl IntoResponse {
    let year = query.year.unwrap_or(2026);
    let month = query.month.unwrap_or(11);

    let rows = sqlx::query(
        "SELECT tx_type, COALESCE(SUM(amount), 0) as sum
         FROM findora.transactions
         WHERE deleted_at IS NULL
         GROUP BY tx_type"
    )
    .fetch_all(&*state.pool)
    .await;

    let mut total_income: i64 = 0;
    let mut total_expense: i64 = 0;

    if let Ok(rows) = rows {
        for r in rows {
            let t: String = r.get("tx_type");
            let s: i64 = r.get("sum");
            match t.as_str() {
                "INCOME" => total_income = s,
                "EXPENSE" => total_expense = s,
                _ => {}
            }
        }
    }

    Json(MonthlyReportResponse {
        year,
        month,
        total_income,
        total_expense,
        net_balance: total_income - total_expense,
    })
}
