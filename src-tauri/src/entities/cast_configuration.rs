use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Somewhere to broadcast to.
///
/// Global rather than per configuration: a station is a place in the world, and
/// the same one is streamed to from whichever patch is loaded.
///
/// No password field. It is held in the keychain under this row's `id`, so the
/// database can be copied without carrying a credential with it.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "cast_configurations")]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    pub server_host: String,
    pub server_port: i32,
    pub mount_point: String,
    pub username: String,
    pub stream_name: String,
    pub stream_description: String,
    pub stream_genre: String,
    pub stream_url: String,
    pub is_public: bool,
    pub audio_format: String,
    pub bitrate_kbps: i32,
    pub variable_bitrate: bool,
    pub vbr_quality: i32,
    pub created_at: ChronoDateTimeUtc,
    pub updated_at: ChronoDateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
