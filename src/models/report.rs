use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct MonthlyReportQuery {
    pub wallet_id: Option<String>,
    pub year: Option<i32>,
    pub month: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CategorySummary {
    #[serde(rename = "categoryId")]
    pub category_id: String,
    #[serde(rename = "categoryName")]
    pub category_name: String,
    #[serde(rename = "categoryNameEn", skip_serializing_if = "Option::is_none")]
    pub category_name_en: Option<String>,
    #[serde(rename = "totalAmount")]
    pub total_amount: i64,
    pub percentage: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MonthlyReportResponse {
    pub year: i32,
    pub month: u32,
    #[serde(rename = "totalIncome")]
    pub total_income: i64,
    #[serde(rename = "totalExpense")]
    pub total_expense: i64,
    #[serde(rename = "netBalance")]
    pub net_balance: i64,
    #[serde(rename = "expenseBreakdown")]
    pub expense_breakdown: Vec<CategorySummary>,
    #[serde(rename = "incomeBreakdown")]
    pub income_breakdown: Vec<CategorySummary>,
}
