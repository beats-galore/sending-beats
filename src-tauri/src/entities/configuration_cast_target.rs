use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// A station that is on a patch's canvas.
///
/// The station itself is global; this says which patches broadcast to it, so a
/// cast can be added and removed like any other destination.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "configuration_cast_targets")]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub configuration_id: String,
    pub cast_configuration_id: String,
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
    #[sea_orm(
        belongs_to = "super::cast_configuration::Entity",
        from = "Column::CastConfigurationId",
        to = "super::cast_configuration::Column::Id",
        on_delete = "Cascade"
    )]
    CastConfiguration,
}

impl Related<super::audio_mixer_configuration::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AudioMixerConfiguration.def()
    }
}

impl Related<super::cast_configuration::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CastConfiguration.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
