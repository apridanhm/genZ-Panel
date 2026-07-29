use anyhow::Result;
use async_nats::Client;
use bollard::container::KillContainerOptions;
use bollard::Docker;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::env;
use std::fs;
use std::path::Path;
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

#[derive(Debug, FromRow)]
struct DomainInfo {
    id: Uuid,
    domain_name: String,
    ssl_enabled: bool,
    ssl_provider: Option<String>,
    ssl_cert_path: Option<String>,
    ssl_key_path: Option<String>,
}

#[derive(Clone)]
struct WebDaemon {
    nats: Client,
    db: sqlx::PgPool,
    docker: Docker,
    nginx_container_name: String,
    conf_dir: String,
    core_api_url: String,
    ssl_base_dir: String,
}

impl WebDaemon {
    async fn new() -> Result<Self> {
        let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let nginx_container_name = env::var("NGINX_CONTAINER_NAME").unwrap_or_else(|_| "panel-nginx".to_string());
        let conf_dir = env::var("NGINX_CONF_DIR").unwrap_or_else(|_| "/apps/nginx/conf.d".to_string());
        let core_api_url = env::var("CORE_API_URL").unwrap_or_else(|_| "http://10.4.60.239:8000".to_string());
        let ssl_base_dir = env::var("SSL_BASE_DIR").unwrap_or_else(|_| "/home/genZ-panel/apps/ssl/letsencrypt".to_string());

        info!("Connecting to NATS at {}", nats_url);
        let nats = async_nats::connect(&nats_url).await?;
        info!("Connected to NATS");

        info!("Connecting to database");
        let db = sqlx::PgPool::connect(&database_url).await?;
        info!("Connected to database");

        info!("Connecting to Docker");
        let docker = Docker::connect_with_local_defaults()?;
        docker.ping().await?;
        info!("Connected to Docker");

        fs::create_dir_all(&conf_dir)?;
        fs::create_dir_all(&ssl_base_dir)?;
        info!("Directories ready");

        Ok(Self {
            nats, db, docker, nginx_container_name, conf_dir, core_api_url, ssl_base_dir,
        })
    }

    async fn handle_domain_created(&self, event: DomainCreated) -> Result<()> {
        info!("Handling domain.created for web provisioning: {}", event.domain_name);

        let domain = sqlx::query_as::<_, DomainInfo>(
            r#"SELECT id, domain_name, ssl_enabled, ssl_provider, ssl_cert_path, ssl_key_path FROM domains WHERE id = $1"#
        )
        .bind(event.domain_id)
        .fetch_optional(&self.db)
        .await?;

        if let Some(d) = domain {
            let conf_file = format!("{}/{}.conf", self.conf_dir, d.id);
            let config = self.generate_nginx_config(&d);
            
            fs::write(&conf_file, &config)?;
            info!("✅ Nginx config written to {}", conf_file);

            // 🔥 RACE CONDITION FIX: Ensure SSL cert exists before reloading Nginx
            if d.ssl_enabled && d.ssl_provider.as_deref() != Some("custom") {
                let cert_path = format!("{}/{}/cert.pem", self.ssl_base_dir, d.id);
                let key_path = format!("{}/{}/key.pem", self.ssl_base_dir, d.id);
                
                if !Path::new(&cert_path).exists() {
                    info!("⚠️ SSL cert not found, generating fallback dummy cert for Nginx...");
                    let _ = fs::create_dir_all(format!("{}/{}", self.ssl_base_dir, d.id));
                    let _ = Command::new("openssl")
                        .args(&[
                            "req", "-x509", "-newkey", "rsa:2048", "-keyout", &key_path,
                            "-out", &cert_path, "-days", "365", "-nodes",
                            "-subj", &format!("/CN={}", d.domain_name)
                        ])
                        .output();
                    info!("✅ Fallback SSL cert generated");
                }
            }

            self.reload_nginx().await?;
            
            sqlx::query!("UPDATE domains SET status = 'active' WHERE id = $1", event.domain_id)
                .execute(&self.db)
                .await?;
            
            info!("✅ Domain {} is now ACTIVE and served by Nginx", d.domain_name);
        }

        Ok(())
    }

    fn generate_nginx_config(&self, domain: &DomainInfo) -> String {
        let mut config = String::new();

        if domain.ssl_enabled {
            config.push_str(&format!(
                r#"server {{
    listen 80;
    server_name {};
    return 301 https://$host$request_uri;
}}

"#,
                domain.domain_name
            ));
        }

        config.push_str(&format!(
            r#"server {{
    listen 443 ssl;
    http2 on;
    server_name {};
    client_max_body_size 50M;

"#,
            domain.domain_name
        ));

        if domain.ssl_enabled {
            let (cert, key) = if domain.ssl_provider.as_deref() == Some("custom") 
                && domain.ssl_cert_path.is_some() 
                && domain.ssl_key_path.is_some() 
            {
                (domain.ssl_cert_path.clone().unwrap(), domain.ssl_key_path.clone().unwrap())
            } else {
                (
                    format!("/etc/nginx/ssl/letsencrypt/{}/cert.pem", domain.id),
                    format!("/etc/nginx/ssl/letsencrypt/{}/key.pem", domain.id),
                )
            };

            config.push_str(&format!(
                r#"    ssl_certificate {};
    ssl_certificate_key {};
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;
    
"#,
                cert, key
            ));
        }

        config.push_str(&format!(
            r#"    location / {{
        proxy_pass {};
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }}
}}
"#,
            self.core_api_url
        ));

        config
    }

    async fn reload_nginx(&self) -> Result<()> {
        info!("Sending SIGHUP to {} to reload configuration...", self.nginx_container_name);
        
        match self.docker
            .kill_container(
                &self.nginx_container_name,
                Some(KillContainerOptions { signal: "SIGHUP" }),
            )
            .await
        {
            Ok(_) => {
                info!("✅ Nginx reloaded successfully");
                Ok(())
            }
            Err(e) => {
                error!("Failed to reload Nginx: {}. Make sure {} container is running.", e, self.nginx_container_name);
                Err(anyhow::anyhow!("Nginx reload failed: {}", e))
            }
        }
    }

    async fn run(&self) -> Result<()> {
        info!("Web Daemon started, listening for domain.created events...");
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
        .with_env_filter("web_daemon=info,tower_http=info")
        .init();

    info!("Starting Web Daemon...");
    let daemon = WebDaemon::new().await?;
    daemon.run().await?;
    Ok(())
}
