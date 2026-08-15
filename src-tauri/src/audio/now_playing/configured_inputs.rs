// Which supported players the mixer is currently set up to capture.

use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter};

use super::types::SupportedPlayer;
use crate::entities::{audio_mixer_configuration, configured_audio_device};

/// Prefix the mixer stores application audio sources under, as
/// `app-<bundle identifier>`.
const APPLICATION_DEVICE_PREFIX: &str = "app-";

/// Supported players configured as inputs on the active session.
///
/// Track metadata is only worth asking for when the mixer is actually capturing
/// the app, so this bounds the watcher's work: with neither player configured it
/// returns empty and no scripts run at all.
pub async fn configured_players(db: &DatabaseConnection) -> Result<Vec<SupportedPlayer>, DbErr> {
    let Some(active_config) = audio_mixer_configuration::Entity::find()
        .filter(audio_mixer_configuration::Column::SessionActive.eq(true))
        .one(db)
        .await?
    else {
        return Ok(Vec::new());
    };

    let devices = configured_audio_device::Entity::find()
        .filter(configured_audio_device::Column::ConfigurationId.eq(&active_config.id))
        .filter(configured_audio_device::Column::IsInput.eq(true))
        .all(db)
        .await?;

    let mut players = Vec::new();
    for device in devices {
        let Some(bundle_id) = device
            .device_identifier
            .strip_prefix(APPLICATION_DEVICE_PREFIX)
        else {
            continue;
        };

        if let Some(player) = SupportedPlayer::from_bundle_id(bundle_id) {
            if !players.contains(&player) {
                players.push(player);
            }
        }
    }

    Ok(players)
}
