use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WalletItem {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub currency: String,
    pub initial_balance: i64,
    pub current_balance: i64,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateWalletRequest {
    pub name: String,
    #[serde(default = "default_currency")]
    pub currency: String,
    #[serde(default)]
    pub initial_balance: i64,
}

fn default_currency() -> String {
    "VND".to_string()
}
