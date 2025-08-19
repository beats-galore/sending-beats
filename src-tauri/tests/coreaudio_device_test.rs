use sendin_beats_lib::audio::{VirtualMixer, MixerConfig, AudioDeviceManager};

#[tokio::test]
async fn test_coreaudio_device_selection() {
    // Create device manager
    let device_manager = std::sync::Arc::new(AudioDeviceManager::new().expect("Failed to create device manager"));
    
    // Enumerate all devices
    let devices = device_manager.enumerate_devices().await.expect("Failed to enumerate devices");
    println!("Found {} total devices", devices.len());
    
    // Look for hardware devices from CoreAudio
    let hardware_devices: Vec<_> = devices.iter()
        .filter(|d| d.host_api == "CoreAudio (Direct)" && d.is_output)
        .collect();
    
    println!("Found {} CoreAudio hardware output devices:", hardware_devices.len());
    for device in &hardware_devices {
        println!("  - {} ({})", device.name, device.id);
    }
    
    // Test device selection for each hardware device
    for device in &hardware_devices {
        println!("\nTesting device selection for: {}", device.name);
        
        match device_manager.find_audio_device(&device.id, false).await {
            Ok(device_handle) => {
                println!("  ✅ Successfully found device handle: {:?}", device_handle);
                
                // Try to create a mixer and set this as output
                let config = MixerConfig::default();
                let mixer = VirtualMixer::new_with_device_manager(config, device_manager.clone()).await.expect("Failed to create mixer");
                
                match mixer.set_output_stream(&device.id).await {
                    Ok(()) => {
                        println!("  ✅ Successfully set as output stream (placeholder implementation)");
                    }
                    Err(e) => {
                        println!("  ❌ Failed to set as output stream: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("  ❌ Failed to find device handle: {}", e);
            }
        }
    }
    
    // The test passes if we found at least some hardware devices
    assert!(!hardware_devices.is_empty(), "Should find at least one CoreAudio hardware device");
}