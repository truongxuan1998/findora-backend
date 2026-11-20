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
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let database_url = env::var("DATABASE_URL")
            .map_err(|_| "Biến môi trường DATABASE_URL không được để trống (Supabase PostgreSQL)!".to_string())?;

        let port = env::var("PORT")
            .unwrap_or_else(|_| "10000".to_string())
            .parse::<u16>()
            .unwrap_or(10000);

        let jwt_secret = env::var("JWT_SECRET")
            .unwrap_or_else(|_| "findora_super_secret_jwt_key_2026_change_in_prod".to_string());

        let env = env::var("APP_ENV")
            .unwrap_or_else(|_| "production".to_string());

        // Cấu hình Supabase Storage
        let supabase_url = env::var("SUPABASE_URL")
            .unwrap_or_else(|_| "https://jbqwbiqrylkiwqhjwgfn.supabase.co".to_string());

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
        })
    }
}
