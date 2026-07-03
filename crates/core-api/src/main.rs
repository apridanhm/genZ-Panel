#![allow(dead_code)]

use axum::{
    routing::{delete, get, post, put},
    Router,
};
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod config;
mod error;
mod events;
mod handlers;
mod middleware;
mod models;
mod openapi;
mod services;
mod state;

use config::Config;
use events::EventPublisher;
use openapi::ApiDoc;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "core_api=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Core API...");

    let config = Config::load()?;
    tracing::info!("Configuration loaded");

    let db_pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;
    tracing::info!("Connected to database");

    sqlx::migrate!("./migrations").run(&db_pool).await?;
    tracing::info!("Database migrations completed");

    let nats_client = async_nats::connect(config.nats_url.clone()).await?;
    tracing::info!("Connected to NATS");

    let docker_client = bollard::Docker::connect_with_local_defaults()?;
    tracing::info!("Connected to Docker");

    // Initialize Event Publisher
    let event_publisher = EventPublisher::new(nats_client.clone());

    let state = AppState {
        config,
        db: db_pool,
        nats: nats_client,
        docker: docker_client,
        event_publisher,
    };

    // Public routes
    let public_routes = Router::new()
        .route("/", get(handlers::root))
        .route("/health", get(handlers::health_check))
        .route("/api/v1/auth/register", post(handlers::register))
        .route("/api/v1/auth/login", post(handlers::login));

    // Protected routes
    let protected_routes = Router::new()
        .route("/api/v1/users/me", get(handlers::get_current_user))
        .route("/api/v1/domains", post(handlers::create_domain))
        .route("/api/v1/domains", get(handlers::list_domains))
        .route("/api/v1/domains/:id", get(handlers::get_domain))
        .route("/api/v1/domains/:id", put(handlers::update_domain))
        .route("/api/v1/domains/:id", delete(handlers::delete_domain))
        .route("/api/v1/applications", get(handlers::list_applications))
        .route("/api/v1/applications", post(handlers::create_application))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::auth::auth_middleware,
        ));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let swagger_ui = SwaggerUi::new("/swagger-ui")
        .url("/api-docs/openapi.json", ApiDoc::openapi());

    let app = public_routes
        .merge(protected_routes)
        .merge(swagger_ui)
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    tracing::info!("Server listening on {}", addr);
    tracing::info!("Swagger UI available at http://localhost:8000/swagger-ui");
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
