// Reading and writing hand-arranged patchbay placements for a configuration

use anyhow::Result;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::entities::patch_layout;

/// Where a node sits and how big it is, in canvas coordinates.
///
/// Every field is optional because a placement is an override on the computed
/// layout: a node that was dragged but never resized carries a position and no
/// size, and still takes its height from whatever it is showing.
///
/// A pinned node takes its position from `pinned_to` rather than from `x` and
/// `y`, so those are left empty while the pin stands and written back when it
/// is released.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    /// Target key of the node this one sits against
    pub pinned_to: Option<String>,
    /// Which edge of that node — `bottom`, `left` or `right`
    pub pin_edge: Option<String>,
}

impl Placement {
    /// Nothing overridden — the same as having no row at all.
    fn is_empty(&self) -> bool {
        self.x.is_none()
            && self.y.is_none()
            && self.width.is_none()
            && self.height.is_none()
            && self.pinned_to.is_none()
    }
}

pub struct PatchLayoutService;

impl PatchLayoutService {
    /// Every placement a configuration has been given, keyed by what it places
    ///
    /// A thing with no entry has never been moved or resized, which the
    /// interface should read as "stack it in its column" rather than "put it at
    /// the origin".
    pub async fn list_for_configuration(
        db: &DatabaseConnection,
        configuration_id: &str,
    ) -> Result<HashMap<String, Placement>> {
        let rows = patch_layout::Entity::find()
            .filter(patch_layout::Column::ConfigurationId.eq(configuration_id))
            .all(db)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    row.target_key,
                    Placement {
                        x: row.x,
                        y: row.y,
                        width: row.width,
                        height: row.height,
                        pinned_to: row.pinned_to,
                        pin_edge: row.pin_edge,
                    },
                )
            })
            .collect())
    }

    /// Place something, replacing whatever placement it had
    ///
    /// The placement given is the whole of it rather than a patch over the
    /// stored one, so clearing a single axis is done by sending it as null. A
    /// placement that overrides nothing deletes the row instead of storing a
    /// record that says the layout is being computed anyway.
    pub async fn set(
        db: &DatabaseConnection,
        configuration_id: &str,
        target_key: &str,
        placement: Placement,
    ) -> Result<()> {
        if placement.is_empty() {
            return Self::clear(db, configuration_id, target_key).await;
        }

        let now = chrono::Utc::now();

        let existing = patch_layout::Entity::find()
            .filter(patch_layout::Column::ConfigurationId.eq(configuration_id))
            .filter(patch_layout::Column::TargetKey.eq(target_key))
            .one(db)
            .await?;

        match existing {
            Some(row) => {
                let mut active: patch_layout::ActiveModel = row.into();
                active.x = Set(placement.x);
                active.y = Set(placement.y);
                active.width = Set(placement.width);
                active.height = Set(placement.height);
                active.pinned_to = Set(placement.pinned_to.clone());
                active.pin_edge = Set(placement.pin_edge.clone());
                active.updated_at = Set(now);
                active.update(db).await?;
            }
            None => {
                patch_layout::ActiveModel {
                    id: Set(uuid::Uuid::new_v4().to_string()),
                    configuration_id: Set(configuration_id.to_string()),
                    target_key: Set(target_key.to_string()),
                    x: Set(placement.x),
                    y: Set(placement.y),
                    width: Set(placement.width),
                    height: Set(placement.height),
                    pinned_to: Set(placement.pinned_to.clone()),
                    pin_edge: Set(placement.pin_edge.clone()),
                    created_at: Set(now),
                    updated_at: Set(now),
                }
                .insert(db)
                .await?;
            }
        }

        Ok(())
    }

    /// Forget where something was put, so it goes back to its column
    ///
    /// Deliberately leaves anything pinned to it alone: the node still exists,
    /// it has only gone back to being placed by the canvas, and its followers
    /// should go with it. A pin left pointing at a node that has actually been
    /// deleted is read as no pin at all, so nothing has to be cleaned up here.
    pub async fn clear(
        db: &DatabaseConnection,
        configuration_id: &str,
        target_key: &str,
    ) -> Result<()> {
        patch_layout::Entity::delete_many()
            .filter(patch_layout::Column::ConfigurationId.eq(configuration_id))
            .filter(patch_layout::Column::TargetKey.eq(target_key))
            .exec(db)
            .await?;
        Ok(())
    }

    /// Forget the whole arrangement — what the interface's "tidy" does
    pub async fn clear_all(db: &DatabaseConnection, configuration_id: &str) -> Result<()> {
        patch_layout::Entity::delete_many()
            .filter(patch_layout::Column::ConfigurationId.eq(configuration_id))
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

    fn at(x: f64, y: f64) -> Placement {
        Placement {
            x: Some(x),
            y: Some(y),
            ..Placement::default()
        }
    }

    fn pinned(anchor: &str, edge: &str) -> Placement {
        Placement {
            pinned_to: Some(anchor.to_string()),
            pin_edge: Some(edge.to_string()),
            ..Placement::default()
        }
    }

    #[tokio::test]
    async fn a_configuration_nobody_has_arranged_lists_nothing() {
        let db = db().await;

        let placements = PatchLayoutService::list_for_configuration(&db, CONFIG)
            .await
            .unwrap();

        assert!(placements.is_empty());
    }

    #[tokio::test]
    async fn placements_survive_a_round_trip_for_every_kind_of_node() {
        let db = db().await;

        PatchLayoutService::set(&db, CONFIG, "ch:2", at(120.0, 240.0))
            .await
            .unwrap();
        PatchLayoutService::set(&db, CONFIG, "bus:main", at(600.0, 40.0))
            .await
            .unwrap();
        PatchLayoutService::set(&db, CONFIG, "out:BlackHole2ch", at(980.0, 300.0))
            .await
            .unwrap();
        PatchLayoutService::set(&db, CONFIG, "stream", at(980.0, 60.0))
            .await
            .unwrap();

        let placements = PatchLayoutService::list_for_configuration(&db, CONFIG)
            .await
            .unwrap();

        assert_eq!(placements.get("ch:2"), Some(&at(120.0, 240.0)));
        assert_eq!(
            placements.get("bus:main"),
            Some(&at(600.0, 40.0)),
            "mixes are placed the same way sources are"
        );
        assert_eq!(placements.get("out:BlackHole2ch"), Some(&at(980.0, 300.0)));
        assert_eq!(placements.get("stream"), Some(&at(980.0, 60.0)));
    }

    #[tokio::test]
    async fn a_node_that_was_only_moved_stores_no_size() {
        let db = db().await;

        PatchLayoutService::set(&db, CONFIG, "ch:2", at(120.0, 240.0))
            .await
            .unwrap();

        let stored = PatchLayoutService::list_for_configuration(&db, CONFIG)
            .await
            .unwrap();

        let placement = stored.get("ch:2").unwrap();
        assert_eq!(
            (placement.width, placement.height),
            (None, None),
            "a dragged node keeps taking its size from what it is showing"
        );
    }

    #[tokio::test]
    async fn a_node_that_was_only_resized_stores_no_position() {
        let db = db().await;

        PatchLayoutService::set(
            &db,
            CONFIG,
            "ch:2",
            Placement {
                width: Some(460.0),
                height: Some(472.0),
                ..Placement::default()
            },
        )
        .await
        .unwrap();

        let stored = PatchLayoutService::list_for_configuration(&db, CONFIG)
            .await
            .unwrap();

        let placement = stored.get("ch:2").unwrap();
        assert_eq!(
            (placement.x, placement.y),
            (None, None),
            "a node that was never dragged stays in the column it was stacked into"
        );
    }

    #[tokio::test]
    async fn placing_something_again_replaces_it_rather_than_adding_one() {
        let db = db().await;

        PatchLayoutService::set(&db, CONFIG, "ch:2", at(120.0, 240.0))
            .await
            .unwrap();
        PatchLayoutService::set(&db, CONFIG, "ch:2", at(300.0, 80.0))
            .await
            .unwrap();

        let placements = PatchLayoutService::list_for_configuration(&db, CONFIG)
            .await
            .unwrap();

        assert_eq!(placements.len(), 1);
        assert_eq!(placements.get("ch:2"), Some(&at(300.0, 80.0)));
    }

    #[tokio::test]
    async fn storing_a_placement_that_overrides_nothing_forgets_it() {
        let db = db().await;
        PatchLayoutService::set(&db, CONFIG, "ch:2", at(120.0, 240.0))
            .await
            .unwrap();

        PatchLayoutService::set(&db, CONFIG, "ch:2", Placement::default())
            .await
            .unwrap();

        assert!(
            PatchLayoutService::list_for_configuration(&db, CONFIG)
                .await
                .unwrap()
                .is_empty(),
            "a row saying every part of the layout is derived is the same as no row"
        );
    }

    #[tokio::test]
    async fn clearing_a_placement_puts_the_node_back_in_its_column() {
        let db = db().await;
        PatchLayoutService::set(&db, CONFIG, "ch:2", at(120.0, 240.0))
            .await
            .unwrap();

        PatchLayoutService::clear(&db, CONFIG, "ch:2")
            .await
            .unwrap();

        assert!(PatchLayoutService::list_for_configuration(&db, CONFIG)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn a_pin_survives_a_round_trip_and_stores_no_position() {
        let db = db().await;

        PatchLayoutService::set(&db, CONFIG, "ch:2", pinned("bus:main", "left"))
            .await
            .unwrap();

        let stored = PatchLayoutService::list_for_configuration(&db, CONFIG)
            .await
            .unwrap();

        let placement = stored.get("ch:2").unwrap();
        assert_eq!(placement.pinned_to.as_deref(), Some("bus:main"));
        assert_eq!(placement.pin_edge.as_deref(), Some("left"));
        assert_eq!(
            (placement.x, placement.y),
            (None, None),
            "a pinned node takes its position from its anchor, not from a stored one"
        );
    }

    #[tokio::test]
    async fn a_pin_alone_is_enough_to_keep_a_row() {
        let db = db().await;

        PatchLayoutService::set(&db, CONFIG, "ch:2", pinned("bus:main", "bottom"))
            .await
            .unwrap();

        assert_eq!(
            PatchLayoutService::list_for_configuration(&db, CONFIG)
                .await
                .unwrap()
                .len(),
            1,
            "a placement that overrides nothing but the pin is still a placement"
        );
    }

    #[tokio::test]
    async fn unpinning_puts_the_node_back_where_it_was_drawn() {
        let db = db().await;
        PatchLayoutService::set(&db, CONFIG, "ch:2", pinned("bus:main", "bottom"))
            .await
            .unwrap();

        // What releasing a pin does: the derived position becomes a stored one,
        // so the node stays exactly where it was rather than jumping.
        PatchLayoutService::set(&db, CONFIG, "ch:2", at(540.0, 228.0))
            .await
            .unwrap();

        let stored = PatchLayoutService::list_for_configuration(&db, CONFIG)
            .await
            .unwrap();

        let placement = stored.get("ch:2").unwrap();
        assert_eq!(placement.pinned_to, None);
        assert_eq!((placement.x, placement.y), (Some(540.0), Some(228.0)));
    }

    #[tokio::test]
    async fn clearing_an_anchor_leaves_its_followers_pinned_to_it() {
        let db = db().await;
        PatchLayoutService::set(&db, CONFIG, "bus:main", at(600.0, 40.0))
            .await
            .unwrap();
        PatchLayoutService::set(&db, CONFIG, "ch:2", pinned("bus:main", "left"))
            .await
            .unwrap();

        PatchLayoutService::clear(&db, CONFIG, "bus:main")
            .await
            .unwrap();

        let stored = PatchLayoutService::list_for_configuration(&db, CONFIG)
            .await
            .unwrap();

        assert_eq!(
            stored.get("ch:2").unwrap().pinned_to.as_deref(),
            Some("bus:main"),
            "the anchor has gone back to its column, not away, so the group holds"
        );
    }

    #[tokio::test]
    async fn tidying_forgets_the_whole_arrangement() {
        let db = db().await;
        PatchLayoutService::set(&db, CONFIG, "ch:2", at(120.0, 240.0))
            .await
            .unwrap();
        PatchLayoutService::set(&db, CONFIG, "bus:main", at(600.0, 40.0))
            .await
            .unwrap();

        PatchLayoutService::clear_all(&db, CONFIG).await.unwrap();

        assert!(PatchLayoutService::list_for_configuration(&db, CONFIG)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn tidying_one_configuration_leaves_the_others_arranged() {
        let db = db().await;
        sqlx::query("INSERT INTO audio_mixer_configurations (id, name, description, configuration_type, created_at, updated_at) VALUES ('config-2', 'Other', '', 'session', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
            .execute(db.get_sqlite_connection_pool())
            .await
            .unwrap();

        PatchLayoutService::set(&db, CONFIG, "ch:2", at(120.0, 240.0))
            .await
            .unwrap();
        PatchLayoutService::set(&db, "config-2", "ch:2", at(700.0, 90.0))
            .await
            .unwrap();

        PatchLayoutService::clear_all(&db, CONFIG).await.unwrap();

        assert!(PatchLayoutService::list_for_configuration(&db, CONFIG)
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            PatchLayoutService::list_for_configuration(&db, "config-2")
                .await
                .unwrap()
                .get("ch:2"),
            Some(&at(700.0, 90.0)),
            "an arrangement belongs to the patch it was made on"
        );
    }
}
