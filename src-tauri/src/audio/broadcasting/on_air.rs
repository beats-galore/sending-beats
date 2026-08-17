// Which station is on air, and by which transmitter.
//
// There is one broadcast at a time, and until now the answer to "which one" was
// spread across whichever service happened to be holding a connection. With two
// transmitters that no longer works: coming off air has to reach the one that is
// actually running, and asking the wrong one reports a station that is not on
// while missing the one that is.
//
// Deliberately just the answer, not the machinery. The services own their own
// state; this owns which of them to ask.

use std::sync::{OnceLock, RwLock};

use super::protocol::CastProtocol;

/// The station currently broadcasting
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnAir {
    /// The cast configuration's row id, which is also what the mixer routes to
    pub station_id: String,
    pub protocol: CastProtocol,
}

fn slot() -> &'static RwLock<Option<OnAir>> {
    static SLOT: OnceLock<RwLock<Option<OnAir>>> = OnceLock::new();
    SLOT.get_or_init(|| RwLock::new(None))
}

pub fn set(station_id: String, protocol: CastProtocol) {
    if let Ok(mut current) = slot().write() {
        *current = Some(OnAir {
            station_id,
            protocol,
        });
    }
}

pub fn current() -> Option<OnAir> {
    slot().read().ok().and_then(|on_air| on_air.clone())
}

pub fn clear() {
    if let Ok(mut current) = slot().write() {
        *current = None;
    }
}
