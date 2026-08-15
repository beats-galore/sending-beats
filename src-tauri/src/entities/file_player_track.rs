use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// One file in a player, either waiting to play or already played.
///
/// `status` is what separates the queue from the history, and `position` orders
/// within each — the queue order for pending tracks, the order they were played
/// for the rest.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "file_player_tracks")]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub file_player_id: String,
    pub file_path: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    pub file_size: i64,
    pub status: String,
    pub position: i32,
    pub played_at: Option<ChronoDateTimeUtc>,
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
