use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "audio_effects_custom")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub device_id: String,
    pub configuration_id: String,
    #[sea_orm(column_name = "type")]
    pub effect_type: String,
    pub parameters: String,  // JSON string
    pub created_at: ChronoDateTimeUtc,
    pub updated_at: ChronoDateTimeUtc,
    pub deleted_at: Option<ChronoDateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::audio_mixer_configuration::Entity",
        from = "Column::ConfigurationId",
        to = "super::audio_mixer_configuration::Column::Id"
    )]
    AudioMixerConfiguration,
}

impl Related<super::audio_mixer_configuration::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AudioMixerConfiguration.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}