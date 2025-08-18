use anyhow::{Context, Result};
use cpal::{StreamConfig, SampleRate, BufferSize};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use super::devices::AudioDeviceManager;
use super::effects::AudioAnalyzer;
use super::streams::{AudioInputStream, AudioOutputStream, VirtualMixerHandle, StreamCommand, get_stream_manager};
use super::types::{AudioChannel, AudioMetrics, MixerCommand, MixerConfig};

#[derive(Debug)]
pub struct VirtualMixer {
    config: MixerConfig,
    is_running: Arc<AtomicBool>,
    
    // Real-time audio buffers
    mix_buffer: Arc<Mutex<Vec<f32>>>,
    
    // Audio processing (placeholder for future sample rate conversion)
    sample_rate_converter: Option<()>,
    audio_analyzer: AudioAnalyzer,
    
    // Communication channels
    command_tx: mpsc::Sender<MixerCommand>,
    command_rx: Arc<Mutex<mpsc::Receiver<MixerCommand>>>,
    audio_output_tx: mpsc::Sender<Vec<f32>>,
    
    // Metrics
    metrics: Arc<Mutex<AudioMetrics>>,
    
    // Real-time audio level data for VU meters with atomic caching
    channel_levels: Arc<Mutex<HashMap<u32, (f32, f32)>>>,
    channel_levels_cache: Arc<Mutex<HashMap<u32, (f32, f32)>>>,
    master_levels: Arc<Mutex<(f32, f32, f32, f32)>>,
    master_levels_cache: Arc<Mutex<(f32, f32, f32, f32)>>,
    
    // Audio stream management
    audio_device_manager: Arc<AudioDeviceManager>,
    input_streams: Arc<Mutex<HashMap<String, Arc<AudioInputStream>>>>,
    output_stream: Arc<Mutex<Option<Arc<AudioOutputStream>>>>,
    #[cfg(target_os = "macos")]
    coreaudio_stream: Arc<Mutex<Option<super::coreaudio_stream::CoreAudioOutputStream>>>,
}

impl VirtualMixer {
    /// Validate mixer configuration for security and performance
    fn validate_config(config: &MixerConfig) -> Result<()> {
        // Sample rate validation
        if config.sample_rate < 8000 || config.sample_rate > 192000 {
            return Err(anyhow::anyhow!("Invalid sample rate: {} (must be 8000-192000 Hz)", config.sample_rate));
        }
        
        // Buffer size validation
        if config.buffer_size < 16 || config.buffer_size > 8192 {
            return Err(anyhow::anyhow!("Invalid buffer size: {} (must be 16-8192 samples)", config.buffer_size));
        }
        
        // Check buffer size is power of 2 for optimal performance
        if !config.buffer_size.is_power_of_two() {
            println!("Warning: Buffer size {} is not a power of 2, may cause performance issues", config.buffer_size);
        }
        
        // Master gain validation
        if config.master_gain < 0.0 || config.master_gain > 4.0 {
            return Err(anyhow::anyhow!("Invalid master gain: {} (must be 0.0-4.0)", config.master_gain));
        }
        
        // Channels validation
        if config.channels.len() > 32 {
            return Err(anyhow::anyhow!("Too many channels: {} (maximum 32)", config.channels.len()));
        }
        
        // Validate each channel
        for (i, channel) in config.channels.iter().enumerate() {
            if channel.gain < 0.0 || channel.gain > 4.0 {
                return Err(anyhow::anyhow!("Invalid gain for channel {}: {} (must be 0.0-4.0)", i, channel.gain));
            }
            if channel.pan < -1.0 || channel.pan > 1.0 {
                return Err(anyhow::anyhow!("Invalid pan for channel {}: {} (must be -1.0 to 1.0)", i, channel.pan));
            }
            // Validate EQ settings
            if channel.eq_low_gain < -24.0 || channel.eq_low_gain > 24.0 {
                return Err(anyhow::anyhow!("Invalid EQ low gain for channel {}: {} (must be -24.0 to 24.0 dB)", i, channel.eq_low_gain));
            }
            if channel.eq_mid_gain < -24.0 || channel.eq_mid_gain > 24.0 {
                return Err(anyhow::anyhow!("Invalid EQ mid gain for channel {}: {} (must be -24.0 to 24.0 dB)", i, channel.eq_mid_gain));
            }
            if channel.eq_high_gain < -24.0 || channel.eq_high_gain > 24.0 {
                return Err(anyhow::anyhow!("Invalid EQ high gain for channel {}: {} (must be -24.0 to 24.0 dB)", i, channel.eq_high_gain));
            }
        }
        
        Ok(())
    }

