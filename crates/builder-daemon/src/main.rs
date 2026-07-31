use anyhow::Result;
use async_nats::Client;
use bollard::container::{Config, CreateContainerOptions, StartContainerOptions};
use bollard::models::{HostConfig, PortBinding};
use bollard::network::{ConnectNetworkOptions, CreateNetworkOptions};
use bollard::Docker;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppDeployed {
    app_id: Uuid,
    domain_id: Uuid,
    container_name: String,
    exposed_port: i32,
}

#[derive(Clone)]
struct BuilderDaemon {
    nats: Client,
    db: PgPool,
    docker: Docker,
    apps_base_dir: String,
    network_name: String,
}

impl BuilderDaemon {
    async fn new() -> Result<Self> {
        let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let apps_base_dir = env::var("APPS_BASE_DIR").unwrap_or_else(|_| "/home/genZ-panel/apps/data".to_string());
        let network_name = env::var("PANEL_NETWORK_NAME").unwrap_or_else(|_| "panel-network".to_string());

        info!("Connecting to NATS at {}", nats_url);
        let nats = async_nats::connect(&nats_url).await?;
        info!("Connected to NATS");

        info!("Connecting to database");
        let db = PgPool::connect(&database_url).await?;
        info!("Connected to database");

        info!("Connecting to Docker");
        let docker = Docker::connect_with_local_defaults()?;
        docker.ping().await?;
        info!("Connected to Docker");

        fs::create_dir_all(&apps_base_dir)?;
        info!("Apps base directory ready at {}", apps_base_dir);

        info!("🛡️ Ensuring Docker network '{}' exists...", network_name);
        match docker.inspect_network::<String>(&network_name, None).await {
            Ok(_) => info!("✅ Network '{}' already exists.", network_name),
            Err(_) => {
                info!("⚠️ Network '{}' not found. Creating it automatically...", network_name);
                docker.create_network(CreateNetworkOptions {
                    name: network_name.clone(),
                    check_duplicate: true,
                    ..Default::default()
                }).await?;
                info!("✅ Network '{}' created successfully!", network_name);
            }
        }

        let nginx_container = "panel-nginx";
        match docker.inspect_container(nginx_container, None).await {
            Ok(container_info) => {
                let mut is_connected = false;
                if let Some(network_settings) = container_info.network_settings {
                    if let Some(networks) = network_settings.networks {
                        if networks.contains_key(&network_name) {
                            is_connected = true;
                        }
                    }
                }

                if !is_connected {
                    info!("🔗 Connecting '{}' to network '{}'...", nginx_container, network_name);
                    docker.connect_network(&network_name, ConnectNetworkOptions {
                        container: nginx_container.to_string(),
                        ..Default::default()
                    }).await?;
                    info!("✅ '{}' successfully connected to '{}'!", nginx_container, network_name);
                } else {
                    info!("✅ '{}' is already connected to '{}'.", nginx_container, network_name);
                }
            }
            Err(_) => warn!("⚠️ Container '{}' not found. Make sure it's running.", nginx_container),
        }

        Ok(Self { nats, db, docker, apps_base_dir, network_name })
    }

