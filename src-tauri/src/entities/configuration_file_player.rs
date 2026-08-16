use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Which queues a patch has on its canvas.
///
/// The queue is global; this is the patch's side of it, so one can be put on a
/// canvas and taken off it like any other source.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "configuration_file_players")]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub configuration_id: String,
    pub file_player_id: String,
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
    #[sea_orm(
        belongs_to = "super::file_player::Entity",
        from = "Column::FilePlayerId",
        to = "super::file_player::Column::Id",
        on_delete = "Cascade"
    )]
    FilePlayer,
}

impl Related<super::file_player::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FilePlayer.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
