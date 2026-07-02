use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use validator::Validate;

use crate::error::AppError;
use crate::middleware::auth::Claims;
use crate::models::{LoginRequest, RegisterRequest};
use crate::services::auth;
use crate::state::AppState;

pub async fn root() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({
        "service": "GenZ Panel API",
        "version": "0.1.0",
        "status": "running",
        "endpoints": {
            "health": "/health",
            "register": "POST /api/v1/auth/register",
            "login": "POST /api/v1/auth/login",
            "me": "GET /api/v1/users/me (protected)"
        }
    })))
}

pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"status": "ok", "service": "core-api"})))
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<impl IntoResponse, AppError> {
    req.validate().map_err(|e| AppError::Validation(e.to_string()))?;
    let response = auth::register(&state.db, req, &state.config.jwt_secret).await?;
    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    req.validate().map_err(|e| AppError::Validation(e.to_string()))?;
    let response = auth::login(&state.db, req, &state.config.jwt_secret).await?;
    Ok(Json(response))
}

// Protected endpoint - pakai Extension<Claims>
pub async fn get_current_user(Extension(claims): Extension<Claims>) -> impl IntoResponse {
    (StatusCode::OK, Json(json!({
        "user_id": claims.sub,
        "email": claims.email,
        "role": claims.role
    })))
}

pub async fn list_domains(Extension(claims): Extension<Claims>) -> impl IntoResponse {
    (StatusCode::OK, Json(json!({
        "message": "List domains endpoint ready",
        "user_id": claims.sub
    })))
}

pub async fn create_domain(Extension(claims): Extension<Claims>) -> impl IntoResponse {
    (StatusCode::OK, Json(json!({
        "message": "Create domain endpoint ready",
        "user_id": claims.sub
    })))
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
