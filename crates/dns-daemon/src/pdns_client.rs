use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, error};

#[derive(Clone)]
pub struct PdnsClient {
    client: Client,
    base_url: String,
    api_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Zone {
    pub id: String,
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Serialize)]
pub struct CreateZoneRequest {
    pub name: String,
    pub kind: String,
    pub nameservers: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct RrSet {
    pub name: String,
    pub r#type: String,
    pub ttl: u32,
    pub changetype: String,
    pub records: Vec<Record>,
}

#[derive(Debug, Serialize)]
pub struct Record {
    pub content: String,
    pub disabled: bool,
}

#[derive(Debug, Serialize)]
pub struct PatchZoneRequest {
    pub rrsets: Vec<RrSet>,
}

impl PdnsClient {
    pub fn new(base_url: &str, api_key: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
        }
    }

    pub async fn list_zones(&self) -> Result<Vec<Zone>> {
        let url = format!("{}/api/v1/servers/localhost/zones", self.base_url);
        let response = self.client
            .get(&url)
            .header("X-API-Key", &self.api_key)
            .send()
            .await?;
        
        let zones: Vec<Zone> = response.json().await?;
        Ok(zones)
    }

    pub async fn create_zone(&self, name: &str, nameserver: &str) -> Result<Zone> {
        let url = format!("{}/api/v1/servers/localhost/zones", self.base_url);
        let request = CreateZoneRequest {
            name: name.to_string(),
            kind: "Native".to_string(),
            nameservers: vec![nameserver.to_string()],
        };

        info!("Creating zone: {}", name);
        let response = self.client
            .post(&url)
            .header("X-API-Key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            error!("Failed to create zone: {}", error_text);
            return Err(anyhow::anyhow!("Failed to create zone: {}", error_text));
        }

        let zone: Zone = response.json().await?;
        info!("Zone created: {}", zone.name);
        Ok(zone)
    }

    pub async fn create_a_record(&self, zone_name: &str, record_name: &str, ip: &str) -> Result<()> {
        let url = format!("{}/api/v1/servers/localhost/zones/{}", self.base_url, zone_name);
        
        let rrset = RrSet {
            name: format!("{}.", record_name),
            r#type: "A".to_string(),
            ttl: 3600,
            changetype: "REPLACE".to_string(),
            records: vec![Record {
                content: ip.to_string(),
                disabled: false,
            }],
        };

        let request = PatchZoneRequest {
            rrsets: vec![rrset],
        };

        info!("Creating A record: {} -> {}", record_name, ip);
        let response = self.client
            .patch(&url)
            .header("X-API-Key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            error!("Failed to create A record: {}", error_text);
            return Err(anyhow::anyhow!("Failed to create A record: {}", error_text));
        }

        info!("A record created successfully");
        Ok(())
    }

    pub async fn delete_a_record(&self, zone_name: &str, record_name: &str) -> Result<()> {
        let url = format!("{}/api/v1/servers/localhost/zones/{}", self.base_url, zone_name);
        
        let rrset = RrSet {
            name: format!("{}.", record_name),
            r#type: "A".to_string(),
            ttl: 3600,
            changetype: "DELETE".to_string(),
            records: vec![],
        };

        let request = PatchZoneRequest {
            rrsets: vec![rrset],
        };

        info!("Deleting A record: {}", record_name);
        let response = self.client
            .patch(&url)
            .header("X-API-Key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            error!("Failed to delete A record: {}", error_text);
            return Err(anyhow::anyhow!("Failed to delete A record: {}", error_text));
        }

        info!("A record deleted successfully");
        Ok(())
    }
}
