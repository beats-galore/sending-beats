use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "system_audio_state")]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub dummy_aggregate_device_uid: Option<String>,
    pub previous_default_device_uid: Option<String>,
    pub is_diverted: bool,
    pub created_at: ChronoDateTimeUtc,
    pub updated_at: ChronoDateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Create a new system audio state record
    pub fn new() -> ActiveModel {
        let now = chrono::Utc::now();
        ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            dummy_aggregate_device_uid: Set(None),
            previous_default_device_uid: Set(None),
            is_diverted: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
        }
    }
}

impl Default for Model {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            dummy_aggregate_device_uid: None,
            previous_default_device_uid: None,
            is_diverted: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }
}
