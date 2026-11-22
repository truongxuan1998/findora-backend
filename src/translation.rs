use crate::redis_client::RedisService;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

pub struct TranslationService {
    redis: Arc<RedisService>,
    http_client: Client,
    vi_to_en: HashMap<&'static str, &'static str>,
    en_to_vi: HashMap<&'static str, &'static str>,
}

#[derive(Deserialize)]
struct MyMemoryApiResponse {
    #[serde(rename = "responseData")]
    response_data: Option<MyMemoryData>,
    #[allow(dead_code)]
    #[serde(rename = "responseStatus")]
    response_status: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct MyMemoryData {
    #[serde(rename = "translatedText")]
    translated_text: Option<String>,
}

impl TranslationService {
    pub fn new(redis: Arc<RedisService>) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(4))
            .user_agent("Findora-TranslationService/1.0")
            .build()
            .unwrap_or_else(|_| Client::new());

        // Từ điển chuyên ngành tài chính cá nhân & chi tiêu hàng ngày (Song ngữ Việt - Anh)
        let pairs: &[(&'static str, &'static str)] = &[
            // Ẩm thực, Ăn uống
            ("ăn uống", "Food & Beverage"),
            ("ăn sáng", "Breakfast"),
            ("ăn trưa", "Lunch"),
            ("ăn tối", "Dinner"),
            ("cơm trưa", "Lunch"),
            ("cơm tối", "Dinner"),
            ("cơm tấm", "Broken rice"),
            ("phở", "Pho"),
            ("bánh mì", "Banh mi"),
            ("cà phê", "Coffee"),
            ("cafe", "Coffee"),
            ("trà sữa", "Milk tea"),
            ("nước ngọt", "Soft drink"),
            ("trái cây", "Fruits"),
            ("hoa quả", "Fruits"),
            ("ăn vặt", "Snacks"),
            ("tiệc tùng", "Party"),
            ("nhậu", "Social gathering"),
            // Mua sắm & Sinh hoạt
            ("đi chợ", "Groceries"),
            ("siêu thị", "Supermarket"),
            ("mua sắm", "Shopping"),
            ("quần áo", "Clothing"),
            ("giày dép", "Footwear"),
            ("mỹ phẩm", "Cosmetics"),
            ("đồ gia dụng", "Housewares"),
            ("văn phòng phẩm", "Office supplies"),
            ("sách vở", "Books & Stationery"),
            // Nhà cửa & Hóa đơn tiện ích
            ("tiền nhà", "Rent"),
            ("thuê nhà", "House rent"),
            ("tiền phòng", "Room rent"),
            ("tiền điện", "Electricity bill"),
            ("tiền nước", "Water bill"),
            ("tiền mạng", "Internet bill"),
            ("tiền wifi", "Wifi fee"),
            ("tiền rác", "Waste disposal fee"),
            ("phí quản lý", "Management fee"),
            ("phí dịch vụ", "Service fee"),
            // Đi lại & Phương tiện
            ("xăng xe", "Fuel"),
            ("tiền xăng", "Gasoline"),
            ("đổ xăng", "Refuel"),
            ("gửi xe", "Parking fee"),
            ("rửa xe", "Car wash"),
            ("sửa xe", "Vehicle repair"),
            ("bảo dưỡng xe", "Vehicle maintenance"),
            ("tiền xe", "Transportation"),
            ("đi lại", "Commute"),
            ("tiền grab", "Grab ride"),
            ("taxi", "Taxi"),
            ("xe buýt", "Bus fare"),
            ("vé tàu", "Train ticket"),
            ("vé máy bay", "Flight ticket"),
            // Thu nhập
            ("tiền lương", "Salary"),
            ("lương", "Salary"),
            ("lương tháng", "Monthly salary"),
            ("lương cứng", "Base salary"),
            ("thưởng", "Bonus"),
            ("thưởng tết", "Tet bonus"),
            ("hoa hồng", "Commission"),
            ("thu nhập phụ", "Side income"),
            ("tiền làm thêm", "Overtime pay"),
            ("bán hàng", "Sales income"),
            ("lợi nhuận", "Profit"),
            ("tiền lãi", "Interest income"),
            ("cổ tức", "Dividends"),
            // Đầu tư & Tiết kiệm
            ("tiết kiệm", "Savings"),
            ("gửi tiết kiệm", "Savings deposit"),
            ("đầu tư", "Investment"),
            ("chứng khoán", "Securities"),
            ("cổ phiếu", "Stocks"),
            ("vàng", "Gold"),
            ("bất động sản", "Real estate"),
            ("quỹ", "Funds"),
            // Sức khỏe & Y tế
            ("y tế", "Healthcare"),
            ("thuốc men", "Medicine"),
            ("mua thuốc", "Buy medicine"),
            ("khám bệnh", "Medical checkup"),
            ("bệnh viện", "Hospital fee"),
            ("nha khoa", "Dental care"),
            ("tiêm phòng", "Vaccination"),
            ("bảo hiểm", "Insurance"),
            ("bảo hiểm y tế", "Health insurance"),
            // Giáo dục & Học tập
            ("giáo dục", "Education"),
            ("học phí", "Tuition fee"),
            ("khóa học", "Course fee"),
            ("học tiếng anh", "English class"),
            // Giải trí, Thể thao & Du lịch
            ("giải trí", "Entertainment"),
            ("xem phim", "Movie ticket"),
            ("du lịch", "Travel"),
            ("khách sạn", "Hotel"),
            ("tập gym", "Gym membership"),
            ("thể thao", "Sports"),
            ("cắt tóc", "Haircut"),
            ("làm đẹp", "Beauty care"),
            ("spa", "Spa"),
            ("thú cưng", "Pet care"),
            // Gia đình, Xã hội & Khác
            ("quà tặng", "Gifts"),
            ("sinh nhật", "Birthday gift"),
            ("đám cưới", "Wedding gift"),
            ("mừng tuổi", "Lucky money"),
            ("lì xì", "Lucky money"),
            ("từ thiện", "Charity"),
            ("cho vay", "Lending"),
            ("vay mượn", "Loan"),
            ("trả nợ", "Debt repayment"),
            ("trả góp", "Installment payment"),
            ("chuyển khoản", "Bank transfer"),
            ("phí chuyển tiền", "Transfer fee"),
            ("rút tiền", "Cash withdrawal"),
            ("khác", "Other"),
        ];

