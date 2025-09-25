use anyhow::{Context, Result};
use sea_orm::{Database, DatabaseConnection, ConnectOptions};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::error::Error;
use std::path::Path;
use std::time::Duration;

// Legacy SQLx table modules (keeping for VU levels and other non-migrated tables)
pub mod audio_device_levels;
pub mod broadcasts;
pub mod recordings;

// SeaORM services
pub mod seaorm_services;

// Re-export only legacy types that are still needed
pub use audio_device_levels::*;
pub use broadcasts::*;
pub use recordings::*;

/// SQLite-based database manager for audio system
pub struct AudioDatabase {
    pool: SqlitePool,
    sea_orm_db: DatabaseConnection,
    retention_seconds: i64,
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
        let mut opt = ConnectOptions::new(database_url.clone());
        opt.max_connections(10)
            .min_connections(1)
            .connect_timeout(Duration::from_secs(8))
            .idle_timeout(Duration::from_secs(8));

        let sea_orm_db = Database::connect(opt)
            .await
            .context("Failed to create SeaORM connection")?;

        println!("✅ SeaORM connection established");

        // Set default VU retention to 60 seconds
        let retention_seconds = 60;
        println!("📊 VU level retention set to {} seconds", retention_seconds);

        Ok(Self {
            pool,
            sea_orm_db,
            retention_seconds,
        })
    }

    /// Get database connection pool for advanced queries
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Get SeaORM database connection
    pub fn sea_orm(&self) -> &DatabaseConnection {
        &self.sea_orm_db
    }

    /// Cleanup old VU level data to prevent database growth
    pub async fn cleanup_old_vu_levels(&self) -> Result<u64> {
        let cutoff_timestamp =
            chrono::Utc::now().timestamp_micros() - (self.retention_seconds * 1_000_000);

        let result = sqlx::query(
            "UPDATE audio_device_levels SET deleted_at = CURRENT_TIMESTAMP
             WHERE created_at < ? AND deleted_at IS NULL",
        )
        .bind(
            chrono::DateTime::from_timestamp_micros(cutoff_timestamp)
                .unwrap()
                .to_rfc3339(),
        )
        .execute(&self.pool)
        .await?;

        let deleted_count = result.rows_affected();

        if deleted_count > 0 {
            println!(
                "🧹 Soft deleted {} old VU level records older than {}s",
                deleted_count, self.retention_seconds
            );
        }

        Ok(deleted_count)
    }
}

/// Lock-free audio event bus for real-time VU meter data
pub struct AudioEventBus {
    vu_events: std::sync::Arc<crossbeam::queue::SegQueue<VULevelData>>,
    max_queue_size: usize,
}

impl AudioEventBus {
    pub fn new(max_queue_size: usize) -> Self {
        Self {
            vu_events: std::sync::Arc::new(crossbeam::queue::SegQueue::new()),
            max_queue_size,
        }
    }

    /// Push VU level data (real-time safe, lock-free)
    pub fn push_vu_levels(&self, level: VULevelData) {
        // Prevent queue from growing too large
        while self.vu_events.len() >= self.max_queue_size {
            self.vu_events.pop();
        }

        self.vu_events.push(level);
    }

    /// Drain all VU level events
    pub fn drain_vu_events(&self) -> Vec<VULevelData> {
        let mut events = Vec::new();
        while let Some(event) = self.vu_events.pop() {
            events.push(event);
        }
        events
    }
}
