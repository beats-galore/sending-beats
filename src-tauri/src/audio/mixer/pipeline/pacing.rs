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

/// How long a stage is assumed to be able to go unscheduled
///
/// Zero, because the stages between the callbacks now run on their own threads
/// against audio deadlines and no longer wait behind anything. What is left at
/// each handoff is the structural minimum — a consumer that takes a whole buffer
/// at a time needs a whole buffer to be there — and that floor is applied where
/// it is needed rather than added here.
///
/// This is the dial to raise if audio starts breaking up: it adds delay at every
/// handoff at once, in exchange for tolerating a stage being late.
pub const SCHEDULING_JITTER_MICROS: u64 = 0;

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
    fn a_duration_is_the_same_however_the_buffers_are_sized() {
        // The point of expressing a cushion in time: shrinking the hardware
        // buffer must not quietly shrink how long the pipeline can absorb a
        // stage being late.
        assert_eq!(samples_for_micros(10_000, 48_000, 2), 960); // 480 frames
        assert_eq!(samples_for_micros(10_000, 48_000, 1), 480);
        assert_eq!(samples_for_micros(10_000, 44_100, 2), 882);
    }

    #[test]
    fn zero_rate_costs_nothing_rather_than_dividing_by_it() {
        assert_eq!(samples_for_micros(10_000, 0, 2), 0);
    }

    #[test]
    fn nothing_is_held_for_scheduling_that_no_longer_happens() {
        assert_eq!(jitter_cushion_samples(48_000, 2), 0);
    }
}
