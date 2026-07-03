use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

use crate::error::AppError;
use crate::middleware::auth::Claims;
use crate::models::{
    CreateDomainRequest, DomainResponse, LoginRequest, RegisterRequest,
    UpdateDomainRequest,
};
use crate::services::{auth, domain};
use crate::state::AppState;

#[derive(serde::Serialize, ToSchema)]
pub struct RootResponse {
    pub service: String,
    pub version: String,
    pub status: String,
    pub endpoints: serde_json::Value,
}

#[derive(serde::Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
}

#[derive(serde::Serialize, ToSchema)]
pub struct UserMeResponse {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub user_id: String,
    #[schema(example = "admin@genzpanel.com")]
    pub email: String,
    #[schema(example = "user")]
    pub role: String,
}

#[derive(serde::Serialize, ToSchema)]
pub struct DomainListResponse {
    pub domains: Vec<DomainResponse>,
    pub total: usize,
}

#[derive(Deserialize, IntoParams)]
pub struct DomainIdParam {
    #[param(value_type = String, example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
}

#[utoipa::path(
    get,
    path = "/",
    tag = "General",
    responses(
        (status = 200, description = "API information", body = RootResponse)
    )
)]
pub async fn root() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({
        "service": "GenZ Panel API",
        "version": "0.1.0",
        "status": "running",
        "endpoints": {
            "health": "/health",
            "register": "POST /api/v1/auth/register",
            "login": "POST /api/v1/auth/login",
            "me": "GET /api/v1/users/me (protected)",
            "domains": "GET/POST /api/v1/domains (protected)",
            "swagger": "/swagger-ui"
        }
    })))
}

#[utoipa::path(
    get,
    path = "/health",
    tag = "General",
    responses(
        (status = 200, description = "Health check", body = HealthResponse)
    )
)]
pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"status": "ok", "service": "core-api"})))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    tag = "Authentication",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "User registered successfully", body = AuthResponse),
        (status = 400, description = "Validation error"),
        (status = 409, description = "User already exists")
    )
)]
pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<impl IntoResponse, AppError> {
    req.validate().map_err(|e| AppError::Validation(e.to_string()))?;
    let response = auth::register(&state.db, req, &state.config.jwt_secret).await?;
    Ok((StatusCode::CREATED, Json(response)))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "Authentication",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 401, description = "Invalid credentials")
    )
)]
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    req.validate().map_err(|e| AppError::Validation(e.to_string()))?;
    let response = auth::login(&state.db, req, &state.config.jwt_secret).await?;
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/v1/users/me",
    tag = "Users",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Current user info", body = UserMeResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn get_current_user(Extension(claims): Extension<Claims>) -> impl IntoResponse {
    (StatusCode::OK, Json(UserMeResponse {
        user_id: claims.sub,
        email: claims.email,
        role: claims.role,
    }))
}

// Domain endpoints
#[utoipa::path(
    post,
    path = "/api/v1/domains",
    tag = "Domains",
    security(("bearer_auth" = [])),
    request_body = CreateDomainRequest,
    responses(
        (status = 201, description = "Domain created successfully", body = DomainResponse),
        (status = 400, description = "Validation error"),
        (status = 409, description = "Domain already exists")
    )
)]
pub async fn create_domain(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<CreateDomainRequest>,
) -> Result<impl IntoResponse, AppError> {
    req.validate().map_err(|e| AppError::Validation(e.to_string()))?;
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AppError::Internal)?;
    let response: DomainResponse = domain::create_domain(&state.db, user_id, req).await?;
    Ok((StatusCode::CREATED, Json(response)))
}

#[utoipa::path(
    get,
    path = "/api/v1/domains",
    tag = "Domains",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List of domains", body = DomainListResponse)
    )
)]
pub async fn list_domains(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AppError::Internal)?;
    let domains = domain::list_domains(&state.db, user_id).await?;
    let total = domains.len();
    Ok(Json(DomainListResponse { domains, total }))
}

#[utoipa::path(
    get,
    path = "/api/v1/domains/{id}",
    tag = "Domains",
    security(("bearer_auth" = [])),
    params(DomainIdParam),
    responses(
        (status = 200, description = "Domain details", body = DomainResponse),
        (status = 404, description = "Domain not found")
    )
)]
pub async fn get_domain(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AppError::Internal)?;
    let response: DomainResponse = domain::get_domain(&state.db, user_id, id).await?;
    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/api/v1/domains/{id}",
    tag = "Domains",
    security(("bearer_auth" = [])),
    params(DomainIdParam),
    request_body = UpdateDomainRequest,
    responses(
        (status = 200, description = "Domain updated successfully", body = DomainResponse),
        (status = 404, description = "Domain not found")
    )
)]
pub async fn update_domain(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateDomainRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AppError::Internal)?;
    let response: DomainResponse = domain::update_domain(&state.db, user_id, id, req).await?;
    Ok(Json(response))
}

#[utoipa::path(
    delete,
    path = "/api/v1/domains/{id}",
    tag = "Domains",
    security(("bearer_auth" = [])),
    params(DomainIdParam),
    responses(
        (status = 204, description = "Domain deleted successfully"),
        (status = 404, description = "Domain not found")
    )
)]
pub async fn delete_domain(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AppError::Internal)?;
    domain::delete_domain(&state.db, user_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_applications(Extension(claims): Extension<Claims>) -> impl IntoResponse {
    (StatusCode::OK, Json(json!({
        "message": "List apps endpoint ready",
        "user_id": claims.sub
    })))
}

pub async fn create_application(Extension(claims): Extension<Claims>) -> impl IntoResponse {
    (StatusCode::OK, Json(json!({
        "message": "Create app endpoint ready",
        "user_id": claims.sub
    })))
}