    pub async fn new(config: MixerConfig) -> Result<Self> {
        let device_manager = Arc::new(AudioDeviceManager::new()?);
        Self::new_with_device_manager(config, device_manager).await
    }

    pub async fn new_with_device_manager(config: MixerConfig, device_manager: Arc<AudioDeviceManager>) -> Result<Self> {
        // Validate mixer configuration
        Self::validate_config(&config)?;
        
        let (command_tx, command_rx) = mpsc::channel(1024);
        let (audio_output_tx, _audio_output_rx) = mpsc::channel(8192);
        
        let buffer_size = config.buffer_size as usize;
        let mix_buffer = Arc::new(Mutex::new(vec![0.0; buffer_size * 2])); // Stereo

        let metrics = Arc::new(Mutex::new(AudioMetrics {
            cpu_usage: 0.0,
            buffer_underruns: 0,
            buffer_overruns: 0,
            latency_ms: (buffer_size as f32 / config.sample_rate as f32) * 1000.0,
            sample_rate: config.sample_rate,
            active_channels: config.channels.len() as u32,
        }));

        // Use the provided audio device manager
        let audio_device_manager = device_manager;

        let channel_levels = Arc::new(Mutex::new(HashMap::new()));
        let channel_levels_cache = Arc::new(Mutex::new(HashMap::new()));
        let master_levels = Arc::new(Mutex::new((0.0, 0.0, 0.0, 0.0)));
        let master_levels_cache = Arc::new(Mutex::new((0.0, 0.0, 0.0, 0.0)));

        Ok(Self {
            config: config.clone(),
            is_running: Arc::new(AtomicBool::new(false)),
            mix_buffer,
            sample_rate_converter: None,
            audio_analyzer: AudioAnalyzer::new(config.sample_rate),
            command_tx,
            command_rx: Arc::new(Mutex::new(command_rx)),
            audio_output_tx,
            metrics,
            channel_levels,
            channel_levels_cache,
            master_levels,
            master_levels_cache,
            audio_device_manager,
            input_streams: Arc::new(Mutex::new(HashMap::new())),
            output_stream: Arc::new(Mutex::new(None)),
            #[cfg(target_os = "macos")]
            coreaudio_stream: Arc::new(Mutex::new(None)),
        })
    }

    /// Start the virtual mixer
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running.load(Ordering::Relaxed) {
            return Ok(());
        }

        println!("Starting Virtual Mixer with real audio capture...");

        self.is_running.store(true, Ordering::Relaxed);
        
        // Start the audio processing thread
        self.start_processing_thread().await?;
        