        let mut vi_to_en = HashMap::new();
        let mut en_to_vi = HashMap::new();

        for &(vi, en) in pairs {
            vi_to_en.entry(vi).or_insert(en);
            en_to_vi.entry(en).or_insert(vi);
        }

        Self {
            redis,
            http_client,
            vi_to_en,
            en_to_vi,
        }
    }

    /// Tự động phát hiện cặp ngôn ngữ nguồn và đích
    pub fn detect_languages(&self, text: &str, requested_target: Option<&str>) -> (&'static str, &'static str) {
        let is_vietnamese = text.chars().any(|c| {
            matches!(
                c,
                'à' | 'á' | 'ả' | 'ã' | 'ạ' |
                'ă' | 'ằ' | 'ắ' | 'ẳ' | 'ẵ' | 'ặ' |
                'â' | 'ầ' | 'ấ' | 'ẩ' | 'ẫ' | 'ậ' |
                'đ' |
                'è' | 'é' | 'ẻ' | 'ẽ' | 'ẹ' |
                'ê' | 'ề' | 'ế' | 'ể' | 'ễ' | 'ệ' |
                'ì' | 'í' | 'ỉ' | 'ĩ' | 'ị' |
                'ò' | 'ó' | 'ỏ' | 'õ' | 'ọ' |
                'ô' | 'ồ' | 'ố' | 'ổ' | 'ỗ' | 'ộ' |
                'ơ' | 'ờ' | 'ớ' | 'ở' | 'ỡ' | 'ợ' |
                'ù' | 'ú' | 'ủ' | 'ũ' | 'ụ' |
                'ư' | 'ừ' | 'ứ' | 'ử' | 'ữ' | 'ự' |
                'ỳ' | 'ý' | 'ỷ' | 'ỹ' | 'ỵ' |
                'À' | 'Á' | 'Ả' | 'Ã' | 'Ạ' |
                'Ă' | 'Ằ' | 'Ắ' | 'Ẳ' | 'Ẵ' | 'Ặ' |
                'Â' | 'Ầ' | 'Ấ' | 'Ẩ' | 'Ẫ' | 'Ậ' |
                'Đ' |
                'È' | 'É' | 'Ẻ' | 'Ẽ' | 'Ẹ' |
                'Ê' | 'Ề' | 'Ế' | 'Ể' | 'Ễ' | 'Ệ' |
                'Ì' | 'Í' | 'Ỉ' | 'Ĩ' | 'Ị' |
                'Ò' | 'Ó' | 'Ỏ' | 'Õ' | 'Ọ' |
                'Ô' | 'Ồ' | 'Ố' | 'Ổ' | 'Ỗ' | 'Ộ' |
                'Ơ' | 'Ờ' | 'Ớ' | 'Ở' | 'Ỡ' | 'Ợ' |
                'Ù' | 'Ú' | 'Ủ' | 'Ũ' | 'Ụ' |
                'Ư' | 'Ừ' | 'Ứ' | 'Ử' | 'Ữ' | 'Ự' |
                'Ỳ' | 'Ý' | 'Ỷ' | 'Ỹ' | 'Ỵ'
            )
        });

        if let Some(target) = requested_target {
            let target_lower = target.trim().to_lowercase();
            if target_lower == "vi" {
                ("en", "vi")
            } else {
                ("vi", "en")
            }
        } else if is_vietnamese {
            ("vi", "en")
        } else {
            ("en", "vi")
        }
    }

    /// Dịch đơn lẻ một chuỗi văn bản (có hỗ trợ cache Redis & từ điển chuyên ngành)
    pub async fn translate(&self, text: &str, target: &str, source: Option<&str>) -> String {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return String::new();
        }

        let (src_lang, tgt_lang) = if let Some(s) = source {
            let s_lower = s.trim().to_lowercase();
            let t_lower = target.trim().to_lowercase();
            let final_s = if s_lower == "vi" { "vi" } else { "en" };
            let final_t = if t_lower == "vi" { "vi" } else { "en" };
            (final_s, final_t)
        } else {
            self.detect_languages(trimmed, Some(target))
        };

        if src_lang == tgt_lang {
            return trimmed.to_string();
        }

        let normalized_key = trimmed.to_lowercase();

        // 1. Kiểm tra từ điển tài chính địa phương (0ms, không tốn network)
        if src_lang == "vi" && tgt_lang == "en" {
            if let Some(&found) = self.vi_to_en.get(normalized_key.as_str()) {
                return found.to_string();
            }
        } else if src_lang == "en" && tgt_lang == "vi" {
            // Tra cứu case-insensitive trong en_to_vi
            for (&en_term, &vi_term) in &self.en_to_vi {
                if en_term.eq_ignore_ascii_case(&normalized_key) {
                    return vi_term.to_string();
                }
            }
        }

        // 2. Kiểm tra Redis Cache
        let cache_key = format!("findora:translate:{}:{}:{}", src_lang, tgt_lang, normalized_key);
        if let Some(cached) = self.redis.get(&cache_key).await {
            if !cached.trim().is_empty() {
                debug!(text = %trimmed, cached = %cached, "Cache Hit bản dịch");
                return cached;
            }
        }

        // 3. Gọi Translation Engine API (MyMemory API)
        let langpair = format!("{}|{}", src_lang, tgt_lang);
        let api_url = "https://api.mymemory.translated.net/get";

        let response = self
            .http_client
            .get(api_url)
            .query(&[("q", trimmed), ("langpair", &langpair)])
            .send()
            .await;

        match response {
            Ok(res) if res.status().is_success() => {
                if let Ok(json) = res.json::<MyMemoryApiResponse>().await {
                    if let Some(data) = json.response_data {
                        if let Some(raw_trans) = data.translated_text {
                            let clean_trans = decode_html_entities(&raw_trans);
                            if !clean_trans.is_empty() && !clean_trans.starts_with("MYMEMORY WARNING:") {
                                // Lưu cache Redis trong 30 ngày (86400 * 30 giây)
                                self.redis
                                    .set_ex(&cache_key, &clean_trans, 86400 * 30)
                                    .await;
                                return clean_trans;
                            }
                        }
                    }
                }
            }
            Ok(res) => {
                warn!(
                    status = %res.status(),
                    text = %trimmed,
                    "Translation API trả mã lỗi"
                );
            }
            Err(e) => {
                warn!(
                    error = %e,
                    text = %trimmed,
                    "Lỗi kết nối tới Translation API (sử dụng fallback)"
                );
            }
        }

        // 4. Fallback an toàn: Giữ nguyên văn bản gốc
        trimmed.to_string()
    }

    /// Số request dịch tối đa chạy đồng thời (bound concurrency để tránh bị MyMemory rate-limit
    /// và tránh cạn file descriptor / áp lực RAM khi batch lớn).
    const MAX_TRANSLATE_CONCURRENCY: usize = 8;

    /// Dịch hàng loạt danh sách văn bản (song song có giới hạn, giữ nguyên thứ tự đầu vào)
    pub async fn translate_batch(
        &self,
        texts: &[String],
        target: &str,
        source: Option<&str>,
    ) -> Vec<String> {
        // Chia thành từng nhóm MAX_TRANSLATE_CONCURRENCY phần tử, mỗi nhóm dịch song song (join_all).
        // Giữ nguyên thứ tự đầu vào, giới hạn số request đồng thời tới MyMemory API.
        let mut results = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(Self::MAX_TRANSLATE_CONCURRENCY) {
            let part = futures::future::join_all(
                chunk.iter().map(|text| self.translate(text, target, source)),
            )
            .await;
            results.extend(part);
        }
        results
    }
}

fn decode_html_entities(s: &str) -> String {
    s.replace("&#10;", "\n")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}
