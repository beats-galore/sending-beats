// Reading and writing queues, what is in them, and what they have played
//
// A queue belongs to the studio rather than to a patch, the way a station does.
// What is in it is durable: playing a track does not take it out, it writes a
// row in the play log beside it. So an ad break built once is still there
// tomorrow, and "what went out last night" is a question the log answers.

use anyhow::Result;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};

use crate::entities::{
    configuration_file_player, file_player, file_player_play, file_player_track,
};

/// Persistence for queues. Named apart from `audio::FilePlayerService`, which is
/// the running player itself rather than what is remembered of it.
pub struct FilePlayerStore;

impl FilePlayerStore {
    /// Every queue in the studio, by name
    pub async fn list(db: &DatabaseConnection) -> Result<Vec<file_player::Model>> {
        Ok(file_player::Entity::find()
            .order_by_asc(file_player::Column::Name)
            .all(db)
            .await?)
    }

    pub async fn get(db: &DatabaseConnection, id: &str) -> Result<Option<file_player::Model>> {
        Ok(file_player::Entity::find_by_id(id.to_string())
            .one(db)
            .await?)
    }

    /// Store a new queue
    pub async fn create(
        db: &DatabaseConnection,
        name: &str,
        sample_rate: u32,
        channels: u16,
    ) -> Result<file_player::Model> {
        let now = chrono::Utc::now();

        Ok(file_player::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            name: Set(name.to_string()),
            sample_rate: Set(sample_rate as i32),
            channels: Set(channels as i32),
            volume: Set(1.0),
            repeat_mode: Set("none".to_string()),
            shuffle: Set(false),
            breakpoint_track_id: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(db)
        .await?)
    }

    pub async fn rename(db: &DatabaseConnection, id: &str, name: &str) -> Result<()> {
        let Some(row) = file_player::Entity::find_by_id(id.to_string())
            .one(db)
            .await?
        else {
            return Ok(());
        };

        let mut active: file_player::ActiveModel = row.into();
        active.name = Set(name.to_string());
        active.updated_at = Set(chrono::Utc::now());
        active.update(db).await?;

        Ok(())
    }

    /// Forget a queue. Its tracks and its play log go with it, by the cascade.
    pub async fn remove(db: &DatabaseConnection, id: &str) -> Result<()> {
        file_player::Entity::delete_by_id(id.to_string())
            .exec(db)
            .await?;
        Ok(())
    }

    /// Remember how a queue is set up, so it comes back the same way
    pub async fn update_playback(
        db: &DatabaseConnection,
        player_id: &str,
        volume: f32,
        repeat_mode: &str,
        shuffle: bool,
    ) -> Result<()> {
        let Some(row) = file_player::Entity::find_by_id(player_id.to_string())
            .one(db)
            .await?
        else {
            return Ok(());
        };

        let mut active: file_player::ActiveModel = row.into();
        active.volume = Set(volume);
        active.repeat_mode = Set(repeat_mode.to_string());
        active.shuffle = Set(shuffle);
        active.updated_at = Set(chrono::Utc::now());
        active.update(db).await?;

        Ok(())
    }

    /// Remember where a queue stops on its own, or that it no longer does
    pub async fn set_breakpoint(
        db: &DatabaseConnection,
        player_id: &str,
        track_id: Option<&str>,
    ) -> Result<()> {
        let Some(row) = file_player::Entity::find_by_id(player_id.to_string())
            .one(db)
            .await?
        else {
            return Ok(());
        };

        let mut active: file_player::ActiveModel = row.into();
        active.breakpoint_track_id = Set(track_id.map(str::to_string));
        active.updated_at = Set(chrono::Utc::now());
        active.update(db).await?;

        Ok(())
    }

    /// What is in a queue, in the order it plays
    pub async fn tracks(
        db: &DatabaseConnection,
        player_id: &str,
    ) -> Result<Vec<file_player_track::Model>> {
        Ok(file_player_track::Entity::find()
            .filter(file_player_track::Column::FilePlayerId.eq(player_id))
            .order_by_asc(file_player_track::Column::Position)
            .all(db)
            .await?)
    }

    /// Add a file to the end of a queue
    pub async fn queue_track(
        db: &DatabaseConnection,
        player_id: &str,
        track: QueuedTrackRow<'_>,
    ) -> Result<file_player_track::Model> {
        let now = chrono::Utc::now();

        let position = Self::tracks(db, player_id)
            .await?
            .last()
            .map_or(0, |last| last.position + 1);

        Ok(file_player_track::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            file_player_id: Set(player_id.to_string()),
            file_path: Set(track.file_path.to_string()),
            title: Set(track.title.map(str::to_string)),
            artist: Set(track.artist.map(str::to_string)),
            album: Set(track.album.map(str::to_string)),
            duration_ms: Set(track.duration_ms),
            file_size: Set(track.file_size),
            position: Set(position),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(db)
        .await?)
    }