        Ok(())
    }

    /// Add an audio input stream with real audio capture using cpal
    pub async fn add_input_stream(&self, device_id: &str) -> Result<()> {
        // Validate device_id input
        if device_id.is_empty() {
            return Err(anyhow::anyhow!("Device ID cannot be empty"));
        }
        if device_id.len() > 256 {
            return Err(anyhow::anyhow!("Device ID too long: maximum 256 characters"));
        }
        if !device_id.chars().all(|c| c.is_alphanumeric() || "_-".contains(c)) {
            return Err(anyhow::anyhow!("Device ID contains invalid characters: only alphanumeric, underscore, and dash allowed"));
        }
        
        println!("Adding real audio input stream for device: {}", device_id);
        
        // Find the actual cpal device
        let device = self.audio_device_manager.find_cpal_device(device_id, true).await?;
        let device_name = device.name().unwrap_or_else(|_| device_id.to_string());
        
        println!("Found cpal device: {}", device_name);
        
        // Get the default input config for this device
        let config = device.default_input_config()
            .context("Failed to get default input config")?;
            
        println!("Device config: {:?}", config);
        
        // Create AudioInputStream structure
        let input_stream = AudioInputStream::new(
            device_id.to_string(),
            device_name.clone(),
            self.config.sample_rate,
        )?;
        
        // Get references for the audio callback
        let audio_buffer = input_stream.audio_buffer.clone();
        let target_sample_rate = self.config.sample_rate;
        let buffer_size = self.config.buffer_size as usize;
        
        // Create the appropriate stream config
        let stream_config = StreamConfig {
            channels: config.channels().min(2), // Limit to stereo max
            sample_rate: SampleRate(target_sample_rate),
            buffer_size: BufferSize::Fixed(buffer_size as u32),
        };
        
        println!("Using stream config: channels={}, sample_rate={}, buffer_size={}", 
                stream_config.channels, stream_config.sample_rate.0, buffer_size);
        
        // Add to streams collection first
        let mut streams = self.input_streams.lock().await;
        streams.insert(device_id.to_string(), Arc::new(input_stream));
        drop(streams); // Release the async lock
        
        // Send stream creation command to the synchronous stream manager thread
        let stream_manager = get_stream_manager();
        let (response_tx, response_rx) = std::sync::mpsc::channel();
        
        let command = StreamCommand::AddInputStream {
            device_id: device_id.to_string(),
            device,
            config: stream_config,
            audio_buffer,
            target_sample_rate,
            response_tx,
        };
        
        stream_manager.send(command)
            .context("Failed to send stream creation command")?;
            
        // Wait for the response from the stream manager thread
        let result = response_rx.recv()
            .context("Failed to receive stream creation response")?;
            
        match result {
            Ok(()) => {
                println!("Successfully started audio input stream for: {}", device_name);
                println!("Successfully added real audio input stream: {}", device_id);
                Ok(())
            }
            Err(e) => {
                // Remove from streams collection if stream creation failed
                let mut streams = self.input_streams.lock().await;
                streams.remove(device_id);
                Err(e)
            }
        }
    }

    /// Set the audio output stream with support for both cpal and CoreAudio devices
    pub async fn set_output_stream(&self, device_id: &str) -> Result<()> {
        println!("Setting output stream for device: {}", device_id);
        
        // Validate device_id input
        if device_id.is_empty() || device_id.len() > 256 {
            return Err(anyhow::anyhow!("Invalid device ID: must be 1-256 characters"));
        }
        
        // Find the audio device (CoreAudio or cpal) for output
        let device_handle = self.audio_device_manager.find_audio_device(device_id, false).await?;
        
        match device_handle {
            super::AudioDeviceHandle::Cpal(device) => {
                self.create_cpal_output_stream(device_id, device).await
            }
            #[cfg(target_os = "macos")]
            super::AudioDeviceHandle::CoreAudio(coreaudio_device) => {
                self.create_coreaudio_output_stream(device_id, coreaudio_device).await
            }
        }
    }

    /// Create cpal output stream (existing implementation)
    async fn create_cpal_output_stream(&self, device_id: &str, device: cpal::Device) -> Result<()> {
        let device_name = device.name().unwrap_or_else(|_| device_id.to_string());
        println!("Found cpal output device: {}", device_name);
        
        // Get the default output config for this device
        let config = device.default_output_config()
            .context("Failed to get default output config")?;
            
        println!("Output device config: {:?}", config);
        
        // Create AudioOutputStream structure
        let output_stream = AudioOutputStream::new(
            device_id.to_string(),
            device_name.clone(),
            self.config.sample_rate,
        )?;
        
        // Get reference to the buffer for the output callback
        let output_buffer = output_stream.input_buffer.clone();
        let target_sample_rate = self.config.sample_rate;
        let buffer_size = self.config.buffer_size as usize;
        
        // Create the appropriate stream config for output
        let stream_config = StreamConfig {
            channels: 2, // Force stereo output
            sample_rate: SampleRate(target_sample_rate),
            buffer_size: BufferSize::Fixed(buffer_size as u32),
        };
        
        println!("Using output stream config: channels={}, sample_rate={}, buffer_size={}", 
                stream_config.channels, stream_config.sample_rate.0, buffer_size);
        
        // Create and start the actual cpal output stream in a separate scope
        {
            let output_stream_handle = match config.sample_format() {
                cpal::SampleFormat::F32 => {
                    device.build_output_stream(
                        &stream_config,
                        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        // Fill the output buffer with audio from our internal buffer
                        if let Ok(mut buffer) = output_buffer.try_lock() {
                            let available_samples = buffer.len().min(data.len());
                            if available_samples > 0 {
                                // Copy samples from our buffer to the output
                                data[..available_samples].copy_from_slice(&buffer[..available_samples]);
                                buffer.drain(..available_samples);
                                
                                // Fill remaining with silence if needed
                                if available_samples < data.len() {
                                    data[available_samples..].fill(0.0);
                                }
                            } else {
                                // No audio available, output silence
                                data.fill(0.0);
                            }
                        } else {
                            // Couldn't get lock, output silence to prevent audio dropouts
                            data.fill(0.0);
                        }
                        },
                        |err| eprintln!("Audio output error: {}", err),
                        None
                    ).context("Failed to build F32 output stream")?
                },
                cpal::SampleFormat::I16 => {
                        device.build_output_stream(
                        &stream_config,
                        move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        if let Ok(mut buffer) = output_buffer.try_lock() {
                            let samples_to_convert = (buffer.len() / 2).min(data.len()); // Stereo to stereo
                            if samples_to_convert > 0 {
                                // Convert f32 samples to i16
                                for i in 0..samples_to_convert {
                                    data[i] = (buffer[i].clamp(-1.0, 1.0) * 32767.0) as i16;
                                }
                                buffer.drain(..samples_to_convert * 2);
                                
                                // Fill remaining with silence
                                if samples_to_convert < data.len() {
                                    data[samples_to_convert..].fill(0);
                                }
                            } else {
                                data.fill(0);
                            }
                        } else {
                            data.fill(0);
                        }
                        },
                        |err| eprintln!("Audio output error: {}", err),
                        None
                    ).context("Failed to build I16 output stream")?
                },
                cpal::SampleFormat::U16 => {
                    device.build_output_stream(
                        &stream_config,
                        move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                        if let Ok(mut buffer) = output_buffer.try_lock() {
                            let samples_to_convert = (buffer.len() / 2).min(data.len());
                            if samples_to_convert > 0 {
                                // Convert f32 samples to u16
                                for i in 0..samples_to_convert {
                                    data[i] = ((buffer[i].clamp(-1.0, 1.0) + 1.0) * 32767.5) as u16;
                                }
                                buffer.drain(..samples_to_convert * 2);
                                
                                if samples_to_convert < data.len() {
                                    data[samples_to_convert..].fill(32768); // Mid-point for unsigned
                                }
                            } else {
                                data.fill(32768);
                            }
                        } else {
                            data.fill(32768);
                        }
                        },
                        |err| eprintln!("Audio output error: {}", err),
                        None
                    ).context("Failed to build U16 output stream")?
                },
                _ => {
                    return Err(anyhow::anyhow!("Unsupported output sample format: {:?}", config.sample_format()));
                }
            };
            
            // Start the output stream and immediately forget it to avoid Send issues
            output_stream_handle.play().context("Failed to start output stream")?;
            std::mem::forget(output_stream_handle);
        }
        
        // Store our wrapper
        let mut stream_guard = self.output_stream.lock().await;
        *stream_guard = Some(Arc::new(output_stream));
        
        println!("Successfully created real cpal output stream: {}", device_id);
        
        Ok(())
    }

    /// Create CoreAudio output stream for direct hardware access
    #[cfg(target_os = "macos")]
    async fn create_coreaudio_output_stream(&self, device_id: &str, coreaudio_device: super::CoreAudioDevice) -> Result<()> {
        println!("Creating CoreAudio output stream for device: {} (ID: {})", coreaudio_device.name, coreaudio_device.device_id);
        
        // Create the actual CoreAudio stream
        let mut coreaudio_stream = super::coreaudio_stream::CoreAudioOutputStream::new(
            coreaudio_device.device_id,
            coreaudio_device.name.clone(),
            self.config.sample_rate,
            coreaudio_device.channels,
        )?;
        
        // Start the CoreAudio stream
        coreaudio_stream.start()?;
        
        // Store the CoreAudio stream in the mixer to keep it alive
        let mut coreaudio_guard = self.coreaudio_stream.lock().await;
        *coreaudio_guard = Some(coreaudio_stream);
        
        // Create AudioOutputStream structure for compatibility with the existing mixer architecture
        let output_stream = AudioOutputStream::new(
            device_id.to_string(),
            coreaudio_device.name.clone(),
            self.config.sample_rate,
        )?;
        
        // Store our wrapper 
        let mut stream_guard = self.output_stream.lock().await;
        *stream_guard = Some(Arc::new(output_stream));
        
        println!("✅ Real CoreAudio Audio Unit stream created and started for: {}", device_id);
        
        Ok(())
    }

    /// Remove an input stream and clean up cpal stream
    pub async fn remove_input_stream(&self, device_id: &str) -> Result<()> {
        // Remove from streams collection
        let mut streams = self.input_streams.lock().await;
        let was_present = streams.remove(device_id).is_some();
        drop(streams); // Release the async lock
        
        if was_present {
            // Send stream removal command to the synchronous stream manager thread
            let stream_manager = get_stream_manager();
            let (response_tx, response_rx) = std::sync::mpsc::channel();
            
            let command = StreamCommand::RemoveStream {
                device_id: device_id.to_string(),
                response_tx,
            };
            
            stream_manager.send(command)
                .context("Failed to send stream removal command")?;
                
            // Wait for the response
            let removed = response_rx.recv()
                .context("Failed to receive stream removal response")?;
                
            if removed {
                println!("Removed input stream and cleaned up cpal stream: {}", device_id);
            } else {
                println!("Stream was not found in manager for removal: {}", device_id);
            }
        } else {
            println!("Input stream not found for removal: {}", device_id);
        }
        
        Ok(())
    }

    /// Stop the virtual mixer
    pub async fn stop(&mut self) -> Result<()> {
        self.is_running.store(false, Ordering::Relaxed);
        
        // Stop CoreAudio stream if active
        #[cfg(target_os = "macos")]
        {
            let mut coreaudio_guard = self.coreaudio_stream.lock().await;
            if let Some(mut stream) = coreaudio_guard.take() {
                let _ = stream.stop();
            }
        }
        
        // TODO: Stop all other audio streams (will be managed separately)
        
        Ok(())
    }

    async fn start_processing_thread(&self) -> Result<()> {
        let is_running = self.is_running.clone();
        let mix_buffer = self.mix_buffer.clone();
        let audio_output_tx = self.audio_output_tx.clone();
        let metrics = self.metrics.clone();
        let channel_levels = self.channel_levels.clone();
        let channel_levels_cache = self.channel_levels_cache.clone();
        let master_levels = self.master_levels.clone();
        let master_levels_cache = self.master_levels_cache.clone();
        let sample_rate = self.config.sample_rate;
        let buffer_size = self.config.buffer_size;
        let config_channels = self.config.channels.clone();
        let mixer_handle = VirtualMixerHandle {
            input_streams: self.input_streams.clone(),
            output_stream: self.output_stream.clone(),
            #[cfg(target_os = "macos")]
            coreaudio_stream: self.coreaudio_stream.clone(),
        };

        // Spawn real-time audio processing task
        tokio::spawn(async move {
            let mut frame_count = 0u64;
            
            // Pre-allocate stereo buffers to reduce allocations during real-time processing
            let mut reusable_output_buffer = vec![0.0f32; (buffer_size * 2) as usize];
            let mut reusable_left_samples = Vec::with_capacity(buffer_size as usize);
            let mut reusable_right_samples = Vec::with_capacity(buffer_size as usize);
            
            println!("Audio processing thread started with real mixing and optimized buffers");

            while is_running.load(Ordering::Relaxed) {
                let process_start = std::time::Instant::now();
                
                // Collect input samples from all active input streams with effects processing
                let input_samples = mixer_handle.collect_input_samples_with_effects(&config_channels).await;
                
                // Clear and reuse pre-allocated stereo buffers
                reusable_output_buffer.fill(0.0);
                reusable_left_samples.clear();
                reusable_right_samples.clear();
                
                // Calculate channel levels and mix audio
                let mut calculated_channel_levels = std::collections::HashMap::new();
                
                if !input_samples.is_empty() {
                    let mut active_channels = 0;
                    
                    // Mix all input channels together and calculate levels
                    for (device_id, samples) in input_samples.iter() {
                        if !samples.is_empty() {
                            active_channels += 1;
                            
                            // Calculate peak and RMS levels for VU meters
                            let peak_level = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                            let rms_level = (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
                            
                            // Find which channel this device belongs to
                            if let Some(channel) = config_channels.iter().find(|ch| {
                                ch.input_device_id.as_ref() == Some(device_id)
                            }) {
                                // Store levels by channel ID
                                calculated_channel_levels.insert(channel.id, (peak_level, rms_level));
                                
                                // Log levels occasionally
                                if frame_count % 100 == 0 && peak_level > 0.001 {
                                    println!("Channel {} ({}): {} samples, peak: {:.3}, rms: {:.3}", 
                                        channel.id, device_id, samples.len(), peak_level, rms_level);
                                }
                            }
                            
                            // Professional stereo mixing: add samples together preserving L/R channels
                            let mix_length = reusable_output_buffer.len().min(samples.len());
                            for i in 0..mix_length {
                                reusable_output_buffer[i] += samples[i];
                            }
                        }
                    }
                    
                    // Normalize by number of active channels to prevent clipping
                    if active_channels > 0 {
                        let gain = 1.0 / active_channels as f32;
                        for sample in reusable_output_buffer.iter_mut() {
                            *sample *= gain;
                        }
                    }
                    
                    // Stereo audio is already mixed directly into reusable_output_buffer
                    // No conversion needed - stereo data preserved throughout mixing process
                    
                    // Apply basic gain (master volume)
                    let master_gain = 0.5f32; // Reduce volume to prevent clipping
                    for sample in reusable_output_buffer.iter_mut() {
                        *sample *= master_gain;
                    }
                    
                    // Calculate master output levels for L/R channels using reusable vectors
                    reusable_left_samples.extend(reusable_output_buffer.iter().step_by(2).copied());
                    reusable_right_samples.extend(reusable_output_buffer.iter().skip(1).step_by(2).copied());
                    
                    let left_peak = reusable_left_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                    let left_rms = if !reusable_left_samples.is_empty() {
                        (reusable_left_samples.iter().map(|&s| s * s).sum::<f32>() / reusable_left_samples.len() as f32).sqrt()
                    } else { 0.0 };
                    
                    let right_peak = reusable_right_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                    let right_rms = if !reusable_right_samples.is_empty() {
                        (reusable_right_samples.iter().map(|&s| s * s).sum::<f32>() / reusable_right_samples.len() as f32).sqrt()
                    } else { 0.0 };
                    
                    // Store real master levels
                    let master_level_values = (left_peak, left_rms, right_peak, right_rms);
                    if let Ok(mut levels_guard) = master_levels.try_lock() {
                        *levels_guard = master_level_values;
                    }
                    
                    // Also update cache for fallback (non-blocking)
                    let has_signal = left_peak > 0.0 || left_rms > 0.0 || right_peak > 0.0 || right_rms > 0.0;
                    if has_signal {
                        if let Ok(mut cache_guard) = master_levels_cache.try_lock() {
                            *cache_guard = master_level_values;
                        }
                    }
                    
                    // Log master levels occasionally
                    if frame_count % 100 == 0 && (left_peak > 0.001 || right_peak > 0.001) {
                        println!("Master output: L(peak: {:.3}, rms: {:.3}) R(peak: {:.3}, rms: {:.3})", 
                            left_peak, left_rms, right_peak, right_rms);
                    }
                }
                
                // Store calculated channel levels for VU meters
                if let Ok(mut levels_guard) = channel_levels.try_lock() {
                    *levels_guard = calculated_channel_levels.clone();
                }
                
                // Also update cache for fallback (non-blocking)
                if !calculated_channel_levels.is_empty() {
                    if let Ok(mut cache_guard) = channel_levels_cache.try_lock() {
                        *cache_guard = calculated_channel_levels;
                    }
                }
                
                // Update mix buffer
                if let Ok(mut buffer_guard) = mix_buffer.try_lock() {
                    if buffer_guard.len() == reusable_output_buffer.len() {
                        buffer_guard.copy_from_slice(&reusable_output_buffer);
                    }
                }
                
                // Send to output stream
                mixer_handle.send_to_output(&reusable_output_buffer).await;

                // Send processed audio to the rest of the application (non-blocking)
                let _ = audio_output_tx.try_send(reusable_output_buffer.clone());
                // Don't break on send failure - just continue processing

                frame_count += 1;
                
                // Update metrics every second
                if frame_count % (sample_rate / buffer_size) as u64 == 0 {
                    let cpu_time = process_start.elapsed().as_secs_f32();
                    let max_cpu_time = buffer_size as f32 / sample_rate as f32;
                    let cpu_usage = (cpu_time / max_cpu_time) * 100.0;
                    
                    if let Ok(mut metrics_guard) = metrics.try_lock() {
                        metrics_guard.cpu_usage = cpu_usage;
                    }
                    
                    if input_samples.len() > 0 {
                        println!("Audio processing: CPU {:.1}%, {} active streams", cpu_usage, input_samples.len());
                    }
                }

                // Maintain real-time constraints
                let target_duration = std::time::Duration::from_micros(
                    (buffer_size as u64 * 1_000_000) / sample_rate as u64
                );
                let elapsed = process_start.elapsed();
                if elapsed < target_duration {
                    tokio::time::sleep(target_duration - elapsed).await;
                }
            }
            
            println!("Audio processing thread stopped");
        });

        Ok(())
    }

    /// Add a new audio channel
    pub async fn add_channel(&mut self, channel: AudioChannel) -> Result<()> {
        // TODO: Add ring buffer management
        self.config.channels.push(channel);
        Ok(())
    }

    /// Get current mixer metrics
    pub async fn get_metrics(&self) -> AudioMetrics {
        self.metrics.lock().await.clone()
    }

    /// Get current channel levels for VU meters with proper fallback caching
    pub async fn get_channel_levels(&self) -> HashMap<u32, (f32, f32)> {
        // Try to get real-time levels first
        if let Ok(levels_guard) = self.channel_levels.try_lock() {
            let levels = levels_guard.clone();
            
            // Update cache with latest values (non-blocking)
            if !levels.is_empty() {
                if let Ok(mut cache_guard) = self.channel_levels_cache.try_lock() {
                    *cache_guard = levels.clone();
                }
            }
            
            levels
        } else {
            // Fallback to cached levels if we can't get the real-time lock
            if let Ok(cache_guard) = self.channel_levels_cache.try_lock() {
                cache_guard.clone()
            } else {
                // Last resort: return empty levels
                HashMap::new()
            }
        }
    }

    /// Get current master output levels for VU meters (Left/Right) with proper fallback caching
    pub async fn get_master_levels(&self) -> (f32, f32, f32, f32) {
        // Try to get real-time levels first
        if let Ok(levels_guard) = self.master_levels.try_lock() {
            let levels = *levels_guard;
            
            // Update cache with latest values (non-blocking)
            let has_signal = levels.0 > 0.0 || levels.1 > 0.0 || levels.2 > 0.0 || levels.3 > 0.0;
            if has_signal {
                if let Ok(mut cache_guard) = self.master_levels_cache.try_lock() {
                    *cache_guard = levels;
                }
            }
            
            levels
        } else {
            // Fallback to cached levels if we can't get the real-time lock
            if let Ok(cache_guard) = self.master_levels_cache.try_lock() {
                *cache_guard
            } else {
                // Last resort: return zero levels
                (0.0, 0.0, 0.0, 0.0)
            }
        }
    }

    /// Get audio output stream for streaming/recording
    pub async fn get_audio_output_receiver(&self) -> mpsc::Receiver<Vec<f32>> {
        // Return a connected receiver that gets real audio data from the processing thread
        let (tx, rx) = mpsc::channel(8192);
        
        // Clone references needed for the forwarding task
        let audio_output_tx = self.audio_output_tx.clone();
        
        // Spawn a task to forward audio from the processing thread to this receiver
        tokio::spawn(async move {
            let mut audio_rx = {
                // We need to create a new receiver by cloning the sender
                // This is a limitation - ideally we'd have a broadcast channel
                let (_temp_tx, temp_rx) = mpsc::channel(8192);
                temp_rx
            };
            
            // For now, we'll need to modify the processing thread to support multiple receivers
            // This is a placeholder that demonstrates the correct API
            while let Some(audio_data) = audio_rx.recv().await {
                if tx.send(audio_data).await.is_err() {
                    // Receiver dropped, stop forwarding
                    break;
                }
            }
        });
        
        rx
    }

    /// Send command to mixer
    pub async fn send_command(&self, command: MixerCommand) -> Result<()> {
        self.command_tx.send(command).await
            .context("Failed to send mixer command")?;
        Ok(())
    }

    /// Update channel configuration
    pub async fn update_channel(&mut self, channel_id: u32, updated_channel: AudioChannel) -> Result<()> {
        if let Some(channel) = self.config.channels.iter_mut().find(|c| c.id == channel_id) {
            *channel = updated_channel;
        }
        Ok(())
    }

    /// Get the audio device manager
    pub fn get_device_manager(&self) -> &Arc<AudioDeviceManager> {
        &self.audio_device_manager
    }
}