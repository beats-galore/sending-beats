// Reading and writing bus routing for a configuration
//
// Routing is small and always wanted whole — the mixing layer is handed every
// bus at once when a session opens — so these operate on the whole set rather
// than offering per-row edits.

use anyhow::Result;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};

use crate::audio::mixer::pipeline::bus_routing::Bus;
use crate::entities::{
    audio_bus,
    audio_bus_member::{self, BUS_MEMBER_INPUT, BUS_MEMBER_OUTPUT},
};

pub struct AudioBusService;

impl AudioBusService {
    /// Every stored bus for a configuration, with its members
    ///
    /// Returns an empty list when a configuration has never had routing saved,
    /// which the caller should read as "leave the defaults alone" rather than
    /// "remove everything".
    pub async fn load_for_configuration(
        db: &DatabaseConnection,
        configuration_id: &str,
    ) -> Result<Vec<Bus>> {
        let rows = audio_bus::Entity::find()
            .filter(audio_bus::Column::ConfigurationId.eq(configuration_id))
            .all(db)
            .await?;

        let mut buses = Vec::with_capacity(rows.len());

        for row in rows {
            let members = audio_bus_member::Entity::find()
                .filter(audio_bus_member::Column::BusRowId.eq(row.id.clone()))
                .all(db)
                .await?;

            let mut bus = Bus {
                id: row.bus_id,
                name: row.name,
                gain: row.gain,
                inputs: Default::default(),
                outputs: Default::default(),
            };

            for member in members {
                match member.direction.as_str() {
                    BUS_MEMBER_INPUT => {
                        bus.inputs.insert(member.device_identifier);
                    }
                    BUS_MEMBER_OUTPUT => {
                        bus.outputs.insert(member.device_identifier);
                    }
                    other => {
                        tracing::warn!(
                            "Ignoring bus member with unknown direction '{}' on bus '{}'",
                            other,
                            bus.id
                        );
                    }
                }
            }

            buses.push(bus);
        }

        Ok(buses)
    }

    /// Replace a configuration's routing with what the mixing layer currently has
    ///
    /// Written whole rather than as a diff because routing is a handful of rows
    /// and a partial write would leave the stored table describing a
    /// configuration that was never in use. Bus rows are matched on their
    /// identifier so they keep their created_at; members are cheap and are
    /// replaced outright.
    pub async fn save_for_configuration(
        db: &DatabaseConnection,
        configuration_id: &str,
        buses: &[Bus],
    ) -> Result<()> {
        let now = chrono::Utc::now();
        let transaction = db.begin().await?;

        let existing = audio_bus::Entity::find()
            .filter(audio_bus::Column::ConfigurationId.eq(configuration_id))
            .all(&transaction)
            .await?;

        // Buses the mixing layer no longer has, and everything attached to them
        for row in existing.iter() {
            if buses.iter().any(|bus| bus.id == row.bus_id) {
                continue;
            }

            // Deleted explicitly rather than left to ON DELETE CASCADE, which
            // does not fire unless SQLite has foreign key enforcement turned on
            Self::delete_members(&transaction, &row.id).await?;
            audio_bus::Entity::delete_by_id(row.id.clone())
                .exec(&transaction)
                .await?;
        }

        for bus in buses {
            let row_id = match existing.iter().find(|row| row.bus_id == bus.id) {
                Some(row) => {
                    let mut active: audio_bus::ActiveModel = row.clone().into();
                    active.name = Set(bus.name.clone());
                    active.gain = Set(bus.gain);
                    active.updated_at = Set(now);
                    active.update(&transaction).await?;
                    row.id.clone()
                }
                None => {
                    let row_id = uuid::Uuid::new_v4().to_string();
                    audio_bus::ActiveModel {
                        id: Set(row_id.clone()),
                        configuration_id: Set(configuration_id.to_string()),
                        bus_id: Set(bus.id.clone()),
                        name: Set(bus.name.clone()),
                        gain: Set(bus.gain),
                        created_at: Set(now),
                        updated_at: Set(now),
                    }
                    .insert(&transaction)
                    .await?;
                    row_id
                }
            };

            Self::delete_members(&transaction, &row_id).await?;

            let members = bus
                .inputs
                .iter()
                .map(|device| (device, BUS_MEMBER_INPUT))
                .chain(bus.outputs.iter().map(|device| (device, BUS_MEMBER_OUTPUT)));

            for (device_identifier, direction) in members {
                audio_bus_member::ActiveModel {
                    id: Set(uuid::Uuid::new_v4().to_string()),
                    bus_row_id: Set(row_id.clone()),
                    device_identifier: Set(device_identifier.clone()),
                    direction: Set(direction.to_string()),
                    created_at: Set(now),
                    updated_at: Set(now),
                }
                .insert(&transaction)
                .await?;
            }
        }

        transaction.commit().await?;
        Ok(())
    }

