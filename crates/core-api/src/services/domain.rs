use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

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
        "INSERT INTO domains (user_id, domain_name, ssl_enabled) VALUES ($1, $2, $3) RETURNING *"
    )
    .bind(user_id)
    .bind(&domain_name)
    .bind(ssl_enabled)
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
    
    // Publish Event (Fire and Forget - kita log error tapi tidak block response)
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

    let ssl_enabled = req.ssl_enabled.unwrap_or(domain.ssl_enabled);
    let status = req.status.unwrap_or(domain.status);

    let updated_domain = sqlx::query_as::<_, Domain>(
        "UPDATE domains SET ssl_enabled = $1, status = $2, updated_at = NOW() WHERE id = $3 RETURNING *"
    )
    .bind(ssl_enabled)
    .bind(&status)
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
