use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    #[serde(alias = "fullName")]
    pub full_name: String,
    pub password: String,
    #[serde(alias = "confirmPassword", default)]
    pub confirm_password: Option<String>,
    #[serde(alias = "phoneNumber", default)]
    pub phone_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub full_name: String,
    pub phone_number: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<UserResponse>,
}
