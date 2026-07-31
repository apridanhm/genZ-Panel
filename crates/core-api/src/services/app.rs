use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;
use bollard::Docker;
use std::fs;
use bollard::container::LogsOptions;
use futures::stream::StreamExt;
use axum::response::sse::{Event, Sse, KeepAlive};
use std::convert::Infallible;

use crate::error::AppError;
use crate::events::{AppDeployTriggered, AppDeleted, EventPublisher};
use crate::models::{Application, AppResponse, CreateAppRequest};

pub async fn create_app(
    db: &PgPool,
    user_id: Uuid,
    req: CreateAppRequest,
    publisher: &EventPublisher,
) -> Result<AppResponse, AppError> {
    info!("Creating application {} for user {}", req.name, user_id);

    let source_type = req.source_type.unwrap_or_else(|| "git".to_string());
    let exposed_port = req.exposed_port.unwrap_or(3000);

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

    if let Err(e) = publisher.publish_app_deploy_triggered(AppDeployTriggered {
        app_id: app.id,
        domain_id: app.domain_id.unwrap_or_default(),
        user_id,
        name: app.name.clone(),
        runtime: app.runtime.clone(),
        runtime_version: app.runtime_version.clone(),
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

pub async fn delete_app(
    db: &PgPool,
    docker: &Docker,
    publisher: &EventPublisher,
    user_id: Uuid,
    app_id: Uuid,
) -> Result<(), AppError> {
    info!("Deleting application {} for user {}", app_id, user_id);

    let app = sqlx::query_as::<_, Application>(
        "SELECT * FROM applications WHERE id = $1 AND user_id = $2"
    )
    .bind(app_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    if let Some(app_data) = app {
        if let Some(container_id) = &app_data.container_id {
            info!("Removing Docker container: {}", container_id);
            let _ = docker.remove_container(container_id, Some(bollard::container::RemoveContainerOptions {
                force: true,
                ..Default::default()
            })).await;
        }

        let app_dir = format!("/home/genZ-panel/apps/data/{}", app_id);
        if std::path::Path::new(&app_dir).exists() {
            info!("Removing app directory: {}", app_dir);
            let _ = fs::remove_dir_all(&app_dir);
        }

        let delete_event = AppDeleted {
            app_id,
            domain_id: app_data.domain_id.unwrap_or_default(),
        };
        let _ = publisher.publish_app_deleted(delete_event).await;

        sqlx::query!("DELETE FROM applications WHERE id = $1", app_id)
            .execute(db)
            .await?;

        info!("Application {} deleted successfully", app_id);
    }

    Ok(())
}

pub async fn stream_app_logs(
    db: &PgPool,
    docker: &Docker,
    user_id: Uuid,
    app_id: Uuid,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, AppError> {
    info!("Streaming logs for app {} by user {}", app_id, user_id);

    let app = sqlx::query_as::<_, Application>(
        "SELECT * FROM applications WHERE id = $1 AND user_id = $2"
    )
    .bind(app_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;

    let container_id = app.container_id.ok_or_else(|| {
        AppError::Internal // Menggunakan Internal karena BadRequest tidak ada di enum kamu
    })?;

    let options = LogsOptions::<String> {
        follow: true,
        stdout: true,
        stderr: true,
        tail: "100".to_string(),
        timestamps: false,
        ..Default::default()
    };

    let log_stream = docker.logs::<String>(&container_id, Some(options));

    let sse_stream = log_stream.map(move |log_result| {
        let data = match log_result {
            Ok(bollard::container::LogOutput::StdErr { message }) => {
                format!("[ERR] {}", String::from_utf8_lossy(&message).trim_end())
            }
            Ok(bollard::container::LogOutput::StdOut { message }) => {
                format!("[OUT] {}", String::from_utf8_lossy(&message).trim_end())
            }
            Ok(_) => return Ok(Event::default()),
            Err(e) => format!("[SYSTEM ERROR] {}", e),
        };
        Ok::<_, Infallible>(Event::default().data(data))
    });

    Ok(Sse::new(sse_stream).keep_alive(KeepAlive::new()))
}