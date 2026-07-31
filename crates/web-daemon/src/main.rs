use anyhow::Result;
use async_nats::Client;
use bollard::container::KillContainerOptions;
use bollard::Docker;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::env;
use std::fs;
use std::process::Command;
use tracing::{info, error, warn};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppDeployed {
    app_id: Uuid,
    domain_id: Uuid,
    container_name: String,
    exposed_port: i32,
}

#[derive(Debug, FromRow)]
struct DomainInfo {
    id: Uuid,
    domain_name: String,
    ssl_enabled: bool,
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
        let conf_dir = env::var("NGINX_CONF_DIR").unwrap_or_else(|_| "/home/genZ-panel/apps/nginx/conf.d".to_string());
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

        Ok(Self { nats, db, docker, nginx_container_name, conf_dir, core_api_url, ssl_base_dir })
    }

    // 🛡️ AUTO SSL GENERATOR (Self-Signed untuk Testing Tanpa DNS)
    async fn ensure_ssl(&self, domain_name: &str, domain_id: &str) -> Result<()> {
        info!(" Generating Self-Signed SSL certificate for {}...", domain_name);
        
        let cert_dir = format!("{}/{}", self.ssl_base_dir, domain_id);
        let cert_path = format!("{}/cert.pem", cert_dir);
        let key_path = format!("{}/key.pem", cert_dir);
        
        fs::create_dir_all(&cert_dir)?;

        // Generate sertifikat pakai openssl (tidak butuh internet/DNS)
        let output = Command::new("openssl")
            .args(&[
                "req", "-x509", "-newkey", "rsa:2048",
                "-keyout", &key_path,
                "-out", &cert_path,
                "-days", "365", "-nodes",
                "-subj", &format!("/CN={}", domain_name),
            ])
            .output()?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            error!("❌ openssl failed for {}: {}", domain_name, err_msg);
            return Err(anyhow::anyhow!("openssl failed: {}", err_msg));
        }

        info!("🎉 Self-Signed SSL Certificate generated for {}", domain_name);
        Ok(())
    }

    async fn handle_domain_created(&self, event: DomainCreated) -> Result<()> {
        info!("Handling domain.created: {}", event.domain_name);
        
        let domain = sqlx::query_as::<_, DomainInfo>(
            "SELECT id, domain_name, ssl_enabled FROM domains WHERE id = $1"
        ).bind(event.domain_id).fetch_optional(&self.db).await?;

        if let Some(d) = domain {
            let conf_file = format!("{}/{}.conf", self.conf_dir, d.id);
            
            // Jika SSL diminta, generate sertifikat dan tulis config HTTPS
            if d.ssl_enabled {
                if let Err(e) = self.ensure_ssl(&d.domain_name, &d.id.to_string()).await {
                    warn!("SSL generation failed for {}, falling back to HTTP: {}", d.domain_name, e);
                    let http_config = self.generate_http_nginx_config(&d);
                    fs::write(&conf_file, &http_config)?;
                } else {
                    let https_config = self.generate_https_nginx_config(&d);
                    fs::write(&conf_file, &https_config)?;
                }
            } else {
                let http_config = self.generate_http_nginx_config(&d);
                fs::write(&conf_file, &http_config)?;
            }

            self.reload_nginx().await?;
            
            sqlx::query!("UPDATE domains SET status = 'active' WHERE id = $1", event.domain_id).execute(&self.db).await?;
            info!("✅ Domain {} is now ACTIVE", d.domain_name);
        }
        Ok(())
    }

    async fn handle_app_deployed(&self, event: AppDeployed) -> Result<()> {
        info!(" Handling app.deployed for dynamic routing: {}", event.container_name);
        
        let domain = sqlx::query_as::<_, DomainInfo>(
            "SELECT id, domain_name, ssl_enabled FROM domains WHERE id = $1"
        ).bind(event.domain_id).fetch_optional(&self.db).await?;

        if let Some(d) = domain {
            let conf_file = format!("{}/{}.conf", self.conf_dir, d.id);
            let config = self.generate_app_nginx_config(&d, &event);
            fs::write(&conf_file, &config)?;
            self.reload_nginx().await?;
            info!("🎉 Dynamic routing for {} is now ACTIVE! Pointing to {}", d.domain_name, event.container_name);
        }
        Ok(())
    }

    fn generate_http_nginx_config(&self, domain: &DomainInfo) -> String {
        format!(
            r#"server {{
    listen 80;
    server_name {};
    client_max_body_size 50M;

    location / {{
        proxy_pass {};
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }}
}}
"#,
            domain.domain_name, self.core_api_url
        )
    }

    fn generate_https_nginx_config(&self, domain: &DomainInfo) -> String {
        let cert = format!("{}/{}/cert.pem", self.ssl_base_dir, domain.id);
        let key = format!("{}/{}/key.pem", self.ssl_base_dir, domain.id);
        
        format!(
            r#"# HTTP to HTTPS Redirect
server {{
    listen 80;
    server_name {};
    return 301 https://$host$request_uri;
}}

# HTTPS Server
server {{
    listen 443 ssl http2;
    server_name {};
    client_max_body_size 50M;

    ssl_certificate {};
    ssl_certificate_key {};
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    location / {{
        proxy_pass {};
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }}
}}
"#,
            domain.domain_name, domain.domain_name, cert, key, self.core_api_url
        )
    }

    fn generate_app_nginx_config(&self, domain: &DomainInfo, app: &AppDeployed) -> String {
        let upstream_url = format!("http://{}:{}", app.container_name, app.exposed_port);
        
        if domain.ssl_enabled {
            let cert = format!("{}/{}/cert.pem", self.ssl_base_dir, domain.id);
            let key = format!("{}/{}/key.pem", self.ssl_base_dir, domain.id);
            format!(
                r#"server {{
    listen 80;
    server_name {};
    return 301 https://$host$request_uri;
}}

server {{
    listen 443 ssl http2;
    server_name {};
    client_max_body_size 50M;

    ssl_certificate {};
    ssl_certificate_key {};
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    location / {{
        proxy_pass {};
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }}
}}
"#,
                domain.domain_name, domain.domain_name, cert, key, upstream_url
            )
        } else {
            format!(
                r#"server {{
    listen 80;
    server_name {};
    client_max_body_size 50M;

    location / {{
        proxy_pass {};
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }}
}}
"#,
                domain.domain_name, upstream_url
            )
        }
    }

    async fn reload_nginx(&self) -> Result<()> {
        info!("Sending SIGHUP to {} to reload configuration...", self.nginx_container_name);
        self.docker.kill_container(&self.nginx_container_name, Some(KillContainerOptions { signal: "SIGHUP" })).await?;
        info!("✅ Nginx reloaded successfully");
        Ok(())
    }

    async fn run(&self) -> Result<()> {
        info!("Web Daemon started, listening for events...");
        
        let daemon_clone_1 = self.clone();
        let mut sub_created = self.nats.subscribe("domain.created").await?;
        tokio::spawn(async move {
            while let Some(msg) = sub_created.next().await {
                if let Ok(event) = serde_json::from_slice::<DomainCreated>(&msg.payload) {
                    if let Err(e) = daemon_clone_1.handle_domain_created(event).await {
                        error!("Error handling domain.created: {}", e);
                    }
                }
            }
        });

        let daemon_clone_2 = self.clone();
        let mut sub_deployed = self.nats.subscribe("app.deployed").await?;
        tokio::spawn(async move {
            while let Some(msg) = sub_deployed.next().await {
                if let Ok(event) = serde_json::from_slice::<AppDeployed>(&msg.payload) {
                    if let Err(e) = daemon_clone_2.handle_app_deployed(event).await {
                        error!("Error handling app.deployed: {}", e);
                    }
                }
            }
        });

        tokio::signal::ctrl_c().await?;
        info!("Web Daemon shutting down...");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("web_daemon=info").init();
    info!("Starting Web Daemon...");
    let daemon = WebDaemon::new().await?;
    daemon.run().await?;
    Ok(())
}
