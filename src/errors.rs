use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug)]
#[allow(dead_code)]
pub enum AppError {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    Internal(String),
    Database(sqlx::Error),
}

pub type AppResult<T> = Result<T, AppError>;

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(ref db_err) = err {
            // PostgreSQL Unique Violation Code is 23505
            if db_err.code().as_deref() == Some("23505") {
                return AppError::Conflict("Dữ liệu đã tồn tại trong hệ thống (Duplicate record)".into());
            }
        }
        AppError::Database(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message, code) = match self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg, "BAD_REQUEST"),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg, "UNAUTHORIZED"),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg, "FORBIDDEN"),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg, "NOT_FOUND"),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg, "CONFLICT"),
            AppError::Internal(msg) => {
                tracing::error!("Lỗi hệ thống nội bộ: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Lỗi máy chủ nội bộ".to_string(), "INTERNAL_ERROR")
            }
            AppError::Database(err) => {
                tracing::error!("Lỗi cơ sở dữ liệu: {}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "Lỗi kết nối hoặc thực thi cơ sở dữ liệu".to_string(), "DATABASE_ERROR")
            }
        };

        let body = Json(json!({
            "success": false,
            "code": code,
            "message": message,
        }));

        (status, body).into_response()
    }
}
