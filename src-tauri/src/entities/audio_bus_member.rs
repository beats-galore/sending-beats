use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// The directions a device can be attached to a bus in
///
/// Application-level, as text in the database. An input may appear on several
/// buses; an output on exactly one, which the routing registry enforces.
pub const BUS_MEMBER_INPUT: &str = "input";
pub const BUS_MEMBER_OUTPUT: &str = "output";

/// A device attached to a bus.
///
/// Keyed by `device_identifier`, the string the mixing layer routes by, rather
/// than by a configured_audio_devices row — that row is deleted and recreated
/// when a channel's source is switched, which would discard the routing.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "audio_bus_members")]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub bus_row_id: String,
    pub device_identifier: String,
    pub direction: String,
    pub created_at: ChronoDateTimeUtc,
    pub updated_at: ChronoDateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::audio_bus::Entity",
        from = "Column::BusRowId",
        to = "super::audio_bus::Column::Id",
        on_delete = "Cascade"
    )]
    AudioBus,
}

impl Related<super::audio_bus::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AudioBus.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
