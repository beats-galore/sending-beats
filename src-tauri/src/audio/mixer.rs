use anyhow::{Context, Result};
use cpal::{StreamConfig, SampleRate, BufferSize};
use cpal::traits::DeviceTrait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use super::devices::AudioDeviceManager;
use super::effects::AudioAnalyzer;
use super::streams::{AudioInputStream, AudioOutputStream, VirtualMixerHandle, StreamCommand, get_stream_manager};
use super::types::{AudioChannel, AudioMetrics, MixerCommand, MixerConfig};

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
    
    // Real-time audio level data for VU meters
    channel_levels: Arc<Mutex<HashMap<u32, (f32, f32)>>>,
    master_levels: Arc<Mutex<(f32, f32, f32, f32)>>,
    
    // Audio stream management
    audio_device_manager: Arc<AudioDeviceManager>,
    input_streams: Arc<Mutex<HashMap<String, Arc<AudioInputStream>>>>,
    output_stream: Arc<Mutex<Option<Arc<AudioOutputStream>>>>,
}

impl VirtualMixer {
    pub async fn new(config: MixerConfig) -> Result<Self> {
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

        // Initialize audio device manager
        let audio_device_manager = Arc::new(AudioDeviceManager::new()?);

        let channel_levels = Arc::new(Mutex::new(HashMap::new()));
        let master_levels = Arc::new(Mutex::new((0.0, 0.0, 0.0, 0.0)));

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
            master_levels,
            audio_device_manager,
            input_streams: Arc::new(Mutex::new(HashMap::new())),
            output_stream: Arc::new(Mutex::new(None)),
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

    /// Set the audio output stream
    pub async fn set_output_stream(&self, device_id: &str) -> Result<()> {
        println!("Setting output stream for device: {}", device_id);
        
        // Try to find the actual cpal device for output
        let devices = self.audio_device_manager.enumerate_devices().await?;
        let target_device = devices.iter().find(|d| d.id == device_id && d.is_output);
        
        if target_device.is_none() {
            println!("Warning: Output device {} not found, using default", device_id);
        }
        
        // Create a buffer-based output stream (we'll enhance this with real cpal output later)
        let output_stream = AudioOutputStream::new(
            device_id.to_string(),
            device_id.replace("_", " "),
            self.config.sample_rate,
        )?;
        
        println!("Setting up output routing for: {}", device_id);
        
        // For now, let's at least start a simple audio playback thread that reads from the buffer
        let output_buffer = output_stream.input_buffer.clone();
        let sample_rate = self.config.sample_rate;
        
        tokio::spawn(async move {
            println!("Starting audio playback thread for output device");
            loop {
                // Read samples from buffer and "play" them (for now just consume them)
                if let Ok(mut buffer) = output_buffer.try_lock() {
                    if !buffer.is_empty() {
                        let samples_to_play = buffer.len().min(512); // Play in chunks
                        let _played_samples: Vec<_> = buffer.drain(0..samples_to_play).collect();
                        // In a real implementation, these samples would be sent to cpal output stream
                        if samples_to_play > 0 {
                            // println!("Playing {} samples to output device", samples_to_play);
                        }
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await; // ~10ms intervals
            }
        });
        
        let mut stream_guard = self.output_stream.lock().await;
        *stream_guard = Some(Arc::new(output_stream));
        println!("Successfully set output stream with playback thread: {}", device_id);
        
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
        
        // TODO: Stop all audio streams (will be managed separately)
        
        Ok(())
    }

    async fn start_processing_thread(&self) -> Result<()> {
        let is_running = self.is_running.clone();
        let mix_buffer = self.mix_buffer.clone();
        let audio_output_tx = self.audio_output_tx.clone();
        let metrics = self.metrics.clone();
        let channel_levels = self.channel_levels.clone();
        let master_levels = self.master_levels.clone();
        let sample_rate = self.config.sample_rate;
        let buffer_size = self.config.buffer_size;
        let config_channels = self.config.channels.clone();
        let mixer_handle = VirtualMixerHandle {
            input_streams: self.input_streams.clone(),
            output_stream: self.output_stream.clone(),
        };

        // Spawn real-time audio processing task
        tokio::spawn(async move {
            let mut frame_count = 0u64;
            
            println!("Audio processing thread started with real mixing");

            while is_running.load(Ordering::Relaxed) {
                let process_start = std::time::Instant::now();
                
                // Collect input samples from all active input streams with effects processing
                let input_samples = mixer_handle.collect_input_samples_with_effects(&config_channels).await;
                
                // Create the output buffer (stereo)
                let mut output_buffer = vec![0.0f32; (buffer_size * 2) as usize];
                
                // Calculate channel levels and mix audio
                let mut calculated_channel_levels = std::collections::HashMap::new();
                
                if !input_samples.is_empty() {
                    let mut mixed_samples = vec![0.0f32; buffer_size as usize];
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
                            
                            // Simple mixing: add samples together
                            let mix_length = mixed_samples.len().min(samples.len());
                            for i in 0..mix_length {
                                mixed_samples[i] += samples[i];
                            }
                        }
                    }
                    
                    // Normalize by number of active channels to prevent clipping
                    if active_channels > 0 {
                        let gain = 1.0 / active_channels as f32;
                        for sample in mixed_samples.iter_mut() {
                            *sample *= gain;
                        }
                    }
                    
                    // Convert mono mixed samples to stereo output
                    for (i, &sample) in mixed_samples.iter().enumerate() {
                        if i * 2 + 1 < output_buffer.len() {
                            output_buffer[i * 2] = sample;     // Left channel
                            output_buffer[i * 2 + 1] = sample; // Right channel
                        }
                    }
                    
                    // Apply basic gain (master volume)
                    let master_gain = 0.5f32; // Reduce volume to prevent clipping
                    for sample in output_buffer.iter_mut() {
                        *sample *= master_gain;
                    }
                    
                    // Calculate master output levels for L/R channels
                    let left_samples: Vec<f32> = output_buffer.iter().step_by(2).copied().collect();
                    let right_samples: Vec<f32> = output_buffer.iter().skip(1).step_by(2).copied().collect();
                    
                    let left_peak = left_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                    let left_rms = if !left_samples.is_empty() {
                        (left_samples.iter().map(|&s| s * s).sum::<f32>() / left_samples.len() as f32).sqrt()
                    } else { 0.0 };
                    
                    let right_peak = right_samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                    let right_rms = if !right_samples.is_empty() {
                        (right_samples.iter().map(|&s| s * s).sum::<f32>() / right_samples.len() as f32).sqrt()
                    } else { 0.0 };
                    
                    // Store real master levels
                    if let Ok(mut levels_guard) = master_levels.try_lock() {
                        *levels_guard = (left_peak, left_rms, right_peak, right_rms);
                    }
                    
                    // Log master levels occasionally
                    if frame_count % 100 == 0 && (left_peak > 0.001 || right_peak > 0.001) {
                        println!("Master output: L(peak: {:.3}, rms: {:.3}) R(peak: {:.3}, rms: {:.3})", 
                            left_peak, left_rms, right_peak, right_rms);
                    }
                }
                
                // Store calculated channel levels for VU meters
                if let Ok(mut levels_guard) = channel_levels.try_lock() {
                    *levels_guard = calculated_channel_levels;
                }
                
                // Update mix buffer
                if let Ok(mut buffer_guard) = mix_buffer.try_lock() {
                    if buffer_guard.len() == output_buffer.len() {
                        buffer_guard.copy_from_slice(&output_buffer);
                    }
                }
                
                // Send to output stream
                mixer_handle.send_to_output(&output_buffer).await;

                // Send processed audio to the rest of the application (non-blocking)
                let _ = audio_output_tx.try_send(output_buffer.clone());
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

    /// Get current channel levels for VU meters
    pub async fn get_channel_levels(&self) -> HashMap<u32, (f32, f32)> {
        // Return real audio levels from processing thread
        if let Ok(levels_guard) = self.channel_levels.try_lock() {
            levels_guard.clone()
        } else {
            // Fallback to empty levels if we can't get the lock
            HashMap::new()
        }
    }

    /// Get current master output levels for VU meters (Left/Right)
    pub async fn get_master_levels(&self) -> (f32, f32, f32, f32) {
        // Return real master audio levels from processing thread
        if let Ok(levels_guard) = self.master_levels.try_lock() {
            *levels_guard
        } else {
            // Fallback to zero levels if we can't get the lock
            (0.0, 0.0, 0.0, 0.0)
        }
    }

    /// Get audio output stream for streaming/recording
    pub async fn get_audio_output_receiver(&self) -> mpsc::Receiver<Vec<f32>> {
        let (_tx, rx) = mpsc::channel(8192);
        // In a real implementation, this would connect to the actual audio output
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