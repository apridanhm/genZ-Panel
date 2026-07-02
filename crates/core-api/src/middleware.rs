use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

// Middleware dummy untuk bypass auth dulu
pub async fn auth_middleware(request: Request, next: Next) -> Response {
    // Nanti kita tambahkan logic JWT di sini
    next.run(request).await
}
