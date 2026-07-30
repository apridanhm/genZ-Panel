use anyhow::Result;
use async_nats::Client;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use tracing::{info, error, warn};
use uuid::Uuid;
use futures::StreamExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppDeployTriggered {
    app_id: Uuid,
    domain_id: Uuid,
    user_id: Uuid,
    name: String,
    runtime: String,
    runtime_version: Option<String>,
    source_type: String,
    git_repo_url: Option<String>,
    git_branch: Option<String>,
    zip_file_path: Option<String>,
    build_command: Option<String>,
    start_command: String,
    exposed_port: i32,
}

#[derive(Clone)]
struct BuilderDaemon {
    nats: Client,
    db: PgPool,
    apps_base_dir: String,
}

impl BuilderDaemon {
    async fn new() -> Result<Self> {
        let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let apps_base_dir = env::var("APPS_BASE_DIR").unwrap_or_else(|_| "/home/genZ-panel/apps/data".to_string());

        info!("Connecting to NATS at {}", nats_url);
        let nats = async_nats::connect(&nats_url).await?;
        info!("Connected to NATS");

        info!("Connecting to database");
        let db = PgPool::connect(&database_url).await?;
        info!("Connected to database");

        fs::create_dir_all(&apps_base_dir)?;
        info!("Apps base directory ready at {}", apps_base_dir);

        Ok(Self { nats, db, apps_base_dir })
    }

    async fn handle_deploy(&self, event: AppDeployTriggered) -> Result<()> {
        info!("🚀 Starting deployment for app: {} ({})", event.name, event.runtime);

        // Update status ke building
        sqlx::query!("UPDATE applications SET status = 'building' WHERE id = $1", event.app_id)
            .execute(&self.db)
            .await?;

        let app_dir = format!("{}/{}", self.apps_base_dir, event.app_id);
        let source_dir = format!("{}/source", app_dir);

        // 1. Clone Git Repo (kalau source_type = git)
        if event.source_type == "git" {
            if let Some(repo_url) = &event.git_repo_url {
                info!("📥 Cloning repository: {} (branch: {})", repo_url, event.git_branch.as_deref().unwrap_or("main"));
                
                // Hapus folder lama kalau ada
                let _ = fs::remove_dir_all(&source_dir);
                fs::create_dir_all(&source_dir)?;

                let branch = event.git_branch.as_deref().unwrap_or("main");
                let output = Command::new("git")
                    .args(&["clone", "--depth", "1", "--branch", branch, repo_url, &source_dir])
                    .output()?;

                if !output.status.success() {
                    error!("❌ Git clone failed: {}", String::from_utf8_lossy(&output.stderr));
                    sqlx::query!("UPDATE applications SET status = 'failed' WHERE id = $1", event.app_id)
                        .execute(&self.db)
                        .await?;
                    return Err(anyhow::anyhow!("Git clone failed"));
                }
                info!("✅ Repository cloned successfully");
            }
        }

        // 2. Generate Dockerfile kalau belum ada
        let dockerfile_path = format!("{}/Dockerfile", source_dir);
        if !Path::new(&dockerfile_path).exists() {
            info!("📝 Generating Dockerfile for runtime: {}", event.runtime);
            let dockerfile_content = self.generate_dockerfile(&event);
            fs::write(&dockerfile_path, dockerfile_content)?;
            info!("✅ Dockerfile generated");
        } else {
            info!("✅ Using existing Dockerfile from repository");
        }

        // TODO: Step selanjutnya - Docker Build & Run (akan kita tambahkan setelah ini berhasil)
        info!("⏳ [NEXT STEP] Docker build & container deployment...");
        
        // Untuk sekarang, set status ke running (placeholder)
        sqlx::query!("UPDATE applications SET status = 'running' WHERE id = $1", event.app_id)
            .execute(&self.db)
            .await?;

        info!("✅ App {} deployment simulation complete", event.name);
        Ok(())
    }

    fn generate_dockerfile(&self, event: &AppDeployTriggered) -> String {
        match event.runtime.as_str() {
            "node" => format!(
                r#"FROM node:{}-alpine
WORKDIR /app
COPY package*.json ./
RUN npm install
COPY . .
EXPOSE {}
CMD ["{}"]
"#,
                event.runtime_version.as_deref().unwrap_or("18"),
                event.exposed_port,
                event.start_command
            ),
            "php" => format!(
                r#"FROM php:{}-apache
WORKDIR /var/www/html
COPY . .
RUN a2enmod rewrite
EXPOSE 80
CMD ["apache2-foreground"]
"#,
                event.runtime_version.as_deref().unwrap_or("8.3")
            ),
            "go" => format!(
                r#"FROM golang:{}-alpine AS builder
WORKDIR /app
COPY go.* ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 GOOS=linux go build -o /main .

FROM alpine:latest
COPY --from=builder /main /main
EXPOSE {}
CMD ["/main"]
"#,
                event.runtime_version.as_deref().unwrap_or("1.21"),
                event.exposed_port
            ),
            _ => {
                warn!("Unknown runtime {}, using basic alpine", event.runtime);
                format!(
                    r#"FROM alpine:latest
WORKDIR /app
COPY . .
EXPOSE {}
CMD ["sh"]
"#,
                    event.exposed_port
                )
            }
        }
    }

    async fn run(&self) -> Result<()> {
        info!("Builder Daemon started, listening for app.deploy.triggered events...");

        let mut sub = self.nats.subscribe("app.deploy.triggered").await?;

        while let Some(msg) = sub.next().await {
            match serde_json::from_slice::<AppDeployTriggered>(&msg.payload) {
                Ok(event) => {
                    if let Err(e) = self.handle_deploy(event).await {
                        error!("Error handling deployment: {}", e);
                    }
                }
                Err(e) => error!("Failed to parse app.deploy.triggered event: {}", e),
            }
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("builder_daemon=info")
        .init();

    info!("Starting Builder Daemon...");
    let daemon = BuilderDaemon::new().await?;
    daemon.run().await?;

    Ok(())
}
