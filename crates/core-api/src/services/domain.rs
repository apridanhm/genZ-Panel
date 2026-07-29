use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;
use std::fs;
use std::path::Path;

use crate::error::AppError;
use crate::events::{DomainCreated, DomainDeleted, DomainUpdated, EventPublisher};
use crate::models::{CreateDomainRequest, Domain, DomainResponse, UpdateDomainRequest};

pub async fn create_domain(
    db: &PgPool,
    user_id: Uuid,
    req: CreateDomainRequest,
    publisher: &EventPublisher,
) -> Result<DomainResponse, AppError> {
    let domain_name = req.domain_name.to_lowercase();
    let ssl_enabled = req.ssl_enabled.unwrap_or(true);

    info!("Creating domain {} for user {}", domain_name, user_id);

    let result = sqlx::query_as::<_, Domain>(
        "INSERT INTO domains (user_id, domain_name, ssl_enabled, ssl_provider) VALUES ($1, $2, $3, $4) RETURNING *"
    )
    .bind(user_id)
    .bind(&domain_name)
    .bind(ssl_enabled)
    .bind(if ssl_enabled { Some("letsencrypt") } else { None })
    .fetch_optional(db)
    .await;

    let domain = match result {
        Ok(Some(domain)) => domain,
        Ok(None) => return Err(AppError::Internal),
        Err(e) => {
            if e.to_string().contains("duplicate key") || e.to_string().contains("unique constraint") {
                return Err(AppError::DomainAlreadyExists);
            }
            return Err(AppError::Database(e));
        }
    };

    info!("Domain created in DB: {}", domain.id);
    
    // Publish Event
    if let Err(e) = publisher.publish_domain_created(DomainCreated {
        domain_id: domain.id,
        domain_name: domain.domain_name.clone(),
        user_id,
        ssl_enabled: domain.ssl_enabled,
        created_at: domain.created_at,
    }).await {
        tracing::error!("Failed to publish domain.created event: {}", e);
    }
    
    Ok(DomainResponse::from(domain))
}

