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

/// What a request to stop a named station should do
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopTarget {
    /// Nothing is on air, so there is nothing to do and nothing went wrong
    Nothing,
    /// Something else is on air. Stopping it would be answering a question
    /// nobody asked, so the request is refused and named.
    NotThisOne { live: String },
    /// Stop it, over the transmitter that started it
    Stop(CastProtocol),
}

/// Decide what stopping `requested` means, given what is actually on air
///
/// The protocol comes from here rather than from the station's row on purpose:
/// the row can be edited while a station is broadcasting, and a stop routed by
/// an edited protocol reaches a transmitter that was never started.
pub fn resolve_stop(live: Option<&OnAir>, requested: &str) -> StopTarget {
    match live {
        None => StopTarget::Nothing,
        Some(live) if live.station_id == requested => StopTarget::Stop(live.protocol),
        Some(live) => StopTarget::NotThisOne {
            live: live.station_id.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_stop, CastProtocol, OnAir, StopTarget};

    fn live_as(station_id: &str, protocol: CastProtocol) -> OnAir {
        OnAir {
            station_id: station_id.to_string(),
            protocol,
        }
    }

    /// The status poll lagging behind a broadcast that already ended. Not a
    /// failure worth reporting — the outcome asked for is the outcome there is.
    #[test]
    fn stopping_when_nothing_is_on_air_does_nothing() {
        assert_eq!(resolve_stop(None, "anything"), StopTarget::Nothing);
    }

    #[test]
    fn the_station_on_air_stops_over_its_own_transmitter() {
        let live = live_as("shady", CastProtocol::Impulse);

        assert_eq!(
            resolve_stop(Some(&live), "shady"),
            StopTarget::Stop(CastProtocol::Impulse)
        );
    }

    /// The failure this exists to prevent. Stopping whatever happened to be live
    /// would look like it worked and would have cut a broadcast nobody asked
    /// about — which is exactly what an argument-free stop cannot avoid.
    #[test]
    fn a_request_for_a_station_that_is_not_on_air_is_refused_and_named() {
        let live = live_as("shady", CastProtocol::Icecast);

        assert_eq!(
            resolve_stop(Some(&live), "some-other-station"),
            StopTarget::NotThisOne {
                live: "shady".to_string()
            }
        );
    }

    /// The protocol is whatever started the broadcast, so a row edited mid-show
    /// cannot route the stop to a transmitter that was never running
    #[test]
    fn the_protocol_comes_from_the_broadcast_rather_than_the_request() {
        let live = live_as("shady", CastProtocol::Icecast);

        assert_eq!(
            resolve_stop(Some(&live), "shady"),
            StopTarget::Stop(CastProtocol::Icecast)
        );
    }
}
