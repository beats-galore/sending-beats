// Comprehensive CoreAudio CPAL replacement integration example
//
// This module demonstrates how to use the complete CoreAudio infrastructure
// as a full replacement for CPAL on macOS. It shows:
// - Device enumeration and selection
// - Stream creation and management
// - Format conversion and sample rate handling
// - Device change monitoring and automatic recovery
// - Error handling and graceful fallbacks

#[cfg(target_os = "macos")]
use anyhow::Result;
#[cfg(target_os = "macos")]
use std::sync::Arc;
#[cfg(target_os = "macos")]
use tokio::sync::Notify;
#[cfg(target_os = "macos")]
use tracing::{error, info, warn};

#[cfg(target_os = "macos")]
use super::{
    CoreAudioDeviceNotifier, CoreAudioManager, CoreAudioStreamConfig, DeviceChangeEvent,
    DeviceChangeListener,
};

/// Example device change listener that handles hot-plug events
#[cfg(target_os = "macos")]
struct ExampleDeviceChangeListener {
    manager: Arc<CoreAudioManager>,
}

#[cfg(target_os = "macos")]
impl DeviceChangeListener for ExampleDeviceChangeListener {
    fn on_device_change(&self, event: DeviceChangeEvent) {
        match event {
            DeviceChangeEvent::DeviceAdded { device_id } => {
                info!("🔌 New audio device connected: {}", device_id);
                // Refresh device list and potentially migrate streams
                let manager = self.manager.clone();
                tokio::spawn(async move {
                    if let Err(e) = manager.refresh_devices().await {
                        error!("❌ Failed to refresh devices after hot-plug: {}", e);
                    }
                });
            }
            DeviceChangeEvent::DeviceRemoved { device_id } => {
                warn!("🔌 Audio device disconnected: {}", device_id);
                // Handle stream migration or graceful shutdown
                // In a real application, you would migrate active streams to other devices
            }
            DeviceChangeEvent::DefaultInputChanged {
                old_device_id,
                new_device_id,
            } => {
                info!(
                    "🎤 Default input device changed: {:?} -> {}",
                    old_device_id, new_device_id
                );
                // Optionally switch active input streams to new default
            }
            DeviceChangeEvent::DefaultOutputChanged {
                old_device_id,
                new_device_id,
            } => {
                info!(
                    "🔊 Default output device changed: {:?} -> {}",
                    old_device_id, new_device_id
                );
                // Optionally switch active output streams to new default
            }
            DeviceChangeEvent::DeviceListRefreshed => {
                info!("🔄 Device list refreshed");
            }
            DeviceChangeEvent::SampleRateChanged {
                device_id,
                new_sample_rate,
            } => {
                info!(
                    "🎵 Device {} sample rate changed to {:.1}Hz",
                    device_id, new_sample_rate
                );
                // Handle sample rate changes in active streams
            }
        }
    }
}

/// Comprehensive example of CoreAudio CPAL replacement usage
#[cfg(target_os = "macos")]
pub struct CoreAudioExample {
    manager: Arc<CoreAudioManager>,
    notifier: Arc<CoreAudioDeviceNotifier>,
    active_streams: Vec<String>,
}

#[cfg(target_os = "macos")]
impl CoreAudioExample {
    /// Initialize the CoreAudio system
    pub async fn new() -> Result<Self> {
        info!("🚀 Initializing CoreAudio CPAL replacement system");

        // Create the comprehensive CoreAudio manager
        let manager = Arc::new(CoreAudioManager::new());
        manager.initialize().await?;

        // Create device change notifier
        let notifier = Arc::new(CoreAudioDeviceNotifier::new());

        // Add device change listener
        let listener = Arc::new(ExampleDeviceChangeListener {
            manager: manager.clone(),
        });
        notifier.add_listener(listener).await;

        // Start device monitoring
        notifier.start_monitoring().await?;

        Ok(Self {
            manager,
            notifier,
            active_streams: Vec::new(),
        })
    }

