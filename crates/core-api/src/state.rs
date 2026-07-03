use async_nats::Client;
use bollard::Docker;
use sqlx::PgPool;

use crate::config::Config;
use crate::events::EventPublisher;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: PgPool,
    pub nats: Client,
    pub docker: Docker,
    pub event_publisher: EventPublisher,
}
