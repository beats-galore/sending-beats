use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::audio::types::{AudioChannel, AudioDeviceHandle, MixerConfig, OutputDevice};

use super::stream_management::{get_stream_manager, StreamCommand};

// Global stream manager functions moved to stream_management.rs

// VirtualMixerHandle moved to mixer_core.rs
