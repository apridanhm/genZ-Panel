use anyhow::Result;
use async_nats::Client;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::env;
use std::fs;
use std::process::Command;
use tracing::{info, error};
use uuid::Uuid;
use futures::StreamExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DomainCreated {
    domain_id: Uuid,
    domain_name: String,
    user_id: Uuid,
    ssl_enabled: bool,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone)]
struct SslDaemon {
    nats: Client,
    db: PgPool,
    ssl_base_dir: String,
}

impl SslDaemon {
    async fn new() -> Result<Self> {
        let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let ssl_base_dir = env::var("SSL_BASE_DIR").unwrap_or_else(|_| "/home/genZ-panel/apps/ssl/letsencrypt".to_string());

        info!("Connecting to NATS at {}", nats_url);
        let nats = async_nats::connect(&nats_url).await?;
        info!("Connected to NATS");

        info!("Connecting to database");
        let db = PgPool::connect(&database_url).await?;
        info!("Connected to database");

        fs::create_dir_all(&ssl_base_dir)?;
        info!("SSL base directory ready at {}", ssl_base_dir);

        Ok(Self { nats, db, ssl_base_dir })
    }

    async fn handle_domain_created(&self, event: DomainCreated) -> Result<()> {
        info!("Checking SSL provisioning for domain: {}", event.domain_name);

        let domain = sqlx::query!(
            r#"
            SELECT id, domain_name, ssl_enabled, ssl_provider 
            FROM domains 
            WHERE id = $1
            "#,
            event.domain_id
        )
        .fetch_optional(&self.db)
        .await?;

        if let Some(d) = domain {
            if d.ssl_enabled && d.ssl_provider.as_deref() == Some("letsencrypt") {
                info!("🔒 Initiating Let's Encrypt SSL provisioning for: {}", d.domain_name);
                
                // Buat direktori untuk domain ini
                let domain_ssl_dir = format!("{}/{}", self.ssl_base_dir, event.domain_id);
                fs::create_dir_all(&domain_ssl_dir)?;

                let cert_path = format!("{}/cert.pem", domain_ssl_dir);
                let key_path = format!("{}/key.pem", domain_ssl_dir);

                info!("Generating dummy SSL certificate for Nginx testing...");
                
                // Generate self-signed cert pakai openssl command
                let output = Command::new("openssl")
                    .args(&[
                        "req", "-x509", "-newkey", "rsa:2048", "-keyout", &key_path,
                        "-out", &cert_path, "-days", "365", "-nodes",
                        "-subj", &format!("/CN={}", d.domain_name)
                    ])
                    .output();

                match output {
                    Ok(out) if out.status.success() => {
                        info!("✅ SSL Certificate successfully generated at {}", cert_path);
                        
                        // Update database
                        sqlx::query!(
                            "UPDATE domains SET status = 'active', ssl_expires_at = NOW() + INTERVAL '90 days' WHERE id = $1",
                            event.domain_id
                        )
                        .execute(&self.db)
                        .await?;
                        
                        info!("Database updated: SSL expires in 90 days");
                    }
                    _ => {
                        error!("Failed to generate SSL certificate");
                    }
                }
            } else {
                info!("Skipping Let's Encrypt: ssl_enabled={}, provider={:?}", d.ssl_enabled, d.ssl_provider);
            }
        }

        Ok(())
    }

    async fn run(&self) -> Result<()> {
        info!("SSL Daemon started, listening for domain.created events...");

        let mut sub_created = self.nats.subscribe("domain.created").await?;

        while let Some(msg) = sub_created.next().await {
            match serde_json::from_slice::<DomainCreated>(&msg.payload) {
                Ok(event) => {
                    if let Err(e) = self.handle_domain_created(event).await {
                        error!("Error handling domain.created: {}", e);
                    }
                }
                Err(e) => error!("Failed to parse domain.created event: {}", e),
            }
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("ssl_daemon=info,tower_http=info")
        .init();

    info!("Starting SSL Daemon...");

    let daemon = SslDaemon::new().await?;
    daemon.run().await?;

    Ok(())
}
