use anyhow::{Context, Result};
use cpal::SampleFormat;
use cpal::traits::DeviceTrait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::effects::AudioEffectsChain;
use super::types::AudioChannel;

// Audio stream management structures
#[derive(Debug)]
pub struct AudioInputStream {
    pub device_id: String,
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub audio_buffer: Arc<Mutex<Vec<f32>>>,
    pub effects_chain: Arc<Mutex<AudioEffectsChain>>,
    // Stream is managed separately via StreamManager to avoid Send/Sync issues
}

impl AudioInputStream {
    pub fn new(device_id: String, device_name: String, sample_rate: u32) -> Result<Self> {
        let audio_buffer = Arc::new(Mutex::new(Vec::new()));
        let effects_chain = Arc::new(Mutex::new(AudioEffectsChain::new(sample_rate)));
        
        Ok(AudioInputStream {
            device_id,
            device_name,
            sample_rate,
            channels: 1, // Start with mono
            audio_buffer,
            effects_chain,
        })
    }
    
    pub fn get_samples(&self) -> Vec<f32> {
        if let Ok(mut buffer) = self.audio_buffer.try_lock() {
            let samples = buffer.clone();
            let sample_count = samples.len();
            buffer.clear();
            
            // Debug: Log when we're actually reading samples
            use std::sync::{LazyLock, Mutex as StdMutex};
            static GET_SAMPLES_COUNT: LazyLock<StdMutex<std::collections::HashMap<String, u64>>> = 
                LazyLock::new(|| StdMutex::new(std::collections::HashMap::new()));
            
            if let Ok(mut count_map) = GET_SAMPLES_COUNT.lock() {
                let count = count_map.entry(self.device_id.clone()).or_insert(0);
                *count += 1;
                
                if sample_count > 0 {
                    if *count % 100 == 0 || (*count < 10) {
                        let peak = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                        let rms = (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
                        println!("📖 GET_SAMPLES [{}]: Retrieved {} samples (call #{}), peak: {:.4}, rms: {:.4}", 
                            self.device_id, sample_count, count, peak, rms);
                    }
                } else if *count % 500 == 0 {
                    println!("📪 GET_SAMPLES [{}]: Empty buffer (call #{})", self.device_id, count);
                }
            }
            
            samples
        } else {
            Vec::new()
        }
    }

    /// Apply effects to input samples and update channel settings
    pub fn process_with_effects(&self, channel: &AudioChannel) -> Vec<f32> {
        if let Ok(mut buffer) = self.audio_buffer.try_lock() {
            let mut samples = buffer.clone();
            let original_sample_count = samples.len();
            buffer.clear();
            
            // Debug: Log processing activity
            use std::sync::{LazyLock, Mutex as StdMutex};
            static PROCESS_COUNT: LazyLock<StdMutex<std::collections::HashMap<String, u64>>> = 
                LazyLock::new(|| StdMutex::new(std::collections::HashMap::new()));
            
            if let Ok(mut count_map) = PROCESS_COUNT.lock() {
                let count = count_map.entry(self.device_id.clone()).or_insert(0);
                *count += 1;
                
                if original_sample_count > 0 {
                    let original_peak = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                    
                    if *count % 100 == 0 || (*count < 10) {
                        println!("⚙️  PROCESS_WITH_EFFECTS [{}]: Processing {} samples (call #{}), peak: {:.4}, channel: {}", 
                            self.device_id, original_sample_count, count, original_peak, channel.name);
                        println!("   Settings: gain: {:.2}, muted: {}, effects: {}", 
                            channel.gain, channel.muted, channel.effects_enabled);
                    }
                }
            }

            // Apply effects if enabled
            if channel.effects_enabled && !samples.is_empty() {
                if let Ok(mut effects) = self.effects_chain.try_lock() {
                    // Update effects parameters based on channel settings
                    effects.set_eq_gain(super::effects::EQBand::Low, channel.eq_low_gain);
                    effects.set_eq_gain(super::effects::EQBand::Mid, channel.eq_mid_gain);
                    effects.set_eq_gain(super::effects::EQBand::High, channel.eq_high_gain);
                    
                    if channel.comp_enabled {
                        effects.set_compressor_params(
                            channel.comp_threshold,
                            channel.comp_ratio,
                            channel.comp_attack,
                            channel.comp_release,
                        );
                    }
                    
                    if channel.limiter_enabled {
                        effects.set_limiter_threshold(channel.limiter_threshold);
                    }

                    // Process samples through effects chain
                    effects.process(&mut samples);
                }
            }
            
            // **CRITICAL FIX**: Apply channel-specific gain and mute (this was missing!)
            if !channel.muted && channel.gain > 0.0 {
                for sample in samples.iter_mut() {
                    *sample *= channel.gain;
                }
                
                // Debug: Log final processed levels
                if let Ok(count_map) = PROCESS_COUNT.lock() {
                    let count = count_map.get(&self.device_id).unwrap_or(&0);
                    if original_sample_count > 0 && (*count % 100 == 0 || *count < 10) {
                        let final_peak = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                        let final_rms = (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
                        println!("✅ PROCESSED [{}]: Final {} samples, peak: {:.4}, rms: {:.4}", 
                            self.device_id, samples.len(), final_peak, final_rms);
                    }
                }
            } else {
                samples.fill(0.0);
                if let Ok(count_map) = PROCESS_COUNT.lock() {
                    let count = count_map.get(&self.device_id).unwrap_or(&0);
                    if original_sample_count > 0 && (*count % 200 == 0 || *count < 5) {
                        println!("🔇 MUTED/ZERO_GAIN [{}]: {} samples set to silence (muted: {}, gain: {:.2})", 
                            self.device_id, samples.len(), channel.muted, channel.gain);
                    }
                }
            }

            samples
        } else {
            Vec::new()
        }
    }
}

#[derive(Debug)]
pub struct AudioOutputStream {
    pub device_id: String,
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub input_buffer: Arc<Mutex<Vec<f32>>>,
    // Stream is handled separately to avoid Send/Sync issues
}

impl AudioOutputStream {
    pub fn new(device_id: String, device_name: String, sample_rate: u32) -> Result<Self> {
        let input_buffer = Arc::new(Mutex::new(Vec::new()));
        
        Ok(AudioOutputStream {
            device_id,
            device_name,
            sample_rate,
            channels: 2, // Stereo output
            input_buffer,
        })
    }
    
    pub fn send_samples(&self, samples: &[f32]) {
        if let Ok(mut buffer) = self.input_buffer.try_lock() {
            buffer.extend_from_slice(samples);
            // Limit buffer size to prevent memory issues
            let max_samples = self.sample_rate as usize * 2; // 2 seconds max
            let buffer_len = buffer.len();
            if buffer_len > max_samples {
                buffer.drain(0..(buffer_len - max_samples));
            }
        }
    }
}

// Stream management handles the actual cpal streams in a separate synchronous context
pub struct StreamManager {
    streams: HashMap<String, cpal::Stream>,
}

impl std::fmt::Debug for StreamManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamManager")
            .field("streams", &format!("{} streams", self.streams.len()))
            .finish()
    }
}

