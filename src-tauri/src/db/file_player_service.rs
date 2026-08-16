// Reading and writing file players and what they have queued
//
// A player is stored against a configuration, and its tracks against the player.
// The identifier the mixer routes by is built from the row's own key, so a
// channel patched to a player still resolves after a restart — which is the
// whole reason any of this is on disk rather than in memory.

use anyhow::Result;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::entities::{file_player, file_player_track};

/// Whether a track is waiting to play or has already played
pub const TRACK_PENDING: &str = "pending";
pub const TRACK_PLAYED: &str = "played";

/// The identifier the mixing layer routes a player by
///
/// Built from the row key rather than minted separately, so there is one
/// identity per player and it is the one that survives a restart.
pub fn device_identifier_for(player_id: &str) -> String {
    format!("file_player_{}", player_id)
}

/// Persistence for file players. Named apart from `audio::FilePlayerService`,
/// which is the running player itself rather than what is remembered of it.
pub struct FilePlayerStore;

impl FilePlayerStore {
    /// Every player in a configuration, oldest first
    pub async fn list_for_configuration(
        db: &DatabaseConnection,
        configuration_id: &str,
    ) -> Result<Vec<file_player::Model>> {
        Ok(file_player::Entity::find()
            .filter(file_player::Column::ConfigurationId.eq(configuration_id))
            .order_by_asc(file_player::Column::CreatedAt)
            .all(db)
            .await?)
    }

    /// Store a new player and return it, with the identifier already derived
    pub async fn create(
        db: &DatabaseConnection,
        configuration_id: &str,
        name: &str,
        sample_rate: u32,
        channels: u16,
    ) -> Result<file_player::Model> {
        let now = chrono::Utc::now();
        let id = uuid::Uuid::new_v4().to_string();

        Ok(file_player::ActiveModel {
            id: Set(id.clone()),
            configuration_id: Set(configuration_id.to_string()),
            device_identifier: Set(device_identifier_for(&id)),
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

    /// Forget a player. Its tracks go with it, by the cascade on the table.
    pub async fn remove(db: &DatabaseConnection, player_id: &str) -> Result<()> {
        file_player::Entity::delete_by_id(player_id.to_string())
            .exec(db)
            .await?;
        Ok(())
    }

    /// Remember how a player is set up, so it comes back the same way
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

    /// Remember where a player stops on its own, or that it no longer does
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

    /// A player's tracks in one status, in order
    ///
    /// `TRACK_PENDING` is the queue as it will play; `TRACK_PLAYED` is the
    /// history in the order it happened.
    pub async fn tracks(
        db: &DatabaseConnection,
        player_id: &str,
        status: &str,
    ) -> Result<Vec<file_player_track::Model>> {
        Ok(file_player_track::Entity::find()
            .filter(file_player_track::Column::FilePlayerId.eq(player_id))
            .filter(file_player_track::Column::Status.eq(status))
            .order_by_asc(file_player_track::Column::Position)
            .all(db)
            .await?)
    }

    /// Add a file to the end of a player's queue
    pub async fn queue_track(
        db: &DatabaseConnection,
        player_id: &str,
        track: QueuedTrackRow<'_>,
    ) -> Result<file_player_track::Model> {
        let now = chrono::Utc::now();

        // Appended after whatever is already waiting, so adding a file while one
        // is playing puts it behind the rest rather than next.
        let position = Self::tracks(db, player_id, TRACK_PENDING)
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
            status: Set(TRACK_PENDING.to_string()),
            position: Set(position),
            played_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(db)
        .await?)
    }

    /// Write down the order a player's queue is now in
    ///
    /// Takes the whole list rather than one move, because by the time this is
    /// called the player has already done the move and knows the answer. Writing
    /// what it says is one pass and cannot drift from it, where replaying the
    /// move against stored positions could.
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

            // A track from another player, or one already at this position, is
            // nothing to write.
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

    /// Move a track into the history, stamped with when it finished
    ///
    /// Its position is re-taken from the end of the played list rather than kept
    /// from the queue, so history reads in the order things actually happened —
    /// which shuffle and repeat make different from the order they were queued.
    pub async fn mark_played(db: &DatabaseConnection, track_id: &str) -> Result<()> {
        let Some(row) = file_player_track::Entity::find_by_id(track_id.to_string())
            .one(db)
            .await?
        else {
            return Ok(());
        };

        let played_position = Self::tracks(db, &row.file_player_id, TRACK_PLAYED)
            .await?
            .last()
            .map_or(0, |last| last.position + 1);

        let now = chrono::Utc::now();
        let mut active: file_player_track::ActiveModel = row.into();
        active.status = Set(TRACK_PLAYED.to_string());
        active.position = Set(played_position);
        active.played_at = Set(Some(now));
        active.updated_at = Set(now);
        active.update(db).await?;

        Ok(())
    }

    /// Empty a player's queue, leaving its history alone
    pub async fn clear_queue(db: &DatabaseConnection, player_id: &str) -> Result<()> {
        file_player_track::Entity::delete_many()
            .filter(file_player_track::Column::FilePlayerId.eq(player_id))
            .filter(file_player_track::Column::Status.eq(TRACK_PENDING))
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
