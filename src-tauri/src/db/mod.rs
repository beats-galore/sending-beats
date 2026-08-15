use anyhow::{Context, Result};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::error::Error;
use std::path::Path;
use std::time::Duration;

pub mod audio_bus_service;
pub mod broadcasts;
pub mod patch_color_service;
pub mod patch_layout_service;
pub mod recordings;

// SeaORM services
pub mod seaorm_services;

pub use audio_bus_service::AudioBusService;
pub use broadcasts::*;
pub use patch_color_service::PatchColorService;
pub use patch_layout_service::{PatchLayoutService, Placement};
pub use recordings::*;
pub use seaorm_services::{
    AudioEffectsDefaultService, AudioMixerConfigurationService, ConfiguredAudioDeviceService,
    SystemAudioStateService,
};

/// SQLite-based database manager for audio system
pub struct AudioDatabase {
    pool: SqlitePool,
    sea_orm_db: DatabaseConnection,
}

impl AudioDatabase {
    /// Initialize the database with automatic migrations
    pub async fn new(database_path: &Path) -> Result<Self> {
        println!(
            "🗄️  Initializing SQLite database at: {}",
            database_path.display()
        );

        // Ensure parent directory exists
        if let Some(parent) = database_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create database directory")?;
        }

        // Create connection pool with SQLite-specific options
        let database_url = format!("sqlite:{}?mode=rwc", database_path.display());
        println!("🗄️  Database URL: {}", database_url);

        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect(&database_url)
            .await
            .context("Failed to connect to SQLite database")?;

        println!(
            "✅ SQLite connection pool created with {} max connections",
            10
        );

        // Run migrations with detailed error reporting
        println!("🔄 Running database migrations...");
        if let Err(migration_error) = sqlx::migrate!("./migrations").run(&pool).await {
            // Print detailed migration error information
            eprintln!("❌ Database migration failed!");
            eprintln!("📄 Migration error details: {}", migration_error);

            // Print the error chain for even more context
            let mut source = migration_error.source();
            let mut level = 1;
            while let Some(err) = source {
                eprintln!("  {}. Caused by: {}", level, err);
                source = err.source();
                level += 1;
            }

            // Check if database is accessible
            match sqlx::query("SELECT 1").execute(&pool).await {
                Ok(_) => eprintln!(
                    "🔗 Database connection is working - issue is likely in migration SQL"
                ),
                Err(conn_err) => eprintln!("🚫 Database connection failed: {}", conn_err),
            }

            // List current migration files for debugging
            if let Ok(entries) = std::fs::read_dir("./migrations") {
                eprintln!("📁 Found migration files:");
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.ends_with(".sql") {
                            eprintln!("  - {}", name);
                        }
                    }
                }
            }

            return Err(migration_error.into());
        }

        println!("✅ Database migrations completed successfully");

        // Create SeaORM connection
        println!("🌊 Initializing SeaORM connection...");

        // Enable SQL logging via environment variable: SWEET_BEATS_STUDIO_SQL_LOGGING=1
        let enable_sql_logging =
            std::env::var("SWEET_BEATS_STUDIO_SQL_LOGGING").unwrap_or_default() == "1";

        let mut opt = ConnectOptions::new(database_url.clone());
        opt.max_connections(10)
            .min_connections(1)
            .connect_timeout(Duration::from_secs(8))
            .idle_timeout(Duration::from_secs(8))
            .sqlx_logging(enable_sql_logging);

        let sea_orm_db = Database::connect(opt)
            .await
            .context("Failed to create SeaORM connection")?;

        println!("✅ SeaORM connection established");

        Ok(Self { pool, sea_orm_db })
    }

    /// Get database connection pool for advanced queries
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Get SeaORM database connection
    pub fn sea_orm(&self) -> &DatabaseConnection {
        &self.sea_orm_db
    }
}
