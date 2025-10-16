use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "audio_applications")]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub bundle_identifier: String,
    pub application_name: String,
    pub operating_system: String,
    pub is_enabled: bool,
    pub notes: Option<String>,
    pub created_at: ChronoDateTimeUtc,
    pub updated_at: ChronoDateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Create a new audio application record
    pub fn new(
        bundle_identifier: String,
        application_name: String,
        operating_system: String,
    ) -> ActiveModel {
        let now = chrono::Utc::now();
        ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            bundle_identifier: Set(bundle_identifier),
            application_name: Set(application_name),
            operating_system: Set(operating_system),
            is_enabled: Set(true),
            notes: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
    }
}
