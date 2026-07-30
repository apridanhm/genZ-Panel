use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use chrono::{DateTime, Utc};

// --- Database Models ---
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct User {
    #[schema(value_type = String, example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub full_name: String,
    pub role: String,
    pub status: String,
    #[schema(value_type = String)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String)]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Domain {
    #[schema(value_type = String)]
    pub id: Uuid,
    #[schema(value_type = String)]
    pub user_id: Uuid,
    pub domain_name: String,
    pub status: String,
    pub ssl_enabled: bool,
    pub ssl_provider: Option<String>,
    pub ssl_cert_path: Option<String>,
    pub ssl_key_path: Option<String>,
    #[schema(value_type = Option<String>)]
    pub ssl_expires_at: Option<DateTime<Utc>>,
    #[schema(value_type = String)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String)]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Application {
    #[schema(value_type = String)]
    pub id: Uuid,
    #[schema(value_type = String)]
    pub user_id: Uuid,
    #[schema(value_type = String)]
    pub domain_id: Option<Uuid>,
    pub name: String,
    pub runtime: String,
    pub runtime_version: Option<String>,
    pub path: String,
    pub build_command: Option<String>,
    pub start_command: Option<String>,
    pub exposed_port: Option<i32>,
    pub status: String,
    pub container_id: Option<String>,
    pub env_vars: Option<serde_json::Value>,
    pub resources: Option<serde_json::Value>,
    pub source_type: Option<String>,
    pub git_repo_url: Option<String>,
    pub git_branch: Option<String>,
    pub zip_file_path: Option<String>,
    #[schema(value_type = String)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String)]
    pub updated_at: DateTime<Utc>,
}

// --- Request DTOs ---
#[derive(Debug, Deserialize, validator::Validate, ToSchema)]
pub struct RegisterRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8))]
    pub password: String,
    #[validate(length(min = 2, max = 100))]
    pub full_name: String,
}

#[derive(Debug, Deserialize, validator::Validate, ToSchema)]
pub struct LoginRequest {
    #[validate(email)]
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize, validator::Validate, ToSchema)]
pub struct CreateDomainRequest {
    #[validate(length(min = 3, max = 255))]
    pub domain_name: String,
    pub ssl_enabled: Option<bool>,
}

#[derive(Debug, Deserialize, validator::Validate, ToSchema)]
pub struct UpdateDomainRequest {
    pub ssl_enabled: Option<bool>,
    pub status: Option<String>,
    pub ssl_provider: Option<String>,
    pub ssl_certificate: Option<String>,
    pub ssl_private_key: Option<String>,
}

#[derive(Debug, Deserialize, validator::Validate, ToSchema)]
pub struct CreateAppRequest {
    #[schema(value_type = String)]
    pub domain_id: Option<Uuid>,
    #[validate(length(min = 3, max = 100))]
    pub name: String,
    #[validate(length(min = 2, max = 50))]
    pub runtime: String,
    pub runtime_version: Option<String>,
    pub path: Option<String>,
    pub build_command: Option<String>,
    pub start_command: Option<String>,
    pub exposed_port: Option<i32>,
    pub env_vars: Option<serde_json::Value>,
    pub resources: Option<serde_json::Value>,
    pub source_type: Option<String>,
    pub git_repo_url: Option<String>,
    pub git_branch: Option<String>,
    pub zip_file_path: Option<String>,
}

// --- Response DTOs ---
#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserResponse {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub role: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DomainResponse {
    #[schema(value_type = String)]
    pub id: Uuid,
    pub domain_name: String,
    pub status: String,
    pub ssl_enabled: bool,
    pub ssl_provider: Option<String>,
    #[schema(value_type = Option<String>)]
    pub ssl_expires_at: Option<DateTime<Utc>>,
    #[schema(value_type = String)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String)]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AppResponse {
    #[schema(value_type = String)]
    pub id: Uuid,
    #[schema(value_type = String)]
    pub domain_id: Option<Uuid>,
    pub name: String,
    pub runtime: String,
    pub runtime_version: Option<String>,
    pub source_type: Option<String>,
    pub git_repo_url: Option<String>,
    pub git_branch: Option<String>,
    pub start_command: Option<String>,
    pub exposed_port: Option<i32>,
    pub status: String,
    #[schema(value_type = String)]
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            full_name: user.full_name,
            role: user.role,
        }
    }
}

impl From<Domain> for DomainResponse {
    fn from(domain: Domain) -> Self {
        Self {
            id: domain.id,
            domain_name: domain.domain_name,
            status: domain.status,
            ssl_enabled: domain.ssl_enabled,
            ssl_provider: domain.ssl_provider,
            ssl_expires_at: domain.ssl_expires_at,
            created_at: domain.created_at,
            updated_at: domain.updated_at,
        }
    }
}

impl From<Application> for AppResponse {
    fn from(app: Application) -> Self {
        Self {
            id: app.id,
            domain_id: app.domain_id,
            name: app.name,
            runtime: app.runtime,
            runtime_version: app.runtime_version,
            source_type: app.source_type,
            git_repo_url: app.git_repo_url,
            git_branch: app.git_branch,
            start_command: app.start_command,
            exposed_port: app.exposed_port,
            status: app.status,
            created_at: app.created_at,
        }
    }
}
