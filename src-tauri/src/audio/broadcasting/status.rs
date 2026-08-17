// One answer to "are we on air", whichever transmitter is doing it.
//
// The interface asks this every couple of seconds and should not have to know
// which protocol a station uses to read the reply. So both transmitters report
// in the same shape, and the protocol-specific detail sits in its own optional
// field rather than changing the shape underneath a caller.

use super::impulse::get_impulse_service;
use super::on_air;
use super::protocol::CastProtocol;
use super::types::{
    BitrateInfo, ConnectionDiagnostics, ImpulseStreamingStats, StreamingServiceStatus,
};
use super::utils::get_streaming_status;

/// Whatever is on air now
///
/// The station and its protocol are attached here rather than by either
/// transmitter, because this is the only layer that knows which of them is
/// running. A caller reads them back to name the broadcast it wants stopped.
pub async fn cast_status() -> StreamingServiceStatus {
    let live = on_air::current();

    let mut status = match live.as_ref().map(|station| station.protocol) {
        Some(CastProtocol::Impulse) => impulse_status().await,
        // Nothing on air reads as the Icecast service's idea of nothing, which
        // is what every caller was already being given.
        _ => get_streaming_status().await,
    };

    status.station_id = live.as_ref().map(|station| station.station_id.clone());
    status.protocol = live.map(|station| station.protocol.as_str().to_string());

    status
}

async fn impulse_status() -> StreamingServiceStatus {
    let service = get_impulse_service().await;

    let is_running = service.is_running().await;
    let uptime_seconds = service.uptime_seconds().await;
    let stats = service.stats().await;
    let config = service.config().await;

    let impulse_stats = stats.as_ref().map(|stats| ImpulseStreamingStats {
        segments_sent: stats.segments_sent,
        bytes_sent: stats.bytes_sent,
        segments_dropped: stats.segments_dropped,
        send_errors: stats.send_errors,
        on_air: stats.on_air,
        media_sequence: stats.media_sequence,
    });

    StreamingServiceStatus {
        is_running,
        // Connected means the far end acknowledged a segment as on air, which is
        // a stronger claim than a socket being open: a connection can be up to a
        // server that is putting nothing in front of listeners.
        is_connected: stats.as_ref().is_some_and(|stats| stats.on_air),
        is_streaming: is_running,
        uptime_seconds,
        audio_stats: None,
        icecast_stats: None,
        impulse_stats,
        connection_diagnostics: ConnectionDiagnostics {
            // Nothing is held open to measure a round trip against, and the
            // per-segment request time is dominated by the segment's own length.
            latency_ms: None,
            packet_loss_rate: 0.0,
            connection_stability: stability(stats.as_ref()),
            // Every segment retries on its own, so there is no connection-level
            // reconnect to count.
            reconnect_attempts: 0,
            time_since_last_reconnect_seconds: None,
            connection_uptime_seconds: is_running.then_some(uptime_seconds),
        },
        bitrate_info: BitrateInfo {
            current_bitrate: config.as_ref().map(|c| c.bitrate_kbps).unwrap_or(192),
            available_bitrates: vec![96, 128, 160, 192, 256, 320],
            codec: "MP3".to_string(),
            // Segment durations are measured from the frames in them, so a
            // variable rate would work — but the encoder is not set up for one.
            is_variable_bitrate: false,
            vbr_quality: 0,
            actual_bitrate: None,
        },
        last_error: stats.and_then(|stats| stats.last_error),
        // Attached by `cast_status`, which owns the answer.
        station_id: None,
        protocol: None,
    }
}

/// How much of the broadcast is actually reaching the far end
///
/// Dropped segments are the only real failure here. A retry that succeeded cost
/// a moment and nothing else; a segment that never landed is a hole a listener
/// hears.
fn stability(stats: Option<&super::impulse::ImpulseStats>) -> f32 {
    let Some(stats) = stats else {
        return 0.0;
    };

    let attempted = stats.segments_sent + stats.segments_dropped;

    if attempted == 0 {
        return 1.0;
    }

    stats.segments_sent as f32 / attempted as f32
}

#[cfg(test)]
mod tests {
    use super::stability;
    use crate::audio::broadcasting::ImpulseStats;

    #[test]
    fn nothing_sent_yet_is_not_a_failure() {
        assert_eq!(stability(Some(&ImpulseStats::default())), 1.0);
    }

    #[test]
    fn a_clean_broadcast_is_whole() {
        let stats = ImpulseStats {
            segments_sent: 100,
            ..Default::default()
        };

        assert_eq!(stability(Some(&stats)), 1.0);
    }

    #[test]
    fn dropped_segments_are_what_reads_as_unstable() {
        let stats = ImpulseStats {
            segments_sent: 75,
            segments_dropped: 25,
            ..Default::default()
        };

        assert_eq!(stability(Some(&stats)), 0.75);
    }

    /// Retries are invisible on purpose: one that succeeded cost a moment, not
    /// any audio
    #[test]
    fn retried_segments_do_not_count_against_it() {
        let stats = ImpulseStats {
            segments_sent: 100,
            send_errors: 12,
            ..Default::default()
        };

        assert_eq!(stability(Some(&stats)), 1.0);
    }

    #[test]
    fn no_broadcast_reads_as_nothing() {
        assert_eq!(stability(None), 0.0);
    }
}