impl StreamManager {
    pub fn new() -> Self {
        Self {
            streams: HashMap::new(),
        }
    }

    pub fn add_input_stream(
        &mut self,
        device_id: String,
        device: cpal::Device,
        config: cpal::StreamConfig,
        audio_buffer: Arc<Mutex<Vec<f32>>>,
        target_sample_rate: u32,
    ) -> Result<()> {
        use cpal::SampleFormat;
        use cpal::traits::StreamTrait;
        
        let device_config = device.default_input_config().context("Failed to get device config")?;
        
        // Add debugging context
        let device_name_for_debug = device.name().unwrap_or_else(|_| "Unknown Device".to_string());
        let debug_device_id = device_id.clone();
        let debug_device_id_for_callback = debug_device_id.clone();
        let debug_device_id_for_error = debug_device_id.clone();
        
        let stream = match device_config.sample_format() {
            SampleFormat::F32 => {
                println!("🎤 Creating F32 input stream for: {} ({})", device_name_for_debug, debug_device_id);
                println!("   Config: {} channels, {} Hz, {} samples/buffer", 
                    config.channels, config.sample_rate.0, 
                    match &config.buffer_size { 
                        cpal::BufferSize::Fixed(s) => s.to_string(),
                        cpal::BufferSize::Default => "default".to_string()
                    });
                
                // Debug counters
                let mut callback_count = 0u64;
                let mut total_samples_captured = 0u64;
                let mut last_debug_time = std::time::Instant::now();
                
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        callback_count += 1;
                        
                        // Calculate audio levels for debugging
                        let peak_level = data.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                        let rms_level = (data.iter().map(|&s| s * s).sum::<f32>() / data.len() as f32).sqrt();
                        
                        // Keep stereo data as-is to prevent pitch shifting - don't convert to mono
                        let audio_samples: Vec<f32> = data.to_vec();
                        
                        total_samples_captured += audio_samples.len() as u64;
                        
                        // Debug logging every 2 seconds (approximately)
                        if callback_count % 200 == 0 || (peak_level > 0.01 && callback_count % 50 == 0) {
                            println!("🔊 INPUT [{}] Callback #{}: {} samples, peak: {:.4}, rms: {:.4}", 
                                debug_device_id_for_callback, callback_count, data.len(), peak_level, rms_level);
                            println!("   Total samples captured: {}, stereo samples: {}", total_samples_captured, audio_samples.len());
                        }
                        
                        // Store in buffer with additional debugging
                        if let Ok(mut buffer) = audio_buffer.try_lock() {
                            let buffer_size_before = buffer.len();
                            buffer.extend_from_slice(&audio_samples);
                            let buffer_size_after = buffer.len();
                            
                            // Only log buffer state changes when significant or debug needed
                            if buffer_size_before == 0 && buffer_size_after > 0 && callback_count < 10 {
                                println!("📦 BUFFER: First audio data stored in buffer for {}: {} samples", debug_device_id, buffer_size_after);
                            }
                            
                            let max_buffer_size = target_sample_rate as usize / 10; // 100ms max for real-time audio
                            if buffer.len() > max_buffer_size {
                                let excess = buffer.len() - max_buffer_size;
                                buffer.drain(0..excess);
                                if callback_count % 100 == 0 {
                                    println!("⚠️  BUFFER OVERFLOW: Drained {} samples from {}, now {} samples", 
                                        excess, debug_device_id, buffer.len());
                                }
                            }
                            
                            // Debug buffer state periodically
                            if callback_count % 500 == 0 && buffer.len() > 0 {
                                println!("📊 BUFFER STATUS [{}]: {} samples stored (max {})", 
                                    debug_device_id, buffer.len(), max_buffer_size);
                            }
                        } else {
                            if callback_count % 100 == 0 {
                                println!("🔒 BUFFER LOCK FAILED [{}]: Callback #{} couldn't access buffer", debug_device_id, callback_count);
                            }
                        }
                    },
                    {
                        let error_device_id = debug_device_id_for_error.clone();
                        move |err| eprintln!("❌ Audio input error [{}]: {}", error_device_id, err)
                    },
                    None
                )?
            },
            SampleFormat::I16 => {
                println!("🎤 Creating I16 input stream for: {} ({})", device_name_for_debug, debug_device_id);
                
                let mut callback_count = 0u64;
                let debug_device_id_i16 = debug_device_id.clone();
                let debug_device_id_i16_error = debug_device_id.clone();
                
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        callback_count += 1;
                        
                        let f32_samples: Vec<f32> = data.iter()
                            .map(|&sample| sample as f32 / 32768.0)
                            .collect();
                        
                        let peak_level = f32_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                        let rms_level = (f32_samples.iter().map(|&s| s * s).sum::<f32>() / f32_samples.len() as f32).sqrt();
                            
                        // Keep stereo data as-is to prevent pitch shifting - don't convert to mono
                        let audio_samples = f32_samples;
                        
                        if callback_count % 200 == 0 || (peak_level > 0.01 && callback_count % 50 == 0) {
                            println!("🔊 INPUT I16 [{}] Callback #{}: {} samples, peak: {:.4}, rms: {:.4}", 
                                debug_device_id_i16, callback_count, data.len(), peak_level, rms_level);
                        }
                        
                        if let Ok(mut buffer) = audio_buffer.try_lock() {
                            let buffer_size_before = buffer.len();
                            buffer.extend_from_slice(&audio_samples);
                            
                            if buffer_size_before == 0 && buffer.len() > 0 && callback_count < 10 {
                                println!("📦 BUFFER I16: First audio data stored for {}: {} samples", debug_device_id_i16, buffer.len());
                            }
                            
                            let max_buffer_size = target_sample_rate as usize / 10; // 100ms max for real-time audio
                            if buffer.len() > max_buffer_size {
                                let excess = buffer.len() - max_buffer_size;
                                buffer.drain(0..excess);
                                if callback_count % 100 == 0 {
                                    println!("⚠️  BUFFER OVERFLOW I16: Drained {} samples from {}", excess, debug_device_id_i16);
                                }
                            }
                        }
                    },
                    {
                        let error_device_id = debug_device_id_i16_error.clone();
                        move |err| eprintln!("❌ Audio input error I16 [{}]: {}", error_device_id, err)
                    },
                    None
                )?
            },
            SampleFormat::U16 => {
                device.build_input_stream(
                    &config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        let f32_samples: Vec<f32> = data.iter()
                            .map(|&sample| (sample as f32 - 32768.0) / 32768.0)
                            .collect();
                            
                        // Keep stereo data as-is to prevent pitch shifting - don't convert to mono
                        let audio_samples = f32_samples;
                        
                        if let Ok(mut buffer) = audio_buffer.try_lock() {
                            buffer.extend_from_slice(&audio_samples);
                            let max_buffer_size = target_sample_rate as usize / 10; // 100ms max for real-time audio
                            if buffer.len() > max_buffer_size {
                                let excess = buffer.len() - max_buffer_size;
                                buffer.drain(0..excess);
                            }
                        }
                    },
                    |err| eprintln!("Audio input error: {}", err),
                    None
                )?
            },
            _ => {
                return Err(anyhow::anyhow!("Unsupported sample format: {:?}", device_config.sample_format()));
            }
        };
        
        stream.play().context("Failed to start input stream")?;
        self.streams.insert(device_id, stream);
        
        Ok(())
    }
    
    pub fn remove_stream(&mut self, device_id: &str) -> bool {
        if let Some(stream) = self.streams.remove(device_id) {
            println!("Stopping and removing stream for device: {}", device_id);
            // Stream will be automatically dropped and stopped here
            drop(stream);
            true
        } else {
            println!("Stream not found for removal: {}", device_id);
            false
        }
    }
}