    async fn delete_members<C: sea_orm::ConnectionTrait>(db: &C, bus_row_id: &str) -> Result<()> {
        audio_bus_member::Entity::delete_many()
            .filter(audio_bus_member::Column::BusRowId.eq(bus_row_id))
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

    /// A migrated in-memory database, so the schema under test is the real one
    async fn db() -> DatabaseConnection {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool);

        // Routing hangs off a configuration, so one has to exist to point at
        sqlx::query("INSERT INTO audio_mixer_configurations (id, name, description, configuration_type, created_at, updated_at) VALUES (?, 'Test', '', 'session', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
            .bind(CONFIG)
            .execute(db.get_sqlite_connection_pool())
            .await
            .unwrap();

        db
    }

    fn bus(id: &str, name: &str, inputs: &[&str], outputs: &[&str]) -> Bus {
        Bus {
            id: id.to_string(),
            name: name.to_string(),
            gain: 1.0,
            inputs: inputs.iter().map(|s| s.to_string()).collect(),
            outputs: outputs.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn sorted(mut buses: Vec<Bus>) -> Vec<Bus> {
        buses.sort_by(|a, b| a.id.cmp(&b.id));
        buses
    }

    #[tokio::test]
    async fn a_configuration_with_no_saved_routing_loads_empty() {
        let db = db().await;

        let loaded = AudioBusService::load_for_configuration(&db, CONFIG)
            .await
            .unwrap();

        assert!(loaded.is_empty());
    }

    #[tokio::test]
    async fn routing_survives_a_round_trip() {
        let db = db().await;
        let buses = vec![
            bus("main", "Main", &["mic"], &["speakers"]),
            bus("cue", "Cue", &["deck"], &["headphones"]),
        ];

        AudioBusService::save_for_configuration(&db, CONFIG, &buses)
            .await
            .unwrap();
        let loaded = sorted(
            AudioBusService::load_for_configuration(&db, CONFIG)
                .await
                .unwrap(),
        );

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, "cue");
        assert_eq!(loaded[0].name, "Cue");
        assert_eq!(loaded[0].inputs, ["deck".to_string()].into_iter().collect());
        assert_eq!(
            loaded[0].outputs,
            ["headphones".to_string()].into_iter().collect()
        );
        assert_eq!(loaded[1].id, "main");
        assert_eq!(loaded[1].inputs, ["mic".to_string()].into_iter().collect());
    }

    #[tokio::test]
    async fn an_input_on_several_buses_is_stored_on_each() {
        let db = db().await;
        let buses = vec![
            bus("main", "Main", &["mic"], &["speakers"]),
            bus("stream", "Stream", &["mic"], &["icecast"]),
        ];

        AudioBusService::save_for_configuration(&db, CONFIG, &buses)
            .await
            .unwrap();
        let loaded = sorted(
            AudioBusService::load_for_configuration(&db, CONFIG)
                .await
                .unwrap(),
        );

        assert!(loaded[0].inputs.contains("mic"), "main keeps the mic");
        assert!(loaded[1].inputs.contains("mic"), "stream keeps it too");
    }

    #[tokio::test]
    async fn saving_again_replaces_what_was_there() {
        let db = db().await;

        AudioBusService::save_for_configuration(
            &db,
            CONFIG,
            &[bus("main", "Main", &["mic", "deck"], &["speakers"])],
        )
        .await
        .unwrap();
        AudioBusService::save_for_configuration(
            &db,
            CONFIG,
            &[bus("main", "Main", &["mic"], &["speakers"])],
        )
        .await
        .unwrap();

        let loaded = AudioBusService::load_for_configuration(&db, CONFIG)
            .await
            .unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(
            loaded[0].inputs,
            ["mic".to_string()].into_iter().collect(),
            "the removed input is gone rather than lingering"
        );
    }

    #[tokio::test]
    async fn a_removed_bus_takes_its_members_with_it() {
        let db = db().await;

        AudioBusService::save_for_configuration(
            &db,
            CONFIG,
            &[
                bus("main", "Main", &["mic"], &["speakers"]),
                bus("cue", "Cue", &["deck"], &["headphones"]),
            ],
        )
        .await
        .unwrap();
        AudioBusService::save_for_configuration(
            &db,
            CONFIG,
            &[bus("main", "Main", &["mic"], &["speakers", "headphones"])],
        )
        .await
        .unwrap();

        let loaded = AudioBusService::load_for_configuration(&db, CONFIG)
            .await
            .unwrap();
        assert_eq!(loaded.len(), 1);

        // Orphaned members would reappear if a bus with the same id came back
        let orphans = audio_bus_member::Entity::find()
            .all(&db)
            .await
            .unwrap()
            .len();
        assert_eq!(orphans, 3, "one input and two outputs on the one bus left");
    }

    #[tokio::test]
    async fn a_bus_keeps_its_row_across_saves() {
        let db = db().await;

        AudioBusService::save_for_configuration(&db, CONFIG, &[bus("main", "Main", &[], &[])])
            .await
            .unwrap();
        let first = audio_bus::Entity::find().all(&db).await.unwrap()[0].clone();

        AudioBusService::save_for_configuration(&db, CONFIG, &[bus("main", "Renamed", &[], &[])])
            .await
            .unwrap();
        let second = audio_bus::Entity::find().all(&db).await.unwrap()[0].clone();

        assert_eq!(first.id, second.id, "matched on identifier, not recreated");
        assert_eq!(first.created_at, second.created_at);
        assert_eq!(second.name, "Renamed");
    }

    #[tokio::test]
    async fn gain_is_stored_per_bus() {
        let db = db().await;
        let mut quiet = bus("cue", "Cue", &[], &[]);
        quiet.gain = 0.25;

        AudioBusService::save_for_configuration(&db, CONFIG, &[quiet])
            .await
            .unwrap();
        let loaded = AudioBusService::load_for_configuration(&db, CONFIG)
            .await
            .unwrap();

        assert!((loaded[0].gain - 0.25).abs() < 1e-6);
    }

    #[tokio::test]
    async fn routing_is_scoped_to_its_configuration() {
        let db = db().await;
        sqlx::query("INSERT INTO audio_mixer_configurations (id, name, description, configuration_type, created_at, updated_at) VALUES ('config-2', 'Other', '', 'session', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
            .execute(db.get_sqlite_connection_pool())
            .await
            .unwrap();

        AudioBusService::save_for_configuration(&db, CONFIG, &[bus("cue", "Cue", &["deck"], &[])])
            .await
            .unwrap();

        let other = AudioBusService::load_for_configuration(&db, "config-2")
            .await
            .unwrap();
        assert!(other.is_empty(), "another session sees none of it");

        // And saving there leaves the first alone
        AudioBusService::save_for_configuration(&db, "config-2", &[])
            .await
            .unwrap();
        assert_eq!(
            AudioBusService::load_for_configuration(&db, CONFIG)
                .await
                .unwrap()
                .len(),
            1
        );
    }
}
