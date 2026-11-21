use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct TranslateRequest {
    pub text: Option<String>,
    pub texts: Option<Vec<String>>,
    pub source: Option<String>,
    #[serde(default = "default_target_lang")]
    pub target: String,
}

fn default_target_lang() -> String {
    "en".to_string()
}

#[derive(Debug, Deserialize)]
pub struct TranslateQuery {
    pub text: String,
    pub source: Option<String>,
    pub target: Option<String>,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct TranslationItemResult {
    pub original: String,
    pub translated: String,
}

#[derive(Debug, Serialize)]
pub struct TranslateResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translated_text: Option<String>,
    pub translations: Vec<TranslationItemResult>,
    pub source_lang: String,
    pub target_lang: String,
}

#[derive(Debug, Deserialize)]
pub struct TranslateTransactionsRequest {
    #[serde(default = "default_target_lang")]
    pub target_lang: String,
    pub transaction_ids: Option<Vec<String>>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct TranslateTransactionsResponse {
    pub success: bool,
    pub target_lang: String,
    pub count: usize,
    pub translations: Vec<TransactionNoteTranslation>,
}

#[derive(Debug, Serialize)]
pub struct TransactionNoteTranslation {
    pub id: String,
    pub original_note: String,
    pub translated_note: String,
}
