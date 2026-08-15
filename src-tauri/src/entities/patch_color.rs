use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// A colour the user gave to something on the patchbay.
///
/// Keyed by the patchbay's own vocabulary — `ch:<n>` for an input strip,
/// `out:<device identifier>` for a hardware destination, `stream` and `rec` for
/// the broadcast and the tape — because the things being coloured share no
/// common row to hang a column off.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "patch_colors")]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub configuration_id: String,
    pub target_key: String,
    pub color: String,
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
}

impl Related<super::audio_mixer_configuration::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AudioMixerConfiguration.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
