## Lock-Free SPSC Audio Buffer Refactor

**Status**: PLANNED  
**Priority**: High  
**Date Identified**: 2025-09-04

**Description**: Replace `tokio::sync::Mutex<VecDeque<f32>>` audio buffers with lock-free Single Producer Single Consumer (SPSC) queues to eliminate timing drift caused by lock contention micro-delays that accumulate over thousands of audio processing cycles.

**Current Issues**:

- Lock acquisition delays (even 50-100μs) accumulate into significant timing drift (1.31ms observed)
- Audio crackle caused by occasional lock contention between audio input callbacks and mixer thread
- `tokio::sync::Mutex` adds unnecessary overhead for single producer/consumer scenario
- Lock contention diagnostics show timing variations that compound over time
- Current architecture has no theoretical guarantee of contention-free operation

**Proposed Solution**:

**Multi-Stage Lock-Free Pipeline Architecture**:

**Stage 1: Input Buffers (Multiple RTRB SPSC Queues)**
- Each input device/channel gets its own dedicated `rtrb::RingBuffer` 
- Single Producer per queue: Audio input callback thread (CPAL) for that device
- Single Consumer per queue: Mixer processing thread
- Benefits: Wait-free real-time operations, ~100-120ns latency, device isolation

**Stage 2: Mixed Output Queue (spmcq SPMC Queue)**  
- After mixer synchronizes inputs and applies gain/effects
- Single Producer: Mixer processing thread (after combining all inputs)
- Multiple Consumers: Recording service + Icecast streaming service + Core audio output  
- Benefits: Different consumer priorities, dropout detection, skip-ahead for lagging consumers

**Benefits**:
- Zero lock acquisition timing variations
- Complete isolation between input devices  
- Clean separation of mixed audio distribution
- Maintains exact same API surface for minimal disruption
- Enables independent consumer processing rates

**Files Affected**:

**Stage 1: Input Buffer Replacement**
- `src-tauri/src/audio/mixer/stream_management.rs` - Replace AudioInputStream buffer with per-device SPSC queue
- `src-tauri/src/audio/mixer/types.rs` - Update VirtualMixer to track multiple input queues
- `src-tauri/src/audio/devices/cpal_integration.rs` - Update audio callback to push to device-specific SPSC queue

**Stage 2: Mixed Output Pipeline**
- `src-tauri/src/audio/mixer/mixer_core.rs` - Add mixed output queue after synchronization/effects
- `src-tauri/src/audio/recording/` - Update recording service to consume from mixed output queue
- `src-tauri/src/audio/streaming/icecast.rs` - Update streaming service to consume from mixed output queue
- `src-tauri/src/audio/devices/coreaudio_stream.rs` - Update core audio output to consume from mixed output queue

**Dependencies**
- `src-tauri/Cargo.toml` - Add `rtrb` for SPSC input queues and `spmcq` for SPMC output distribution

**Implementation Steps**:

1. **Research and Dependency Selection** ✅ COMPLETED
   - **SPSC Choice: RTRB (Real-Time Ring Buffer)**
     - Specifically designed for real-time audio applications
     - ~100-120ns per operation, ~20% faster than crossbeam-queue
     - Wait-free operations with real-time guarantees
     - Widely adopted in Rust audio ecosystem
   - **SPMC Choice: spmcq (Single Producer Multiple Consumer Queue)**  
     - Perfect fit for audio producer with multiple consumers (recording/streaming/output)
     - Handles different consumer priorities (high-priority audio, low-priority GUI)
     - Built-in dropout detection and skip-ahead functionality
     - Updated in 2024 with active audio-focused maintenance

2. **Stage 1: Input Buffer Replacement (RTRB)**
   - Replace `Arc<tokio::sync::Mutex<VecDeque<f32>>>` with `rtrb::RingBuffer<f32>`
   - Update AudioInputStream constructor to create RTRB queue (capacity: 4096-8192 samples)
   - Modify get_samples() method to use `consumer.pop_slice()` for bulk operations  
   - Update process_with_effects() method to use lock-free `consumer.pop_slice()`
   - Audio callbacks use `producer.push_slice()` for efficient sample writing

