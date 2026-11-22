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
    // Key là tiếng Anh viết thường → tra cứu O(1) (thay vì quét tuyến tính)
    en_to_vi: HashMap<String, &'static str>,
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
            en_to_vi.entry(en.to_lowercase()).or_insert(vi);
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

    /// Phân giải cặp ngôn ngữ nguồn → đích.
    fn resolve_langs(&self, trimmed: &str, target: &str, source: Option<&str>) -> (&'static str, &'static str) {
        if let Some(s) = source {
            let s_lower = s.trim().to_lowercase();
            let t_lower = target.trim().to_lowercase();
            let final_s = if s_lower == "vi" { "vi" } else { "en" };
            let final_t = if t_lower == "vi" { "vi" } else { "en" };
            (final_s, final_t)
        } else {
            self.detect_languages(trimmed, Some(target))
        }
    }

    /// Tra từ điển tài chính địa phương (0ms, không network). `normalized_key` phải viết thường.
    fn local_translate(&self, normalized_key: &str, src_lang: &str, tgt_lang: &str) -> Option<String> {
        if src_lang == "vi" && tgt_lang == "en" {
            self.vi_to_en.get(normalized_key).map(|s| s.to_string())
        } else if src_lang == "en" && tgt_lang == "vi" {
            self.en_to_vi.get(normalized_key).map(|s| s.to_string())
        } else {
            None
        }
    }

    /// Gọi MyMemory API dịch 1 chuỗi, lưu cache nếu thành công; fallback giữ nguyên gốc.
    async fn translate_via_api(&self, trimmed: &str, src_lang: &str, tgt_lang: &str, cache_key: &str) -> String {
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
                                self.redis.set_ex(cache_key, &clean_trans, 86400 * 30).await;
                                return clean_trans;
                            }
                        }
                    }
                }
            }
            Ok(res) => {
                warn!(status = %res.status(), text = %trimmed, "Translation API trả mã lỗi");
            }
            Err(e) => {
                warn!(error = %e, text = %trimmed, "Lỗi kết nối tới Translation API (sử dụng fallback)");
            }
        }

        // Fallback an toàn: giữ nguyên văn bản gốc
        trimmed.to_string()
    }

    /// Dịch đơn lẻ một chuỗi văn bản (từ điển → cache Redis → MyMemory API).
    pub async fn translate(&self, text: &str, target: &str, source: Option<&str>) -> String {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return String::new();
        }

        let (src_lang, tgt_lang) = self.resolve_langs(trimmed, target, source);
        if src_lang == tgt_lang {
            return trimmed.to_string();
        }

        let normalized_key = trimmed.to_lowercase();

        // 1. Từ điển địa phương
        if let Some(hit) = self.local_translate(&normalized_key, src_lang, tgt_lang) {
            return hit;
        }

        // 2. Redis cache
        let cache_key = format!("findora:translate:{}:{}:{}", src_lang, tgt_lang, normalized_key);
        if let Some(cached) = self.redis.get(&cache_key).await {
            if !cached.trim().is_empty() {
                debug!(text = %trimmed, cached = %cached, "Cache Hit bản dịch");
                return cached;
            }
        }

        // 3 + 4. API + fallback
        self.translate_via_api(trimmed, src_lang, tgt_lang, &cache_key).await
    }

    /// Số request dịch tối đa chạy đồng thời (bound concurrency để tránh bị MyMemory rate-limit
    /// và tránh cạn file descriptor / áp lực RAM khi batch lớn).
    const MAX_TRANSLATE_CONCURRENCY: usize = 8;

    /// Dịch hàng loạt danh sách văn bản (dedup + song song có giới hạn, giữ nguyên thứ tự đầu vào)
    pub async fn translate_batch(
        &self,
        texts: &[String],
        target: &str,
        source: Option<&str>,
    ) -> Vec<String> {
        // 1. Gom các ghi chú DUY NHẤT để mỗi chuỗi chỉ dịch đúng 1 lần — tránh gọi
        //    trùng API/Redis cho ghi chú lặp (rất phổ biến: "Ăn trưa", "Cà phê"...).
        let mut unique: Vec<&str> = Vec::new();
        let mut index_of: HashMap<&str, usize> = HashMap::new();
        let slots: Vec<usize> = texts
            .iter()
            .map(|t| {
                let key = t.as_str();
                *index_of.entry(key).or_insert_with(|| {
                    let idx = unique.len();
                    unique.push(key);
                    idx
                })
            })
            .collect();

        // 2. Phân giải từng unique: kết quả tức thì (same-lang/từ điển) hoặc cần tra cache Redis.
        let mut translated_unique: Vec<Option<String>> = vec![None; unique.len()];
        // Mỗi phần tử cần tra: (chỉ số trong unique, trimmed, src, tgt, cache_key)
        let mut lookups: Vec<(usize, String, &'static str, &'static str, String)> = Vec::new();

        for (uidx, &note) in unique.iter().enumerate() {
            let trimmed = note.trim();
            if trimmed.is_empty() {
                translated_unique[uidx] = Some(String::new());
                continue;
            }
            let (src, tgt) = self.resolve_langs(trimmed, target, source);
            if src == tgt {
                translated_unique[uidx] = Some(trimmed.to_string());
                continue;
            }
            let normalized = trimmed.to_lowercase();
            if let Some(hit) = self.local_translate(&normalized, src, tgt) {
                translated_unique[uidx] = Some(hit);
                continue;
            }
            let cache_key = format!("findora:translate:{}:{}:{}", src, tgt, normalized);
            lookups.push((uidx, trimmed.to_string(), src, tgt, cache_key));
        }

        // 3. MGET toàn bộ cache key trong 1 round-trip; tách hit / miss.
        let keys: Vec<String> = lookups.iter().map(|l| l.4.clone()).collect();
        let cached = self.redis.mget(&keys).await;

        let mut misses: Vec<(usize, String, &'static str, &'static str, String)> = Vec::new();
        for (l, cv) in lookups.into_iter().zip(cached) {
            match cv {
                Some(v) if !v.trim().is_empty() => translated_unique[l.0] = Some(v),
                _ => misses.push(l),
            }
        }

        // 4. Gọi API cho các cache miss, song song có giới hạn concurrency.
        for chunk in misses.chunks(Self::MAX_TRANSLATE_CONCURRENCY) {
            let results = futures::future::join_all(
                chunk.iter().map(|m| self.translate_via_api(&m.1, m.2, m.3, &m.4)),
            )
            .await;
            for (m, res) in chunk.iter().zip(results) {
                translated_unique[m.0] = Some(res);
            }
        }

        // 5. Map ngược về đúng thứ tự & số lượng đầu vào.
        let translated_unique: Vec<String> =
            translated_unique.into_iter().map(|o| o.unwrap_or_default()).collect();
        slots.iter().map(|&idx| translated_unique[idx].clone()).collect()
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
