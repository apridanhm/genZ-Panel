use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

use crate::error::AppError;
use crate::events::{AppDeployTriggered, EventPublisher};
use crate::models::{Application, AppResponse, CreateAppRequest};

pub async fn create_app(
    db: &PgPool,
    user_id: Uuid,
    req: CreateAppRequest,
    publisher: &EventPublisher,
) -> Result<AppResponse, AppError> {
    info!("Creating application {} for user {}", req.name, user_id);

    // Set default values
    let source_type = req.source_type.unwrap_or_else(|| "git".to_string());
    let exposed_port = req.exposed_port.unwrap_or(3000);
    let cpu_limit = req.cpu_limit.unwrap_or(0.5);
    let ram_limit_mb = req.ram_limit_mb.unwrap_or(512);

    let app = sqlx::query_as::<_, Application>(
        r#"
        INSERT INTO applications (
            user_id, domain_id, name, runtime, runtime_version, path, 
            build_command, start_command, exposed_port, status, 
            env_vars, resources, source_type, git_repo_url, git_branch, zip_file_path
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, 'pending', $10, $11, $12, $13, $14, $15
        ) RETURNING *
        "#
    )
    .bind(user_id)
    .bind(req.domain_id)
    .bind(&req.name)
    .bind(&req.runtime)
    .bind(req.runtime_version)
    .bind(req.path.unwrap_or_else(|| "/app".to_string()))
    .bind(req.build_command)
    .bind(req.start_command)
    .bind(exposed_port)
    .bind(req.env_vars)
    .bind(req.resources)
    .bind(&source_type)
    .bind(req.git_repo_url)
    .bind(req.git_branch)
    .bind(req.zip_file_path)
    .fetch_one(db)
    .await?;

    info!("Application created in DB: {}", app.id);

    // Publish event untuk trigger Builder Daemon
    if let Err(e) = publisher.publish_app_deploy_triggered(AppDeployTriggered {
        app_id: app.id,
        domain_id: app.domain_id.unwrap_or_default(),
        user_id,
        name: app.name.clone(),
        runtime: app.runtime.clone(),
        source_type: app.source_type.clone().unwrap_or_else(|| "git".to_string()),
        git_repo_url: app.git_repo_url.clone(),
        git_branch: app.git_branch.clone(),
        zip_file_path: app.zip_file_path.clone(),
        build_command: app.build_command.clone(),
        start_command: app.start_command.clone().unwrap_or_default(),
        exposed_port: app.exposed_port.unwrap_or(3000),
    }).await {
        tracing::error!("Failed to publish app.deploy.triggered event: {}", e);
    }

    Ok(AppResponse::from(app))
}

pub async fn list_apps(
    db: &PgPool,
    user_id: Uuid,
) -> Result<Vec<AppResponse>, AppError> {
    info!("Listing applications for user {}", user_id);
    
    let apps = sqlx::query_as::<_, Application>(
        "SELECT * FROM applications WHERE user_id = $1 ORDER BY created_at DESC"
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;

    Ok(apps.into_iter().map(AppResponse::from).collect())
}