pub async fn list_domains(
    db: &PgPool,
    user_id: Uuid,
) -> Result<Vec<DomainResponse>, AppError> {
    info!("Listing domains for user {}", user_id);
    
    let domains = sqlx::query_as::<_, Domain>(
        "SELECT * FROM domains WHERE user_id = $1 ORDER BY created_at DESC"
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;

    info!("Found {} domains", domains.len());
    
    Ok(domains.into_iter().map(DomainResponse::from).collect())
}

pub async fn get_domain(
    db: &PgPool,
    user_id: Uuid,
    domain_id: Uuid,
) -> Result<DomainResponse, AppError> {
    info!("Getting domain {} for user {}", domain_id, user_id);
    
    let domain = sqlx::query_as::<_, Domain>(
        "SELECT * FROM domains WHERE id = $1 AND user_id = $2"
    )
    .bind(domain_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?;
    
    match domain {
        Some(d) => {
            info!("Domain found: {}", d.domain_name);
            Ok(DomainResponse::from(d))
        }
        None => {
            info!("Domain not found");
            Err(AppError::NotFound)
        }
    }
}

pub async fn update_domain(
    db: &PgPool,
    user_id: Uuid,
    domain_id: Uuid,
    req: UpdateDomainRequest,
    publisher: &EventPublisher,
) -> Result<DomainResponse, AppError> {
    info!("Updating domain {} for user {}", domain_id, user_id);
    
    let domain = sqlx::query_as::<_, Domain>(
        "SELECT * FROM domains WHERE id = $1 AND user_id = $2"
    )
    .bind(domain_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;

    // Handle custom SSL upload
    let (ssl_cert_path, ssl_key_path, ssl_provider, ssl_expires_at) = if let (Some(cert), Some(key)) = (&req.ssl_certificate, &req.ssl_private_key) {
        // Validate certificate and key
        if !cert.contains("BEGIN CERTIFICATE") {
            return Err(AppError::Validation("Invalid SSL certificate format".to_string()));
        }
        if !key.contains("BEGIN PRIVATE KEY") && !key.contains("BEGIN RSA PRIVATE KEY") {
            return Err(AppError::Validation("Invalid private key format".to_string()));
        }

        // Create directory for custom SSL
        let ssl_dir = format!("/apps/ssl/custom/{}", domain_id);
        fs::create_dir_all(&ssl_dir).map_err(|_| AppError::Internal)?;

        // Save certificate and key
        let cert_path = format!("{}/cert.pem", ssl_dir);
        let key_path = format!("{}/key.pem", ssl_dir);
        
        fs::write(&cert_path, cert).map_err(|_| AppError::Internal)?;
        fs::write(&key_path, key).map_err(|_| AppError::Internal)?;

        // Set proper permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).map_err(|_| AppError::Internal)?;
        }

        info!("Custom SSL saved for domain {}", domain.domain_name);

        // Parse expiry from certificate (simplified - in production use x509 parser)
        let expires_at = chrono::Utc::now() + chrono::Duration::days(365); // Placeholder

        (Some(cert_path), Some(key_path), Some("custom".to_string()), Some(expires_at))
    } else {
        (domain.ssl_cert_path, domain.ssl_key_path, req.ssl_provider, domain.ssl_expires_at)
    };

    let ssl_enabled = req.ssl_enabled.unwrap_or(domain.ssl_enabled);
    let status = req.status.unwrap_or(domain.status);

    let updated_domain = sqlx::query_as::<_, Domain>(
        "UPDATE domains SET ssl_enabled = $1, status = $2, ssl_provider = $3, ssl_cert_path = $4, ssl_key_path = $5, ssl_expires_at = $6, updated_at = NOW() WHERE id = $7 RETURNING *"
    )
    .bind(ssl_enabled)
    .bind(&status)
    .bind(&ssl_provider)
    .bind(&ssl_cert_path)
    .bind(&ssl_key_path)
    .bind(ssl_expires_at)
    .bind(domain_id)
    .fetch_one(db)
    .await?;

    info!("Domain updated in DB: {}", updated_domain.domain_name);

    // Publish Event
    if let Err(e) = publisher.publish_domain_updated(DomainUpdated {
        domain_id: updated_domain.id,
        domain_name: updated_domain.domain_name.clone(),
        user_id,
        status: updated_domain.status.clone(),
        ssl_enabled: updated_domain.ssl_enabled,
        updated_at: updated_domain.updated_at,
    }).await {
        tracing::error!("Failed to publish domain.updated event: {}", e);
    }
    
    Ok(DomainResponse::from(updated_domain))
}

pub async fn delete_domain(
    db: &PgPool,
    user_id: Uuid,
    domain_id: Uuid,
    publisher: &EventPublisher,
) -> Result<(), AppError> {
    info!("Deleting domain {} for user {}", domain_id, user_id);
    
    // Fetch domain first to get details for the event
    let domain = sqlx::query_as::<_, Domain>(
        "SELECT * FROM domains WHERE id = $1 AND user_id = $2"
    )
    .bind(domain_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;

    // Clean up custom SSL files if exists
    if let Some(cert_path) = &domain.ssl_cert_path {
        if cert_path.contains("/apps/ssl/custom/") {
            let ssl_dir = Path::new(cert_path).parent().unwrap();
            if ssl_dir.exists() {
                fs::remove_dir_all(ssl_dir).ok();
            }
        }
    }

    let result = sqlx::query(
        "DELETE FROM domains WHERE id = $1 AND user_id = $2"
    )
    .bind(domain_id)
    .bind(user_id)
    .execute(db)
    .await?;

    if result.rows_affected() == 0 {
        info!("Domain not found for deletion");
        return Err(AppError::NotFound);
    }

    info!("Domain deleted from DB");

    // Publish Event
    if let Err(e) = publisher.publish_domain_deleted(DomainDeleted {
        domain_id: domain.id,
        domain_name: domain.domain_name,
        user_id,
    }).await {
        tracing::error!("Failed to publish domain.deleted event: {}", e);
    }
    
    Ok(())
}
