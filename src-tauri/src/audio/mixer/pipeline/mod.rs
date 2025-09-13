// 4-Layer Audio Pipeline Architecture
//
// This module implements the queue-based layered audio processing pipeline
// designed to eliminate lock contention and enable scalable audio processing.
//
// Architecture:
// Layer 1: Device Input Capture → Input Queues
// Layer 2: Input Workers → [Resample→Max + Effects] → Processed Queues
// Layer 3: Mixing → [Sum Processed Streams] → Mixed Queue
// Layer 4: Output Workers → [Resample Mixed→Device Rate] → Device Output

pub mod input_worker;
pub mod mixing_layer;
pub mod output_worker;
pub mod pipeline_manager;
pub mod queue_types;

pub use pipeline_manager::AudioPipeline;
pub use queue_types::*;
