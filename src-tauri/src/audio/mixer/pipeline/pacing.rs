// How much audio each handoff in the pipeline keeps in hand
//
// Every stage between the capture callback and the render callback is an
// ordinary async task on a shared runtime. The callbacks at either end are not:
// they run on CoreAudio's realtime threads and take or supply a full buffer every
// hardware period regardless of what the rest of the process is doing.
//
// A queue bridging those two worlds has to hold enough that the consumer is never
// empty while the producer is waiting to be scheduled. Hold too little and the
// shortfall is padded with silence, which is audible; hold too much and every
// sample of it is delay. So the amount is a duration — how long a stage can go
// unscheduled — and not a multiple of the buffer size, which has nothing to do
// with how the runtime behaves.

/// How long a non-realtime stage is assumed to be able to go unscheduled
///
/// This is the tuning dial for the latency/robustness trade across the whole
/// pipeline. Lowering it takes delay out of every handoff at once and starts
/// costing dropouts as soon as it dips under what the runtime actually does;
/// giving the audio stages their own realtime-priority threads is what makes a
/// lower value hold.
pub const SCHEDULING_JITTER_MICROS: u64 = 10_000;

/// Interleaved samples spanning `micros` of audio
pub fn samples_for_micros(micros: u64, sample_rate: u32, channels: u16) -> usize {
    let frames = (micros * sample_rate as u64) / 1_000_000;
    (frames * channels.max(1) as u64) as usize
}

/// Interleaved samples a handoff should keep in hand at this rate and layout
pub fn jitter_cushion_samples(sample_rate: u32, channels: u16) -> usize {
    samples_for_micros(SCHEDULING_JITTER_MICROS, sample_rate, channels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cushion_is_the_same_duration_whatever_the_buffer_size() {
        // The point of expressing it in time: shrinking the hardware buffer must
        // not quietly shrink how long the pipeline can absorb being descheduled.
        let stereo = jitter_cushion_samples(48_000, 2);
        assert_eq!(stereo, 960); // 480 frames, 10ms

        let mono = jitter_cushion_samples(48_000, 1);
        assert_eq!(mono, 480);

        let slower = jitter_cushion_samples(44_100, 2);
        assert_eq!(slower, 882);
    }

    #[test]
    fn zero_rate_costs_nothing_rather_than_dividing_by_it() {
        assert_eq!(jitter_cushion_samples(0, 2), 0);
    }
}
