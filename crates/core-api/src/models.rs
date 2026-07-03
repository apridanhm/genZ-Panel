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
    #[schema(value_type = String, example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    #[schema(value_type = String, example = "550e8400-e29b-41d4-a716-446655440000")]
    pub user_id: Uuid,
    #[schema(example = "example.com")]
    pub domain_name: String,
    #[schema(example = "active")]
    pub status: String,
    pub ssl_enabled: bool,
    pub ssl_cert_path: Option<String>,
    pub ssl_key_path: Option<String>,
    #[schema(value_type = Option<String>)]
    pub ssl_expires_at: Option<DateTime<Utc>>,
    #[schema(value_type = String)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String)]
    pub updated_at: DateTime<Utc>,
}

// --- Request DTOs ---
#[derive(Debug, Deserialize, validator::Validate, ToSchema)]
pub struct RegisterRequest {
    #[validate(email)]
    #[schema(example = "admin@genzpanel.com")]
    pub email: String,
    
    #[validate(length(min = 8, message = "Password minimal 8 karakter"))]
    #[schema(example = "supersecretpassword123")]
    pub password: String,
    
    #[validate(length(min = 2, max = 100))]
    #[schema(example = "Admin GenZ")]
    pub full_name: String,
}

#[derive(Debug, Deserialize, validator::Validate, ToSchema)]
pub struct LoginRequest {
    #[validate(email)]
    #[schema(example = "admin@genzpanel.com")]
    pub email: String,
    
    #[schema(example = "supersecretpassword123")]
    pub password: String,
}

#[derive(Debug, Deserialize, validator::Validate, ToSchema)]
pub struct CreateDomainRequest {
    #[validate(length(min = 3, max = 255))]
    #[schema(example = "example.com")]
    pub domain_name: String,
    
    #[schema(example = true)]
    pub ssl_enabled: Option<bool>,
}

#[derive(Debug, Deserialize, validator::Validate, ToSchema)]
pub struct UpdateDomainRequest {
    #[schema(example = true)]
    pub ssl_enabled: Option<bool>,
    
    #[schema(example = "active")]
    pub status: Option<String>,
}

// --- Response DTOs ---
#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    #[schema(example = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...")]
    pub token: String,
    pub user: UserResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserResponse {
    #[schema(value_type = String, example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    #[schema(example = "admin@genzpanel.com")]
    pub email: String,
    #[schema(example = "Admin GenZ")]
    pub full_name: String,
    #[schema(example = "user")]
    pub role: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DomainResponse {
    #[schema(value_type = String, example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    #[schema(example = "example.com")]
    pub domain_name: String,
    #[schema(example = "active")]
    pub status: String,
    pub ssl_enabled: bool,
    #[schema(value_type = Option<String>)]
    pub ssl_expires_at: Option<DateTime<Utc>>,
    #[schema(value_type = String)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String)]
    pub updated_at: DateTime<Utc>,
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
            ssl_expires_at: domain.ssl_expires_at,
            created_at: domain.created_at,
            updated_at: domain.updated_at,
        }
    }
}
