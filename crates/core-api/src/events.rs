use async_nats::Client;
use serde::{Deserialize, Serialize};
use tracing::{error, info};
use uuid::Uuid;
use chrono::{DateTime, Utc};

// --- Event Payloads ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainCreated {
    pub domain_id: Uuid,
    pub domain_name: String,
    pub user_id: Uuid,
    pub ssl_enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainUpdated {
    pub domain_id: Uuid,
    pub domain_name: String,
    pub user_id: Uuid,
    pub status: String,
    pub ssl_enabled: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainDeleted {
    pub domain_id: Uuid,
    pub domain_name: String,
    pub user_id: Uuid,
}

// --- Event Publisher ---

#[derive(Clone)]
pub struct EventPublisher {
    client: Client,
}

impl EventPublisher {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn publish_domain_created(&self, event: DomainCreated) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let subject = "domain.created";
        let payload = serde_json::to_string(&event)?;
        info!("Publishing event to {}: {}", subject, payload);
        self.client.publish(subject.to_string(), payload.into()).await?;
        Ok(())
    }

    pub async fn publish_domain_updated(&self, event: DomainUpdated) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let subject = "domain.updated";
        let payload = serde_json::to_string(&event)?;
        info!("Publishing event to {}: {}", subject, payload);
        self.client.publish(subject.to_string(), payload.into()).await?;
        Ok(())
    }

    pub async fn publish_domain_deleted(&self, event: DomainDeleted) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let subject = "domain.deleted";
        let payload = serde_json::to_string(&event)?;
        info!("Publishing event to {}: {}", subject, payload);
        self.client.publish(subject.to_string(), payload.into()).await?;
        Ok(())
    }
}
