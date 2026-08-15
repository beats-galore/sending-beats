use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// A named mix that inputs send to and outputs take.
///
/// Stored per configuration, since routing is part of what a session is. The
/// `bus_id` column is the identifier the mixing layer routes by; `id` is the
/// row's own key and never reaches the audio path.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "audio_buses")]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub configuration_id: String,
    pub bus_id: String,
    pub name: String,
    pub gain: f32,
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
    #[sea_orm(has_many = "super::audio_bus_member::Entity")]
    AudioBusMember,
}

impl Related<super::audio_mixer_configuration::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AudioMixerConfiguration.def()
    }
}

impl Related<super::audio_bus_member::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AudioBusMember.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