    async fn handle_deploy(&self, event: AppDeployTriggered) -> Result<()> {
        info!("🚀 Starting deployment for app: {} ({})", event.name, event.runtime);

        let app_id_str = event.app_id.to_string();
        let image_name = format!("genzpanel-app-{}", app_id_str);
        let container_name = format!("app-{}", app_id_str);
        let app_dir = format!("{}/{}", self.apps_base_dir, app_id_str);
        let source_dir = format!("{}/source", app_dir);

        sqlx::query!("UPDATE applications SET status = 'building' WHERE id = $1", event.app_id)
            .execute(&self.db)
            .await?;

        if event.source_type == "git" {
            if let Some(repo_url) = &event.git_repo_url {
                info!("📥 Cloning repository: {} (branch: {})", repo_url, event.git_branch.as_deref().unwrap_or("main"));
                let _ = fs::remove_dir_all(&source_dir);
                fs::create_dir_all(&source_dir)?;

                let branch = event.git_branch.as_deref().unwrap_or("main");
                let output = Command::new("git")
                    .args(&["clone", "--depth", "1", "--branch", branch, repo_url, &source_dir])
                    .output()?;

                if !output.status.success() {
                    error!("❌ Git clone failed: {}", String::from_utf8_lossy(&output.stderr));
                    self.update_status(event.app_id, "failed").await?;
                    return Err(anyhow::anyhow!("Git clone failed"));
                }
                info!("✅ Repository cloned successfully");
            }
        }

        let dockerfile_path = format!("{}/Dockerfile", source_dir);
        if !Path::new(&dockerfile_path).exists() {
            info!("📝 Generating Dockerfile for runtime: {}", event.runtime);
            let dockerfile_content = self.generate_dockerfile(&event);
            fs::write(&dockerfile_path, dockerfile_content)?;
            info!("✅ Dockerfile generated");
        }

        info!("🔨 Building Docker image: {}", image_name);
        let build_output = Command::new("docker")
            .args(&["build", "-t", &image_name, "."])
            .current_dir(&source_dir)
            .output()?;

        if !build_output.status.success() {
            error!("❌ Docker build failed: {}", String::from_utf8_lossy(&build_output.stderr));
            self.update_status(event.app_id, "failed").await?;
            return Err(anyhow::anyhow!("Docker build failed"));
        }
        info!("✅ Docker image built successfully");

        info!("🏃 Starting container: {} on port {}", container_name, event.exposed_port);
        let _ = self.docker.remove_container(&container_name, None).await;

        // 🛡️ HORMATI INPUT USER: Gunakan port yang diminta user untuk host binding
        let container_port = format!("{}/tcp", event.exposed_port);
        let host_port_str = event.exposed_port.to_string();
        
        let mut port_bindings = HashMap::new();
        port_bindings.insert(container_port.clone(), Some(vec![PortBinding {
            host_ip: Some("0.0.0.0".to_string()),
            host_port: Some(host_port_str.clone()), // <-- Menggunakan port pilihan user
        }]));

        let mut exposed_ports = HashMap::new();
        exposed_ports.insert(container_port, HashMap::new());

        let memory_limit: i64 = 512 * 1024 * 1024;
        let cpu_quota: i64 = 50000;

        let config = Config {
            image: Some(image_name.clone()),
            cmd: Some(vec!["sh".to_string(), "-c".to_string(), event.start_command.clone()]),
            exposed_ports: Some(exposed_ports),
            host_config: Some(HostConfig {
                port_bindings: Some(port_bindings),
                memory: Some(memory_limit),
                cpu_quota: Some(cpu_quota),
                network_mode: Some(self.network_name.clone()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let container = match self.docker.create_container(
            Some(CreateContainerOptions {
                name: container_name.clone(),
                platform: None,
            }),
            config,
        ).await {
            Ok(c) => c,
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("port is already allocated") {
                    error!("❌ Port {} sudah digunakan oleh aplikasi lain di host ini.", event.exposed_port);
                    self.update_status(event.app_id, "failed").await?;
                    return Err(anyhow::anyhow!("Port {} is already allocated on the host. Please choose a different exposed_port.", event.exposed_port));
                }
                error!("❌ Failed to create container: {}", e);
                self.update_status(event.app_id, "failed").await?;
                return Err(anyhow::anyhow!("Failed to create container: {}", e));
            }
        };

        self.docker
            .start_container(&container.id, None::<StartContainerOptions<String>>)
            .await?;

        info!("✅ Container {} started successfully (ID: {})", container_name, container.id);

        sqlx::query!(
            "UPDATE applications SET status = 'running', container_id = $1 WHERE id = $2",
            container.id,
            event.app_id
        )
        .execute(&self.db)
        .await?;

        // 📡 PUBLISH EVENT: Beri tahu Web Daemon untuk update Nginx!
        let deployed_event = AppDeployed {
            app_id: event.app_id,
            domain_id: event.domain_id,
            container_name: container_name.clone(),
            exposed_port: event.exposed_port,
        };
        let payload = serde_json::to_string(&deployed_event).unwrap_or_default();
        info!("📡 Publishing event to app.deployed: {}", payload);
        let _ = self.nats.publish("app.deployed", payload.into()).await;

        info!("🎉 App {} deployment COMPLETE! Container is running.", event.name);
        Ok(())
    }

    async fn update_status(&self, app_id: Uuid, status: &str) -> Result<()> {
        sqlx::query!("UPDATE applications SET status = $1 WHERE id = $2", status, app_id)
            .execute(&self.db)
            .await?;
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
CMD {}
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
"#,
                event.runtime_version.as_deref().unwrap_or("8.3")
            ),
            "go" => format!(
                r#"FROM golang:{}-alpine AS builder
WORKDIR /app
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
            _ => format!(
                r#"FROM alpine:latest
WORKDIR /app
COPY . .
EXPOSE {}
CMD ["sh"]
"#,
                event.exposed_port
            ),
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
