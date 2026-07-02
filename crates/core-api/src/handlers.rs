use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"status": "ok", "service": "core-api"})))
}

pub async fn register() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"message": "Register endpoint ready"})))
}

pub async fn login() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"message": "Login endpoint ready"})))
}

pub async fn get_current_user() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"message": "Get user endpoint ready"})))
}

pub async fn list_domains() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"message": "List domains endpoint ready"})))
}

pub async fn create_domain() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"message": "Create domain endpoint ready"})))
}

pub async fn list_applications() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"message": "List apps endpoint ready"})))
}

pub async fn create_application() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"message": "Create app endpoint ready"})))
}

pub async fn root() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({
        "service": "GenZ Panel API",
        "version": "0.1.0",
        "status": "running",
        "endpoints": {
            "health": "/health",
            "register": "POST /api/v1/auth/register",
            "login": "POST /api/v1/auth/login"
        }
    })))
}
