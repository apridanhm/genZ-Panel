use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{CreateDomainRequest, Domain, DomainResponse, UpdateDomainRequest};

pub async fn create_domain(
    db: &PgPool,
    user_id: Uuid,
    req: CreateDomainRequest,
) -> Result<DomainResponse, AppError> {
    let domain_name = req.domain_name.to_lowercase();
    let ssl_enabled = req.ssl_enabled.unwrap_or(true);

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

    // TODO: Publish NATS event untuk trigger DNS & SSL provisioning
    
    Ok(DomainResponse::from(domain))
}

pub async fn list_domains(
    db: &PgPool,
    user_id: Uuid,
) -> Result<Vec<DomainResponse>, AppError> {
    let domains = sqlx::query_as::<_, Domain>(
        "SELECT * FROM domains WHERE user_id = $1 ORDER BY created_at DESC"
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;

    Ok(domains.into_iter().map(DomainResponse::from).collect())
}

pub async fn get_domain(
    db: &PgPool,
    user_id: Uuid,
    domain_id: Uuid,
) -> Result<DomainResponse, AppError> {
    let domain = sqlx::query_as::<_, Domain>(
        "SELECT * FROM domains WHERE id = $1 AND user_id = $2"
    )
    .bind(domain_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(DomainResponse::from(domain))
}

pub async fn update_domain(
    db: &PgPool,
    user_id: Uuid,
    domain_id: Uuid,
    req: UpdateDomainRequest,
) -> Result<DomainResponse, AppError> {
    // Check if domain exists and belongs to user
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

    Ok(DomainResponse::from(updated_domain))
}

pub async fn delete_domain(
    db: &PgPool,
    user_id: Uuid,
    domain_id: Uuid,
) -> Result<(), AppError> {
    let result = sqlx::query(
        "DELETE FROM domains WHERE id = $1 AND user_id = $2"
    )
    .bind(domain_id)
    .bind(user_id)
    .execute(db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    // TODO: Publish NATS event untuk cleanup DNS & SSL
    
    Ok(())
}
