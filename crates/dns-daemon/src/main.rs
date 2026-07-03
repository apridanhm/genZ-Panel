use futures::StreamExt;
use anyhow::Result;
use async_nats::Client;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::env;
use tracing::{info, error};
use uuid::Uuid;

mod pdns_client;
use pdns_client::PdnsClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DomainCreated {
    domain_id: Uuid,
    domain_name: String,
    user_id: Uuid,
    ssl_enabled: bool,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DomainDeleted {
    domain_id: Uuid,
    domain_name: String,
    user_id: Uuid,
}

#[derive(Clone)]
struct DnsDaemon {
    nats: Client,
    pdns: PdnsClient,
    db: PgPool,
    server_ip: String,
    default_zone: String,
    nameserver: String,
}

impl DnsDaemon {
    async fn new() -> Result<Self> {
        let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
        let pdns_url = env::var("PDNS_URL").unwrap_or_else(|_| "http://localhost:8081".to_string());
        let pdns_api_key = env::var("PDNS_API_KEY").unwrap_or_else(|_| "supersecretapikey123".to_string());
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let server_ip = env::var("SERVER_IP").unwrap_or_else(|_| "10.4.60.239".to_string());
        let default_zone = env::var("DEFAULT_ZONE").unwrap_or_else(|_| "genzpanel.local.".to_string());
        let nameserver = env::var("NAMESERVER").unwrap_or_else(|_| "ns1.genzpanel.local.".to_string());

        info!("Connecting to NATS at {}", nats_url);
        let nats = async_nats::connect(&nats_url).await?;
        info!("Connected to NATS");

        info!("Connecting to PowerDNS at {}", pdns_url);
        let pdns = PdnsClient::new(&pdns_url, &pdns_api_key);
        info!("Connected to PowerDNS");

        info!("Connecting to database");
        let db = PgPool::connect(&database_url).await?;
        info!("Connected to database");

        Ok(Self {
            nats,
            pdns,
            db,
            server_ip,
            default_zone,
            nameserver,
        })
    }

    async fn handle_domain_created(&self, event: DomainCreated) -> Result<()> {
        info!("Handling domain.created event for: {}", event.domain_name);

        // Extract zone from domain name
        // For simplicity, we assume all domains are subdomains of default_zone
        // Example: myapp.genzpanel.local -> zone: genzpanel.local.
        let zone_name = &self.default_zone;
        let record_name = &event.domain_name;

        // Check if zone exists, create if not
        let zones = self.pdns.list_zones().await?;
        let zone_exists = zones.iter().any(|z| z.name == *zone_name);
        
        if !zone_exists {
            info!("Zone {} does not exist, creating...", zone_name);
            self.pdns.create_zone(zone_name, &self.nameserver).await?;
        }

        // Create A record
        self.pdns.create_a_record(zone_name, record_name, &self.server_ip).await?;

        // Update database status
        sqlx::query("UPDATE domains SET status = 'active' WHERE id = $1")
            .bind(event.domain_id)
            .execute(&self.db)
            .await?;

        info!("DNS record created for {}", event.domain_name);
        Ok(())
    }

    async fn handle_domain_deleted(&self, event: DomainDeleted) -> Result<()> {
        info!("Handling domain.deleted event for: {}", event.domain_name);

        let zone_name = &self.default_zone;
        let record_name = &event.domain_name;

        // Delete A record
        if let Err(e) = self.pdns.delete_a_record(zone_name, record_name).await {
            error!("Failed to delete DNS record for {}: {}", event.domain_name, e);
        }

        info!("DNS record deleted for {}", event.domain_name);
        Ok(())
    }

    async fn run(&self) -> Result<()> {
        info!("DNS Daemon started, listening for events...");

        let mut sub_created = self.nats.subscribe("domain.created").await?;
        let mut sub_deleted = self.nats.subscribe("domain.deleted").await?;

        loop {
            tokio::select! {
                Some(msg) = sub_created.next() => {
                    match serde_json::from_slice::<DomainCreated>(&msg.payload) {
                        Ok(event) => {
                            if let Err(e) = self.handle_domain_created(event).await {
                                error!("Error handling domain.created: {}", e);
                            }
                        }
                        Err(e) => error!("Failed to parse domain.created event: {}", e),
                    }
                }
                Some(msg) = sub_deleted.next() => {
                    match serde_json::from_slice::<DomainDeleted>(&msg.payload) {
                        Ok(event) => {
                            if let Err(e) = self.handle_domain_deleted(event).await {
                                error!("Error handling domain.deleted: {}", e);
                            }
                        }
                        Err(e) => error!("Failed to parse domain.deleted event: {}", e),
                    }
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("dns_daemon=info,tower_http=info")
        .init();

    info!("Starting DNS Daemon...");

    let daemon = DnsDaemon::new().await?;
    daemon.run().await?;

    Ok(())
}