    /// Demonstrate device enumeration
    pub async fn demonstrate_device_enumeration(&self) -> Result<()> {
        info!("📱 Demonstrating CoreAudio device enumeration");

        // Get input devices
        let input_devices = self.manager.get_input_devices().await?;
        info!("🎤 Found {} input devices:", input_devices.len());
        for device in &input_devices {
            info!(
                "  - {} (ID: {}, {} channels, {:.1}kHz, default: {})",
                device.name,
                device.device_id,
                device.input_channels,
                device.current_sample_rate / 1000.0,
                device.is_default
            );
        }

        // Get output devices
        let output_devices = self.manager.get_output_devices().await?;
        info!("🔊 Found {} output devices:", output_devices.len());
        for device in &output_devices {
            info!(
                "  - {} (ID: {}, {} channels, {:.1}kHz, default: {})",
                device.name,
                device.device_id,
                device.output_channels,
                device.current_sample_rate / 1000.0,
                device.is_default
            );
        }

        // Get default devices
        if let Ok(Some(default_input)) = self.manager.get_default_input_device().await {
            info!("🎤 Default input: {}", default_input.name);
        }
        if let Ok(Some(default_output)) = self.manager.get_default_output_device().await {
            info!("🔊 Default output: {}", default_output.name);
        }

        Ok(())
    }

    /// Demonstrate stream creation with format conversion
    pub async fn demonstrate_stream_creation(&mut self) -> Result<()> {
        info!("🎵 Demonstrating CoreAudio stream creation");

        // Get available devices
        let input_devices = self.manager.get_input_devices().await?;
        let output_devices = self.manager.get_output_devices().await?;

        if input_devices.is_empty() || output_devices.is_empty() {
            warn!("⚠️ No input or output devices available for stream demonstration");
            return Ok(());
        }

        // Select first available input and output devices
        let input_device = &input_devices[0];
        let output_device = &output_devices[0];

        info!(
            "🎤 Creating input stream for: {} ({}Hz)",
            input_device.name, input_device.current_sample_rate
        );

        info!(
            "🔊 Creating output stream for: {} ({}Hz)",
            output_device.name, output_device.current_sample_rate
        );

        // Create RTRB ring buffer for input stream
        let buffer_size = 4096; // 4K samples buffer
        let (producer, consumer) = rtrb::RingBuffer::<f32>::new(buffer_size);

        // Create SPMC queue for output stream  
        let (reader, _writer) = spmcq::ring_buffer(buffer_size);

        // Create input stream configuration
        let input_config = CoreAudioStreamConfig {
            sample_rate: input_device.current_sample_rate,
            channels: input_device.input_channels,
            buffer_size: 512, // Small buffer for low latency
            is_input: true,
        };

        // Create output stream configuration (potentially different sample rate)
        let output_config = CoreAudioStreamConfig {
            sample_rate: output_device.current_sample_rate, // Might be different from input
            channels: output_device.output_channels,
            buffer_size: 512,
            is_input: false,
        };

        // Create notification system
        let input_notifier = Arc::new(Notify::new());

        // Create input stream
        match self
            .manager
            .create_input_stream(
                input_device.device_id,
                input_config,
                producer,
                input_notifier,
            )
            .await
        {
            Ok(input_stream_id) => {
                info!("✅ Created input stream: {}", input_stream_id);
                self.active_streams.push(input_stream_id);
            }
            Err(e) => {
                error!("❌ Failed to create input stream: {}", e);
            }
        }

        // Create output stream
        match self
            .manager
            .create_output_stream(output_device.device_id, output_config, reader)
            .await
        {
            Ok(output_stream_id) => {
                info!("✅ Created output stream: {}", output_stream_id);
                self.active_streams.push(output_stream_id);
            }
            Err(e) => {
                error!("❌ Failed to create output stream: {}", e);
            }
        }

        // Show active stream statistics
        let (input_count, output_count) = self.manager.get_stream_stats().await;
        info!(
            "📊 Active streams: {} inputs, {} outputs",
            input_count, output_count
        );

        Ok(())
    }

