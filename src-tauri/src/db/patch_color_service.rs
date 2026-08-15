// Reading and writing patchbay colours for a configuration

use anyhow::Result;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use std::collections::HashMap;

use crate::entities::patch_color;

pub struct PatchColorService;

impl PatchColorService {
    /// Every colour a configuration has been given, keyed by what it colours
    ///
    /// A thing with no entry has never been assigned one, which the interface
    /// should read as "pick an unused colour" rather than "no colour".
    pub async fn list_for_configuration(
        db: &DatabaseConnection,
        configuration_id: &str,
    ) -> Result<HashMap<String, String>> {
        let rows = patch_color::Entity::find()
            .filter(patch_color::Column::ConfigurationId.eq(configuration_id))
            .all(db)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| (row.target_key, row.color))
            .collect())
    }

    /// Give something a colour, replacing whatever it had
    pub async fn set(
        db: &DatabaseConnection,
        configuration_id: &str,
        target_key: &str,
        color: &str,
    ) -> Result<()> {
        let now = chrono::Utc::now();

        let existing = patch_color::Entity::find()
            .filter(patch_color::Column::ConfigurationId.eq(configuration_id))
            .filter(patch_color::Column::TargetKey.eq(target_key))
            .one(db)
            .await?;

        match existing {
            Some(row) => {
                let mut active: patch_color::ActiveModel = row.into();
                active.color = Set(color.to_string());
                active.updated_at = Set(now);
                active.update(db).await?;
            }
            None => {
                patch_color::ActiveModel {
                    id: Set(uuid::Uuid::new_v4().to_string()),
                    configuration_id: Set(configuration_id.to_string()),
                    target_key: Set(target_key.to_string()),
                    color: Set(color.to_string()),
                    created_at: Set(now),
                    updated_at: Set(now),
                }
                .insert(db)
                .await?;
            }
        }

        Ok(())
    }

    /// Forget something's colour, so it is assigned a fresh one next time
    pub async fn clear(
        db: &DatabaseConnection,
        configuration_id: &str,
        target_key: &str,
    ) -> Result<()> {
        patch_color::Entity::delete_many()
            .filter(patch_color::Column::ConfigurationId.eq(configuration_id))
            .filter(patch_color::Column::TargetKey.eq(target_key))
            .exec(db)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::SqlxSqliteConnector;
    use sqlx::sqlite::SqlitePoolOptions;

    const CONFIG: &str = "config-1";

    async fn db() -> DatabaseConnection {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool);

        sqlx::query("INSERT INTO audio_mixer_configurations (id, name, description, configuration_type, created_at, updated_at) VALUES (?, 'Test', '', 'session', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
            .bind(CONFIG)
            .execute(db.get_sqlite_connection_pool())
            .await
            .unwrap();

        db
    }

    #[tokio::test]
    async fn a_configuration_with_no_colours_lists_none() {
        let db = db().await;

        let colors = PatchColorService::list_for_configuration(&db, CONFIG)
            .await
            .unwrap();

        assert!(colors.is_empty());
    }

    #[tokio::test]
    async fn colours_survive_a_round_trip_for_both_sides_of_the_patch() {
        let db = db().await;

        PatchColorService::set(&db, CONFIG, "ch:2", "#F0C24F")
            .await
            .unwrap();
        PatchColorService::set(&db, CONFIG, "out:BlackHole2ch", "#6BE58A")
            .await
            .unwrap();
        PatchColorService::set(&db, CONFIG, "stream", "#FF5C77")
            .await
            .unwrap();

        let colors = PatchColorService::list_for_configuration(&db, CONFIG)
            .await
            .unwrap();

        assert_eq!(colors.get("ch:2"), Some(&"#F0C24F".to_string()));
        assert_eq!(
            colors.get("out:BlackHole2ch"),
            Some(&"#6BE58A".to_string()),
            "destinations are coloured the same way inputs are"
        );
        assert_eq!(colors.get("stream"), Some(&"#FF5C77".to_string()));
    }

    #[tokio::test]
    async fn setting_a_colour_again_replaces_it_rather_than_adding_one() {
        let db = db().await;

        PatchColorService::set(&db, CONFIG, "ch:2", "#F0C24F")
            .await
            .unwrap();
        PatchColorService::set(&db, CONFIG, "ch:2", "#9B8CFF")
            .await
            .unwrap();

        let colors = PatchColorService::list_for_configuration(&db, CONFIG)
            .await
            .unwrap();

        assert_eq!(colors.len(), 1);
        assert_eq!(colors.get("ch:2"), Some(&"#9B8CFF".to_string()));
    }

    #[tokio::test]
    async fn clearing_a_colour_frees_it_for_reassignment() {
        let db = db().await;
        PatchColorService::set(&db, CONFIG, "ch:2", "#F0C24F")
            .await
            .unwrap();

        PatchColorService::clear(&db, CONFIG, "ch:2").await.unwrap();

        assert!(PatchColorService::list_for_configuration(&db, CONFIG)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn colours_are_scoped_to_their_configuration() {
        let db = db().await;
        sqlx::query("INSERT INTO audio_mixer_configurations (id, name, description, configuration_type, created_at, updated_at) VALUES ('config-2', 'Other', '', 'session', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
            .execute(db.get_sqlite_connection_pool())
            .await
            .unwrap();

        PatchColorService::set(&db, CONFIG, "ch:2", "#F0C24F")
            .await
            .unwrap();
        PatchColorService::set(&db, "config-2", "ch:2", "#6BE58A")
            .await
            .unwrap();

        let first = PatchColorService::list_for_configuration(&db, CONFIG)
            .await
            .unwrap();
        let second = PatchColorService::list_for_configuration(&db, "config-2")
            .await
            .unwrap();

        assert_eq!(first.get("ch:2"), Some(&"#F0C24F".to_string()));
        assert_eq!(second.get("ch:2"), Some(&"#6BE58A".to_string()));
    }
}
