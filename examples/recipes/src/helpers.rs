use sqlx::PgPool;

use crate::embedder::Embedder;

pub struct AppState {
    pub pool: PgPool,
    pub embedder: Embedder,
}
