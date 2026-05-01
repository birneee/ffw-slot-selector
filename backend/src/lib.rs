pub mod db;
pub mod api;

#[allow(warnings)]
#[path = "generated/src/types.rs"]
pub mod types;
#[allow(warnings)]
#[path = "generated/src/header.rs"]
pub mod header;
#[allow(warnings)]
#[path = "generated/src/models.rs"]
pub mod models;
#[allow(warnings)]
#[path = "generated/src/apis/mod.rs"]
pub mod apis;
#[allow(warnings)]
#[path = "generated/src/server/mod.rs"]
pub mod server;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;

pub async fn create_pool(db_url: &str) -> anyhow::Result<sqlx::SqlitePool> {
    let options = SqliteConnectOptions::from_str(db_url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(16)
        .connect_with(options)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}
