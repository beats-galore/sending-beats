// Reading and writing the places this studio broadcasts to
//
// The row holds everything but the password. That lives in the keychain, keyed
// by the row's id, so the two are written and forgotten together while only one
// of them ever sits in a file.

use anyhow::Result;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, QueryOrder, Set};

use crate::entities::cast_configuration;

pub struct CastConfigurationService;

impl CastConfigurationService {
    /// Every station, in the order the picker shows them
    pub async fn list(db: &DatabaseConnection) -> Result<Vec<cast_configuration::Model>> {
        Ok(cast_configuration::Entity::find()
            .order_by_asc(cast_configuration::Column::Name)
            .all(db)
            .await?)
    }

    pub async fn get(
        db: &DatabaseConnection,
        id: &str,
    ) -> Result<Option<cast_configuration::Model>> {
        Ok(cast_configuration::Entity::find_by_id(id.to_string())
            .one(db)
            .await?)
    }

    /// Store a new station, taking the defaults for anything not filled in
    pub async fn create(
        db: &DatabaseConnection,
        draft: CastConfigurationDraft,
    ) -> Result<cast_configuration::Model> {
        let now = chrono::Utc::now();

        Ok(cast_configuration::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            name: Set(draft.name),
            server_host: Set(draft.server_host),
            server_port: Set(draft.server_port),
            mount_point: Set(draft.mount_point),
            username: Set(draft.username),
            stream_name: Set(draft.stream_name),
            stream_description: Set(draft.stream_description),
            stream_genre: Set(draft.stream_genre),
            stream_url: Set(draft.stream_url),
            is_public: Set(draft.is_public),
            audio_format: Set(draft.audio_format),
            bitrate_kbps: Set(draft.bitrate_kbps),
            variable_bitrate: Set(draft.variable_bitrate),
            vbr_quality: Set(draft.vbr_quality),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(db)
        .await?)
    }

    /// Replace a station's details, leaving its id and its password alone
    pub async fn update(
        db: &DatabaseConnection,
        id: &str,
        draft: CastConfigurationDraft,
    ) -> Result<cast_configuration::Model> {
        let row = cast_configuration::Entity::find_by_id(id.to_string())
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No cast configuration '{}'", id))?;

        let mut active: cast_configuration::ActiveModel = row.into();
        active.name = Set(draft.name);
        active.server_host = Set(draft.server_host);
        active.server_port = Set(draft.server_port);
        active.mount_point = Set(draft.mount_point);
        active.username = Set(draft.username);
        active.stream_name = Set(draft.stream_name);
        active.stream_description = Set(draft.stream_description);
        active.stream_genre = Set(draft.stream_genre);
        active.stream_url = Set(draft.stream_url);
        active.is_public = Set(draft.is_public);
        active.audio_format = Set(draft.audio_format);
        active.bitrate_kbps = Set(draft.bitrate_kbps);
        active.variable_bitrate = Set(draft.variable_bitrate);
        active.vbr_quality = Set(draft.vbr_quality);
        active.updated_at = Set(chrono::Utc::now());

        Ok(active.update(db).await?)
    }

    /// Forget a station, and the password stored with it
    ///
    /// The keychain entry goes first: a row removed with its secret left behind
    /// would leave a credential nothing refers to any more.
    pub async fn remove(db: &DatabaseConnection, id: &str) -> Result<()> {
        crate::db::cast_secrets::forget_password(id)?;

        cast_configuration::Entity::delete_by_id(id.to_string())
            .exec(db)
            .await?;

        Ok(())
    }
}

/// The editable half of a station
///
/// Everything the interface can set. The id, the timestamps and the password are
/// each owned by something else.
pub struct CastConfigurationDraft {
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
}
