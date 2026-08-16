use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// A named queue of audio files that plays into the mixer as a source.
///
/// Global rather than owned by a configuration: the same run of ads belongs to
/// the station, not to whichever patch happened to be loaded when it was built.
/// A patch refers to one through `configuration_file_players`, the way it refers
/// to a cast configuration.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "file_players")]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    pub sample_rate: i32,
    pub channels: i32,
    pub volume: f32,
    pub repeat_mode: String,
    pub shuffle: bool,
    /// The track this queue pauses after, when one has been asked for
    pub breakpoint_track_id: Option<String>,
    pub created_at: ChronoDateTimeUtc,
    pub updated_at: ChronoDateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::file_player_track::Entity")]
    FilePlayerTrack,
    #[sea_orm(has_many = "super::file_player_play::Entity")]
    FilePlayerPlay,
}

impl Related<super::file_player_track::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FilePlayerTrack.def()
    }
}

impl Related<super::file_player_play::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FilePlayerPlay.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
