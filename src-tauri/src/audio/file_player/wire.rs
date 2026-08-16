// How a player's timings cross to the interface
//
// A `Duration` serialises as a pair of a seconds count and a nanoseconds count,
// which is exact and completely unusable on the other side: every readout would
// have to put the two back together before it could draw a progress bar. So
// durations go over as whole milliseconds, which is finer than anything the
// interface can show and coarser than anything it needs to add up.
//
// Inside the program they stay `Duration`, because that is what the player and
// the decoder actually work in.

use std::time::Duration;

use serde::{Deserialize, Deserializer, Serializer};

pub fn serialize<S: Serializer>(value: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_u64(value.as_millis() as u64)
}

pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
    u64::deserialize(deserializer).map(Duration::from_millis)
}

/// The same, for a duration a file may not declare
pub mod option {
    use super::*;

    pub fn serialize<S: Serializer>(
        value: &Option<Duration>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(duration) => serializer.serialize_some(&(duration.as_millis() as u64)),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Duration>, D::Error> {
        Ok(Option::<u64>::deserialize(deserializer)?.map(Duration::from_millis))
    }
}
