use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CategoryItem {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub color: String,
    pub r#type: String, // 'EXPENSE' hoặc 'INCOME'
}
