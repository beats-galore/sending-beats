use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// One time a queue played something.
///
/// A log rather than a second copy of the list: a track played three times
/// reads as three rows. What the track was is written down here as well as
/// pointed at, because the log has to still make sense after the track is taken
/// out of the queue it came from.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "file_player_plays")]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub file_player_id: String,
    /// The track it was, while that track is still in the queue
    pub track_id: Option<String>,
    pub file_path: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub duration_ms: Option<i64>,
    pub played_at: ChronoDateTimeUtc,
    pub created_at: ChronoDateTimeUtc,
    pub updated_at: ChronoDateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::file_player::Entity",
        from = "Column::FilePlayerId",
        to = "super::file_player::Column::Id",
        on_delete = "Cascade"
    )]
    FilePlayer,
}

impl Related<super::file_player::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FilePlayer.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
