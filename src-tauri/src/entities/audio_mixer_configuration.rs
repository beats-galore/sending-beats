use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "audio_mixer_configurations")]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub configuration_type: String,
    pub session_active: bool,
    pub reusable_configuration_id: Option<String>,
    pub is_default: bool,
    pub created_at: ChronoDateTimeUtc,
    pub updated_at: ChronoDateTimeUtc,
    pub deleted_at: Option<ChronoDateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "Entity",
        from = "Column::ReusableConfigurationId",
        to = "Column::Id"
    )]
    SelfReferencing,
    #[sea_orm(has_many = "super::configured_audio_device::Entity")]
    ConfiguredAudioDevices,
    #[sea_orm(has_many = "super::audio_effects_default::Entity")]
    AudioEffectsDefaults,
    #[sea_orm(has_many = "super::audio_effects_custom::Entity")]
    AudioEffectsCustom,
}

impl Related<super::configured_audio_device::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ConfiguredAudioDevices.def()
    }
}

impl Related<super::audio_effects_default::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AudioEffectsDefaults.def()
    }
}

impl Related<super::audio_effects_custom::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AudioEffectsCustom.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Create a new mixer configuration
    pub fn new(name: String, configuration_type: String) -> ActiveModel {
        let now = chrono::Utc::now();
        ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            name: Set(name),
            description: Set(None),
            configuration_type: Set(configuration_type),
            session_active: Set(false),
            reusable_configuration_id: Set(None),
            is_default: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
            deleted_at: Set(None),
        }
    }

    /// Create a new session-type configuration
    pub fn new_session(name: String) -> ActiveModel {
        Self::new(name, "session".to_string())
    }

    /// Create a new reusable configuration
    pub fn new_reusable(name: String) -> ActiveModel {
        Self::new(name, "reusable".to_string())
    }
}