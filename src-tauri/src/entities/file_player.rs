use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// A queue of audio files that plays into the mixer as an input source.
///
/// Stored per configuration, since what is queued is part of what a session is.
/// `device_identifier` is what the mixing layer routes by and what a channel is
/// patched to; `id` is the row's own key and is what that identifier is built
/// from, so it survives a restart.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "file_players")]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub configuration_id: String,
    pub device_identifier: String,
    pub name: String,
    pub sample_rate: i32,
    pub channels: i32,
    pub volume: f32,
    pub repeat_mode: String,
    pub shuffle: bool,
    /// The track this player pauses after, when one has been asked for
    pub breakpoint_track_id: Option<String>,
    pub created_at: ChronoDateTimeUtc,
    pub updated_at: ChronoDateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::audio_mixer_configuration::Entity",
        from = "Column::ConfigurationId",
        to = "super::audio_mixer_configuration::Column::Id"
    )]
    AudioMixerConfiguration,
    #[sea_orm(has_many = "super::file_player_track::Entity")]
    FilePlayerTrack,
}

impl Related<super::audio_mixer_configuration::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AudioMixerConfiguration.def()
    }
}

impl Related<super::file_player_track::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FilePlayerTrack.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
