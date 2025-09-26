use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "configured_audio_devices")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub device_identifier: String,
    pub device_name: Option<String>,
    pub sample_rate: i32,
    pub buffer_size: Option<i32>,
    pub channel_format: String,
    pub is_virtual: bool,
    pub is_input: bool,
    pub configuration_id: String,
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