    /// Demonstrate format conversion capabilities
    pub async fn demonstrate_format_conversion(&self) -> Result<()> {
        info!("🔄 Demonstrating format conversion capabilities");

        // Example of sample rate conversion
        use super::CoreAudioSampleRateConverter;
        let mut src = CoreAudioSampleRateConverter::new(44100.0, 48000.0);

        // Generate test audio at 44.1kHz
        let input_samples: Vec<f32> = (0..1024)
            .map(|i| (i as f32 * 440.0 * 2.0 * std::f32::consts::PI / 44100.0).sin() * 0.1)
            .collect();

        // Convert to 48kHz
        let output_samples = src.convert(&input_samples, 1114); // Expected output size
        info!(
            "🎵 Sample rate conversion: {} samples (44.1kHz) -> {} samples (48kHz)",
            input_samples.len(),
            output_samples.len()
        );

        // Example of channel conversion
        use super::CoreAudioChannelConverter;

        // Convert mono to stereo
        let mono_samples = vec![0.5, 0.3, 0.1, -0.2];
        let stereo_samples = CoreAudioChannelConverter::mono_to_stereo(&mono_samples);
        info!(
            "🔄 Channel conversion: {} mono -> {} stereo",
            mono_samples.len(),
            stereo_samples.len()
        );

        // Example of format conversion
        use super::CoreAudioFormatConverter;

        // Convert i16 to f32
        let i16_samples = vec![16384, -16384, 8192, -8192];
        let f32_samples = CoreAudioFormatConverter::i16_to_f32(&i16_samples);
        info!(
            "🎚️ Format conversion: {} i16 samples -> {} f32 samples",
            i16_samples.len(),
            f32_samples.len()
        );

        Ok(())
    }

    /// Demonstrate error handling and recovery
    pub async fn demonstrate_error_handling(&self) -> Result<()> {
        info!("🛡️ Demonstrating error handling and recovery");

        // Try to create stream with invalid device ID
        let invalid_device_id = 999999;
        let config = CoreAudioStreamConfig {
            sample_rate: 48000.0,
            channels: 2,
            buffer_size: 512,
            is_input: true,
        };

        let buffer_size = 4096;
        let (producer, _consumer) = rtrb::RingBuffer::<f32>::new(buffer_size);
        let input_notifier = Arc::new(Notify::new());

        match self
            .manager
            .create_input_stream(invalid_device_id, config, producer, input_notifier)
            .await
        {
            Ok(stream_id) => {
                warn!("⚠️ Unexpectedly succeeded creating stream with invalid device: {}", stream_id);
            }
            Err(e) => {
                info!("✅ Properly handled invalid device error: {}", e);
            }
        }

        // Demonstrate graceful fallback
        info!("🔄 Demonstrating graceful fallback to default device");

        // Try to get default input device as fallback
        match self.manager.get_default_input_device().await {
            Ok(Some(default_device)) => {
                info!("✅ Fallback to default input device: {}", default_device.name);
            }
            Ok(None) => {
                warn!("⚠️ No default input device available");
            }
            Err(e) => {
                error!("❌ Failed to get default input device: {}", e);
            }
        }

        Ok(())
    }

    /// Run a comprehensive demonstration
    pub async fn run_demonstration(&mut self) -> Result<()> {
        info!("🚀 Starting comprehensive CoreAudio CPAL replacement demonstration");

        // Step 1: Device enumeration
        self.demonstrate_device_enumeration().await?;

        // Step 2: Stream creation
        self.demonstrate_stream_creation().await?;

        // Step 3: Format conversion
        self.demonstrate_format_conversion().await?;

        // Step 4: Error handling
        self.demonstrate_error_handling().await?;

        info!("✅ CoreAudio CPAL replacement demonstration completed successfully");
        info!(
            "📊 Final statistics: {} active streams",
            self.active_streams.len()
        );

        Ok(())
    }

    /// Clean shutdown of the CoreAudio system
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("🔴 Shutting down CoreAudio CPAL replacement system");

        // Stop device monitoring
        self.notifier.stop_monitoring().await?;

        // Shutdown manager (will stop all streams)
        self.manager.shutdown().await?;

        self.active_streams.clear();
        info!("✅ CoreAudio system shutdown complete");
        Ok(())
    }
}

// Non-macOS stub implementation
#[cfg(not(target_os = "macos"))]
pub struct CoreAudioExample;

#[cfg(not(target_os = "macos"))]
impl CoreAudioExample {
    pub async fn new() -> anyhow::Result<Self> {
        Err(anyhow::anyhow!("CoreAudio not available on this platform"))
    }

    pub async fn run_demonstration(&mut self) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("CoreAudio not available on this platform"))
    }
}