// Stream management commands for cross-thread communication
pub enum StreamCommand {
    AddInputStream {
        device_id: String,
        device: cpal::Device,
        config: cpal::StreamConfig,
        audio_buffer: Arc<Mutex<Vec<f32>>>,
        target_sample_rate: u32,
        response_tx: std::sync::mpsc::Sender<Result<()>>,
    },
    RemoveStream {
        device_id: String,
        response_tx: std::sync::mpsc::Sender<bool>,
    },
}

// Global stream manager instance
static STREAM_MANAGER: std::sync::OnceLock<std::sync::mpsc::Sender<StreamCommand>> = std::sync::OnceLock::new();

// Initialize the stream manager thread
fn init_stream_manager() -> std::sync::mpsc::Sender<StreamCommand> {
    let (tx, rx) = std::sync::mpsc::channel::<StreamCommand>();
    
    std::thread::spawn(move || {
        let mut manager = StreamManager::new();
        println!("Stream manager thread started");
        
        while let Ok(command) = rx.recv() {
            match command {
                StreamCommand::AddInputStream {
                    device_id,
                    device,
                    config,
                    audio_buffer,
                    target_sample_rate,
                    response_tx,
                } => {
                    let result = manager.add_input_stream(device_id, device, config, audio_buffer, target_sample_rate);
                    let _ = response_tx.send(result);
                }
                StreamCommand::RemoveStream { device_id, response_tx } => {
                    let result = manager.remove_stream(&device_id);
                    let _ = response_tx.send(result);
                }
            }
        }
        
        println!("Stream manager thread stopped");
    });
    
    tx
}

