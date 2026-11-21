use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct MonthlyReportQuery {
    pub wallet_id: Option<String>,
    pub year: Option<i32>,
    pub month: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct ReportSummaryQuery {
    pub period: Option<String>, // "week", "month", "year" (default "month")
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub week: Option<u32>,
    pub date: Option<String>,   // YYYY-MM-DD anchor date
    pub wallet_id: Option<String>,
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
pub struct TimeSeriesPoint {
    pub label: String,
    pub income: i64,
    pub expense: i64,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FinancialReportResponse {
    pub period: String,
    pub title: String,
    #[serde(rename = "startDate")]
    pub start_date: String,
    #[serde(rename = "endDate")]
    pub end_date: String,
    pub year: i32,
    #[serde(rename = "month", skip_serializing_if = "Option::is_none")]
    pub month: Option<u32>,
    #[serde(rename = "week", skip_serializing_if = "Option::is_none")]
    pub week: Option<u32>,
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
    #[serde(rename = "timeSeries")]
    pub time_series: Vec<TimeSeriesPoint>,
}

#[derive(Debug, Deserialize)]
pub struct CategoryReportQuery {
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub months_count: Option<u32>,
    pub wallet_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CategoryMonthStat {
    pub year: i32,
    pub month: u32,
    pub label: String,
    pub total: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CategoryReportResponse {
    #[serde(rename = "categoryId")]
    pub category_id: String,
    #[serde(rename = "categoryName")]
    pub category_name: String,
    #[serde(rename = "categoryNameEn", skip_serializing_if = "Option::is_none")]
    pub category_name_en: Option<String>,
    #[serde(rename = "categoryIcon")]
    pub category_icon: String,
    #[serde(rename = "categoryColor")]
    pub category_color: String,
    #[serde(rename = "txType")]
    pub tx_type: String,
    #[serde(rename = "selectedYear")]
    pub selected_year: i32,
    #[serde(rename = "selectedMonth")]
    pub selected_month: u32,
    #[serde(rename = "currentMonthTotal")]
    pub current_month_total: i64,
    #[serde(rename = "monthlyHistory")]
    pub monthly_history: Vec<CategoryMonthStat>,
}
