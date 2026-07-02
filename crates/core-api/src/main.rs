#![allow(dead_code)] // <-- Ini yang menghilangkan warning dead_code

use axum::{
    routing::{get, post},
    Router,
};
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod error;
mod handlers;
mod middleware;
mod models;
mod services;
mod state;

use config::Config;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "core_api=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Core API...");

    // Load configuration
    let config = Config::load()?;
    tracing::info!("Configuration loaded");

    // Connect to database
    let db_pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;
    tracing::info!("Connected to database");

    // Run migrations
    sqlx::migrate!("./migrations").run(&db_pool).await?;
    tracing::info!("Database migrations completed");

    // Connect to NATS
    let nats_client = async_nats::connect(config.nats_url.clone()).await?;
    tracing::info!("Connected to NATS");

    // Initialize Docker client
    let docker_client = bollard::Docker::connect_with_local_defaults()?;
    tracing::info!("Connected to Docker");

    // Create app state
    let state = AppState {
        config,
        db: db_pool,
        nats: nats_client,
        docker: docker_client,
    };

    // Build router
    let app = Router::new()
        .route("/", get(handlers::root))
        .route("/health", get(handlers::health_check))
        .route("/api/v1/auth/register", post(handlers::register))
        .route("/api/v1/auth/login", post(handlers::login))
        .route("/api/v1/users/me", get(handlers::get_current_user))
        .route("/api/v1/domains", get(handlers::list_domains))
        .route("/api/v1/domains", post(handlers::create_domain))
        .route("/api/v1/applications", get(handlers::list_applications))
        .route("/api/v1/applications", post(handlers::create_application))
        .layer(axum::middleware::from_fn(middleware::auth_middleware))
        .with_state(state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    tracing::info!("Server listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