// Get or initialize the global stream manager
pub fn get_stream_manager() -> &'static std::sync::mpsc::Sender<StreamCommand> {
    STREAM_MANAGER.get_or_init(init_stream_manager)
}

// Helper structure for processing thread
#[derive(Debug)]
pub struct VirtualMixerHandle {
    pub input_streams: Arc<Mutex<HashMap<String, Arc<AudioInputStream>>>>,
    pub output_stream: Arc<Mutex<Option<Arc<AudioOutputStream>>>>,
    #[cfg(target_os = "macos")]
    pub coreaudio_stream: Arc<Mutex<Option<super::coreaudio_stream::CoreAudioOutputStream>>>,
}

impl VirtualMixerHandle {
    /// Get samples from all active input streams with effects processing
    pub async fn collect_input_samples_with_effects(&self, channels: &[AudioChannel]) -> HashMap<String, Vec<f32>> {
        let mut samples = HashMap::new();
        let streams = self.input_streams.lock().await;
        
        // Debug: Log collection attempt
        use std::sync::{LazyLock, Mutex as StdMutex};
        static COLLECTION_COUNT: LazyLock<StdMutex<u64>> = LazyLock::new(|| StdMutex::new(0));
        
        let collection_count = if let Ok(mut count) = COLLECTION_COUNT.lock() {
            *count += 1;
            *count
        } else {
            0
        };
        
        let num_streams = streams.len();
        let num_channels = channels.len();
        
        for (device_id, stream) in streams.iter() {
            // Find the channel configuration for this stream
            if let Some(channel) = channels.iter().find(|ch| {
                ch.input_device_id.as_ref() == Some(device_id)
            }) {
                let stream_samples = stream.process_with_effects(channel);
                if !stream_samples.is_empty() {
                    let peak = stream_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                    let rms = (stream_samples.iter().map(|&s| s * s).sum::<f32>() / stream_samples.len() as f32).sqrt();
                    
                    if collection_count % 200 == 0 || (peak > 0.01 && collection_count % 50 == 0) {
                        println!("🎯 COLLECT WITH EFFECTS [{}]: {} samples collected, peak: {:.4}, rms: {:.4}, channel: {}", 
                            device_id, stream_samples.len(), peak, rms, channel.name);
                    }
                    samples.insert(device_id.clone(), stream_samples);
                }
            } else {
                // No channel config found, use raw samples
                let stream_samples = stream.get_samples();
                if !stream_samples.is_empty() {
                    let peak = stream_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                    let rms = (stream_samples.iter().map(|&s| s * s).sum::<f32>() / stream_samples.len() as f32).sqrt();
                    
                    if collection_count % 200 == 0 || (peak > 0.01 && collection_count % 50 == 0) {
                        println!("🎯 COLLECT RAW [{}]: {} samples collected, peak: {:.4}, rms: {:.4} (no channel config)", 
                            device_id, stream_samples.len(), peak, rms);
                    }
                    samples.insert(device_id.clone(), stream_samples);
                }
            }
        }
        
        // Debug: Log collection summary
        if collection_count % 1000 == 0 {
            println!("📈 COLLECTION SUMMARY: {} streams available, {} channels configured, {} samples collected", 
                num_streams, num_channels, samples.len());
            
            if samples.is_empty() && num_streams > 0 {
                println!("⚠️  NO SAMPLES COLLECTED despite {} active streams - potential issue!", num_streams);
                
                // Debug each stream buffer state
                for (device_id, stream) in streams.iter() {
                    if let Ok(buffer_guard) = stream.audio_buffer.try_lock() {
                        println!("   Stream [{}]: buffer has {} samples", device_id, buffer_guard.len());
                    } else {
                        println!("   Stream [{}]: buffer locked", device_id);
                    }
                }
            }
        }
        
        samples
    }

    /// Get samples from all active input streams (without effects - for compatibility)
    pub async fn collect_input_samples(&self) -> HashMap<String, Vec<f32>> {
        let mut samples = HashMap::new();
        let streams = self.input_streams.lock().await;
        
        for (device_id, stream) in streams.iter() {
            let stream_samples = stream.get_samples();
            if !stream_samples.is_empty() {
                samples.insert(device_id.clone(), stream_samples);
            }
        }
        
        samples
    }

    /// Send mixed samples to the output stream
    pub async fn send_to_output(&self, samples: &[f32]) {
        // Send to regular output stream
        if let Some(output) = self.output_stream.lock().await.as_ref() {
            output.send_samples(samples);
        }
        
        // Send to CoreAudio stream if available
        #[cfg(target_os = "macos")]
        {
            if let Some(ref coreaudio_stream) = *self.coreaudio_stream.lock().await {
                let _ = coreaudio_stream.send_audio(samples);
            }
        }
    }
}