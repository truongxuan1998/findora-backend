use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub jwt_secret: String,
    pub env: String,
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

        Ok(Self {
            database_url,
            port,
            jwt_secret,
            env,
        })
    }
}
