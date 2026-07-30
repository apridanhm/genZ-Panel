use async_nats::Client;
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainCreated {
    pub domain_id: Uuid,
    pub domain_name: String,
    pub user_id: Uuid,
    pub ssl_enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainUpdated {
    pub domain_id: Uuid,
    pub domain_name: String,
    pub user_id: Uuid,
    pub status: String,
    pub ssl_enabled: bool,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainDeleted {
    pub domain_id: Uuid,
    pub domain_name: String,
    pub user_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppDeployTriggered {
    pub app_id: Uuid,
    pub domain_id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub runtime: String,
    pub source_type: String,
    pub git_repo_url: Option<String>,
    pub git_branch: Option<String>,
    pub zip_file_path: Option<String>,
    pub build_command: Option<String>,
    pub start_command: String,
    pub exposed_port: i32,
}

#[derive(Clone)]
pub struct EventPublisher {
    pub client: Client,
}

impl EventPublisher {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn publish_domain_created(&self, event: DomainCreated) -> Result<(), anyhow::Error> {
        let payload = serde_json::to_string(&event)?;
        info!("Publishing event to domain.created: {}", payload);
        self.client.publish("domain.created", payload.into()).await?;
        Ok(())
    }

    pub async fn publish_domain_updated(&self, event: DomainUpdated) -> Result<(), anyhow::Error> {
        let payload = serde_json::to_string(&event)?;
        info!("Publishing event to domain.updated: {}", payload);
        self.client.publish("domain.updated", payload.into()).await?;
        Ok(())
    }

    pub async fn publish_domain_deleted(&self, event: DomainDeleted) -> Result<(), anyhow::Error> {
        let payload = serde_json::to_string(&event)?;
        info!("Publishing event to domain.deleted: {}", payload);
        self.client.publish("domain.deleted", payload.into()).await?;
        Ok(())
    }

    pub async fn publish_app_deploy_triggered(&self, event: AppDeployTriggered) -> Result<(), anyhow::Error> {
        let payload = serde_json::to_string(&event)?;
        info!("Publishing event to app.deploy.triggered: {}", payload);
        self.client.publish("app.deploy.triggered", payload.into()).await?;
        Ok(())
    }
}
