use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransactionItem {
    pub id: String,
    pub wallet_id: String,
    pub amount: i64,
    pub currency: String,
    pub tx_type: String, // 'EXPENSE', 'INCOME', 'TRANSFER'
    pub category_id: String,
    pub category_name: Option<String>,
    pub timestamp: i64,  // Unix millis
    pub note: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translated_note: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTransactionRequest {
    pub wallet_id: Option<String>,
    /// Ví đích — bắt buộc khi tx_type = "TRANSFER"
    #[serde(default, alias = "toWalletId")]
    pub to_wallet_id: Option<String>,
    pub amount: i64,
    #[serde(default = "default_currency")]
    pub currency: String,
    pub tx_type: String, // "EXPENSE", "INCOME", "TRANSFER"
    pub category_id: String,
    pub note: Option<String>,
    pub timestamp: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct TransactionQuery {
    pub wallet_id: Option<String>,
    pub category_id: Option<String>,
    pub translate_to: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

fn default_currency() -> String {
    "VND".to_string()
}
