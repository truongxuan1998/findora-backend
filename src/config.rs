use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub jwt_secret: String,
    pub env: String,
    pub supabase_url: String,
    pub supabase_key: String,
    pub supabase_bucket: String,
    pub redis_url: Option<String>,
    pub upstash_redis_rest_url: Option<String>,
    pub upstash_redis_rest_token: Option<String>,
    pub resend_api_key: Option<String>,
    pub resend_from: String,
    pub brevo_api_key: Option<String>,
    pub brevo_sender_email: String,
    pub brevo_sender_name: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let database_url = env::var("DATABASE_URL")
            .map_err(|_| "Biến môi trường DATABASE_URL không được để trống (Supabase PostgreSQL)!".to_string())?;

        let port = env::var("PORT")
            .unwrap_or_else(|_| "10000".to_string())
            .parse::<u16>()
            .unwrap_or(10000);

        let env = env::var("APP_ENV")
            .unwrap_or_else(|_| "production".to_string());

        // JWT_SECRET: bắt buộc phải cấu hình trong production, tuyệt đối không dùng secret mặc định
        let jwt_secret = match env::var("JWT_SECRET") {
            Ok(s) if !s.trim().is_empty() => s,
            _ => {
                if env == "production" {
                    return Err(
                        "JWT_SECRET bắt buộc phải được cấu hình trong môi trường production (không có giá trị mặc định vì lý do bảo mật)!".to_string()
                    );
                }
                eprintln!("⚠️  JWT_SECRET chưa được cấu hình — dùng secret mặc định CHỈ dành cho development.");
                "findora_dev_only_secret_do_not_use_in_prod".to_string()
            }
        };

        // Cấu hình Supabase Storage (không hardcode project URL thật trong source)
        let supabase_url = env::var("SUPABASE_URL").unwrap_or_default();

        let supabase_key = env::var("SUPABASE_SERVICE_KEY")
            .or_else(|_| env::var("SUPABASE_KEY"))
            .unwrap_or_default();

        let supabase_bucket = env::var("SUPABASE_STORAGE_BUCKET")
            .unwrap_or_else(|_| "findora_avatar_user".to_string());

        let redis_url = env::var("REDIS_URL")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let upstash_redis_rest_url = env::var("UPSTASH_REDIS_REST_URL")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let upstash_redis_rest_token = env::var("UPSTASH_REDIS_REST_TOKEN")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let resend_api_key = env::var("RESEND_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let resend_from = env::var("RESEND_FROM")
            .unwrap_or_else(|_| "Findora Security <onboarding@resend.dev>".to_string());

        let brevo_api_key = env::var("BREVO_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let brevo_sender_email = env::var("BREVO_SENDER_EMAIL")
            .unwrap_or_else(|_| "security@findora.app".to_string());

        let brevo_sender_name = env::var("BREVO_SENDER_NAME")
            .unwrap_or_else(|_| "Findora Security".to_string());

        Ok(Self {
            database_url,
            port,
            jwt_secret,
            env,
            supabase_url,
            supabase_key,
            supabase_bucket,
            redis_url,
            upstash_redis_rest_url,
            upstash_redis_rest_token,
            resend_api_key,
            resend_from,
            brevo_api_key,
            brevo_sender_email,
            brevo_sender_name,
        })
    }
}
