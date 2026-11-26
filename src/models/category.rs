use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CategoryItem {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_en: Option<String>,
    pub icon: String,
    pub color: String,
    pub r#type: String, // 'EXPENSE' hoặc 'INCOME'
}

#[derive(Debug, Deserialize)]
pub struct CreateCategoryRequest {
    pub name: String,
    #[serde(default, alias = "nameEn")]
    pub name_en: Option<String>,
    pub icon: String,
    pub color: String,
    pub r#type: String, // "EXPENSE" | "INCOME"
}