    /// Write down the order a queue is now in
    ///
    /// Takes the whole list rather than one move, because by the time this is
    /// called the player has already done the move and knows the answer.
    pub async fn reorder_queue(
        db: &DatabaseConnection,
        player_id: &str,
        ordered_track_ids: &[String],
    ) -> Result<()> {
        let now = chrono::Utc::now();

        for (position, track_id) in ordered_track_ids.iter().enumerate() {
            let Some(row) = file_player_track::Entity::find_by_id(track_id.clone())
                .one(db)
                .await?
            else {
                continue;
            };

            if row.file_player_id != player_id || row.position == position as i32 {
                continue;
            }

            let mut active: file_player_track::ActiveModel = row.into();
            active.position = Set(position as i32);
            active.updated_at = Set(now);
            active.update(db).await?;
        }

        Ok(())
    }

    pub async fn remove_track(db: &DatabaseConnection, track_id: &str) -> Result<()> {
        file_player_track::Entity::delete_by_id(track_id.to_string())
            .exec(db)
            .await?;
        Ok(())
    }

    /// Empty a queue, leaving what it has played
    pub async fn clear_queue(db: &DatabaseConnection, player_id: &str) -> Result<()> {
        file_player_track::Entity::delete_many()
            .filter(file_player_track::Column::FilePlayerId.eq(player_id))
            .exec(db)
            .await?;
        Ok(())
    }

    /// Write down that a queue played something
    ///
    /// What the track was is copied in rather than only pointed at: the log has
    /// to still read as a list of what went out after the track is taken out of
    /// the queue it came from.
    pub async fn record_play(db: &DatabaseConnection, track_id: &str) -> Result<()> {
        let Some(track) = file_player_track::Entity::find_by_id(track_id.to_string())
            .one(db)
            .await?
        else {
            return Ok(());
        };

        let now = chrono::Utc::now();

        file_player_play::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            file_player_id: Set(track.file_player_id),
            track_id: Set(Some(track.id)),
            file_path: Set(track.file_path),
            title: Set(track.title),
            artist: Set(track.artist),
            duration_ms: Set(track.duration_ms),
            played_at: Set(now),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(db)
        .await?;

        Ok(())
    }

    /// What a queue has played, most recent first
    pub async fn plays(
        db: &DatabaseConnection,
        player_id: &str,
        limit: u64,
    ) -> Result<Vec<file_player_play::Model>> {
        Ok(file_player_play::Entity::find()
            .filter(file_player_play::Column::FilePlayerId.eq(player_id))
            .order_by_desc(file_player_play::Column::PlayedAt)
            .limit(limit)
            .all(db)
            .await?)
    }

    /// Forget what a queue has played, leaving the queue itself alone
    pub async fn clear_plays(db: &DatabaseConnection, player_id: &str) -> Result<()> {
        file_player_play::Entity::delete_many()
            .filter(file_player_play::Column::FilePlayerId.eq(player_id))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// What is known about a file when it is queued
///
/// Borrowed rather than owned because every field comes straight off a track the
/// caller already has, and the row is built and dropped in one step.
pub struct QueuedTrackRow<'a> {
    pub file_path: &'a str,
    pub title: Option<&'a str>,
    pub artist: Option<&'a str>,
    pub album: Option<&'a str>,
    pub duration_ms: Option<i64>,
    pub file_size: i64,
}

/// Which queues a patch has on its canvas
///
/// The queue is global; this is the patch's side of the relationship, so one can
/// be put on a canvas and taken off it like any other source.
pub struct FilePlayerTargetService;

impl FilePlayerTargetService {
    /// The queues on a patch, oldest first
    pub async fn list_for_configuration(
        db: &DatabaseConnection,
        configuration_id: &str,
    ) -> Result<Vec<String>> {
        let rows = configuration_file_player::Entity::find()
            .filter(configuration_file_player::Column::ConfigurationId.eq(configuration_id))
            .order_by_asc(configuration_file_player::Column::CreatedAt)
            .all(db)
            .await?;

        Ok(rows.into_iter().map(|row| row.file_player_id).collect())
    }

    /// Put a queue on a patch. Adding one already there changes nothing.
    pub async fn add(
        db: &DatabaseConnection,
        configuration_id: &str,
        file_player_id: &str,
    ) -> Result<()> {
        let existing = configuration_file_player::Entity::find()
            .filter(configuration_file_player::Column::ConfigurationId.eq(configuration_id))
            .filter(configuration_file_player::Column::FilePlayerId.eq(file_player_id))
            .one(db)
            .await?;

        if existing.is_some() {
            return Ok(());
        }

        let now = chrono::Utc::now();
        configuration_file_player::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            configuration_id: Set(configuration_id.to_string()),
            file_player_id: Set(file_player_id.to_string()),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(db)
        .await?;

        Ok(())
    }

    /// Take a queue off a patch. The queue itself is left alone.
    pub async fn remove(
        db: &DatabaseConnection,
        configuration_id: &str,
        file_player_id: &str,
    ) -> Result<()> {
        configuration_file_player::Entity::delete_many()
            .filter(configuration_file_player::Column::ConfigurationId.eq(configuration_id))
            .filter(configuration_file_player::Column::FilePlayerId.eq(file_player_id))
            .exec(db)
            .await?;

        Ok(())
    }
}
