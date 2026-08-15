use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Where the user put something on the patchbay, and how big they made it.
///
/// An override on the computed layout rather than a replacement for it: a null
/// column means that part of the placement is still being derived, so a node
/// that was dragged but never resized keeps growing with its contents.
///
/// A pinned node takes its position from the node named by `pinned_to` instead
/// of from `x` and `y`, which is what carries a whole group when its anchor is
/// dragged. Both pin columns are set together or not at all.
///
/// Keyed by the patchbay's own vocabulary — `ch:<n>` for an input strip,
/// `bus:<id>` for a mix, `out:<device identifier>` for a hardware destination,
/// `stream` and `rec` for the broadcast and the tape.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "patch_layouts")]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub configuration_id: String,
    pub target_key: String,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    /// Target key of the node this one sits against, if any
    pub pinned_to: Option<String>,
    /// Which edge of that node — `bottom`, `left` or `right`
    pub pin_edge: Option<String>,
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