3. **Stage 2: Mixed Output Pipeline (spmcq)**
   - Add `spmcq::Queue` after mixer synchronization/effects processing
   - Mixer thread becomes single producer using `writer.write(mixed_samples)`
   - Recording service gets dedicated `Reader` with `read()` operations
   - Streaming service gets dedicated `Reader` for Icecast processing  
   - Core audio output gets high-priority `Reader` for speaker output
   - Handle `ReadResult::Dropout` for consumers that fall behind

4. **Audio Callback Integration**
   - Update CPAL audio input callbacks to use `producer.push_slice()`
   - Remove all `mutex.lock()` calls from real-time audio path
   - Handle RTRB queue full with sample dropping (maintains real-time guarantees)
   - Use spmcq dropout detection for graceful consumer recovery

5. **Testing and Validation**
   - Verify timing drift elimination with extended testing (target: <0.1ms over 10 minutes)
   - Confirm audio quality remains identical across all consumer paths
   - Test RTRB buffer overflow behavior (sample dropping under load)
   - Test spmcq consumer dropout/recovery scenarios  
   - Performance benchmarks: CPU usage, latency, throughput vs current mutex implementation

**Testing Strategy**:

- **Timing Drift Test**: Run for 10+ minutes, measure drift accumulation vs current implementation
- **Audio Quality Test**: A/B test processed audio output for artifacts or differences  
- **Load Testing**: Test with multiple simultaneous input streams under CPU load
- **Buffer Behavior**: Test queue full/empty edge cases
- **Performance Benchmarks**: Measure CPU usage and latency improvements
- **Cross-Platform**: Verify operation on different operating systems

**Breaking Changes**: 

- None - maintaining identical public API surface
- Internal buffer implementation is completely hidden from external callers
- Existing audio processing logic unchanged

**Estimated Effort**: 

- **Research Phase**: 4 hours ✅ COMPLETED (RTRB + spmcq selected)
- **Stage 1 Implementation**: 4-6 hours (RTRB input queue replacement)
- **Stage 2 Implementation**: 4-6 hours (spmcq output distribution)  
- **Integration & Testing**: 4-6 hours (end-to-end validation and edge cases)
- **Total**: 16-22 hours over 3-4 sessions

**Key Technical Considerations**:

**Input Stage (RTRB SPSC Queues)**:
- **Queue Capacity**: 4096-8192 samples per device (matches typical audio buffer sizes)
- **Real-time Guarantees**: Wait-free operations, no blocking in audio callbacks
- **Device Isolation**: Complete independence, each device has dedicated producer/consumer pair
- **Timing Synchronization**: Mixer handles different fill rates with bulk `pop_slice()` operations

**Output Stage (spmcq SPMC Queue)**:
- **Consumer Priorities**: High-priority audio output, medium-priority streaming, low-priority recording
- **Dropout Handling**: `ReadResult::Dropout(_)` when consumer falls behind, can skip ahead
- **Rate Independence**: Each consumer reads at own pace, producer never blocks
- **Memory Efficiency**: Single copy of mixed audio, multiple lightweight readers
- **Error Recovery**: `reader.skip_ahead()` allows lagging consumers to catch up gracefully

**General**:
- **Memory Ordering**: Ensure proper atomic operations for cross-thread safety
- **Sample Drop Policy**: Define behavior when producer outpaces consumer (current: drop oldest samples)
- **Performance Characteristics**: RTRB provides O(1) operations, spmcq optimized for audio workloads  
- **Error Handling**: RTRB handles overflow by dropping samples, spmcq provides dropout detection
- **Buffer Alignment**: Both libraries optimized for audio with proper memory alignment
- **Consumer Starvation**: spmcq prevents slow consumers from blocking the producer
- **Data Types**: Both libraries work with `Copy` types (perfect for `f32` audio samples)

**Success Metrics**:

- Timing drift reduced to near-zero (< 0.1ms over 10 minutes)
- Audio crackle elimination
- CPU usage reduction in audio processing thread
- Maintained audio quality and all existing functionality