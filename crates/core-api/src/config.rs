use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub nats_url: String,
    pub jwt_secret: String,
    pub panel_domain: String,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        // Load .env jika ada
        let _ = dotenvy::dotenv();

        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://panel_dev:dev_password_123@localhost:5432/panel_dev".to_string()),
            nats_url: std::env::var("NATS_URL")
                .unwrap_or_else(|_| "nats://localhost:4222".to_string()),
            jwt_secret: std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| "super_secret_jwt_key_change_me_in_production".to_string()),
            panel_domain: std::env::var("PANEL_DOMAIN")
                .unwrap_or_else(|_| "localhost".to_string()),
        })
    }
}
