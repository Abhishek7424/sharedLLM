pub mod models;
pub mod queries;

use anyhow::Result;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::str::FromStr;

pub async fn init_pool(database_url: &str) -> Result<SqlitePool> {
    // Parse the URL into connect options and enable file creation
    let connect_opts = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true);

    // Ensure the parent directory exists before sqlx tries to open the file
    if let Some(filename) = connect_opts.clone().get_filename().to_str() {
        if filename != ":memory:" {
            if let Some(parent) = std::path::Path::new(filename).parent() {
                if !parent.as_os_str().is_empty() {
                    tokio::fs::create_dir_all(parent).await?;
                }
            }
        }
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(connect_opts)
        .await?;

    // Run embedded migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    tracing::info!("Database initialized at {}", database_url);
    Ok(pool)
}
