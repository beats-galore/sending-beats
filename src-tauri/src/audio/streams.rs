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
            buffer.clear();
            samples
        } else {
            Vec::new()
        }
    }

    /// Apply effects to input samples and update channel settings
    pub fn process_with_effects(&self, channel: &AudioChannel) -> Vec<f32> {
        if let Ok(mut buffer) = self.audio_buffer.try_lock() {
            let mut samples = buffer.clone();
            buffer.clear();

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
        
        let stream = match device_config.sample_format() {
            SampleFormat::F32 => {
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        let mono_samples: Vec<f32> = if config.channels == 1 {
                            data.to_vec()
                        } else {
                            data.chunks(config.channels as usize)
                                .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
                                .collect()
                        };
                        
                        if let Ok(mut buffer) = audio_buffer.try_lock() {
                            buffer.extend_from_slice(&mono_samples);
                            let max_buffer_size = target_sample_rate as usize * 2;
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
            SampleFormat::I16 => {
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let f32_samples: Vec<f32> = data.iter()
                            .map(|&sample| sample as f32 / 32768.0)
                            .collect();
                            
                        let mono_samples: Vec<f32> = if config.channels == 1 {
                            f32_samples
                        } else {
                            f32_samples.chunks(config.channels as usize)
                                .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
                                .collect()
                        };
                        
                        if let Ok(mut buffer) = audio_buffer.try_lock() {
                            buffer.extend_from_slice(&mono_samples);
                            let max_buffer_size = target_sample_rate as usize * 2;
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
            SampleFormat::U16 => {
                device.build_input_stream(
                    &config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        let f32_samples: Vec<f32> = data.iter()
                            .map(|&sample| (sample as f32 - 32768.0) / 32768.0)
                            .collect();
                            
                        let mono_samples: Vec<f32> = if config.channels == 1 {
                            f32_samples
                        } else {
                            f32_samples.chunks(config.channels as usize)
                                .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
                                .collect()
                        };
                        
                        if let Ok(mut buffer) = audio_buffer.try_lock() {
                            buffer.extend_from_slice(&mono_samples);
                            let max_buffer_size = target_sample_rate as usize * 2;
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
        self.streams.remove(device_id).is_some()
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
}

impl VirtualMixerHandle {
    /// Get samples from all active input streams with effects processing
    pub async fn collect_input_samples_with_effects(&self, channels: &[AudioChannel]) -> HashMap<String, Vec<f32>> {
        let mut samples = HashMap::new();
        let streams = self.input_streams.lock().await;
        
        for (device_id, stream) in streams.iter() {
            // Find the channel configuration for this stream
            if let Some(channel) = channels.iter().find(|ch| {
                ch.input_device_id.as_ref() == Some(device_id)
            }) {
                let stream_samples = stream.process_with_effects(channel);
                if !stream_samples.is_empty() {
                    samples.insert(device_id.clone(), stream_samples);
                }
            } else {
                // No channel config found, use raw samples
                let stream_samples = stream.get_samples();
                if !stream_samples.is_empty() {
                    samples.insert(device_id.clone(), stream_samples);
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
        if let Some(output) = self.output_stream.lock().await.as_ref() {
            output.send_samples(samples);
        }
    }
}