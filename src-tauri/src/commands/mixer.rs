use crate::{
   AudioConfigFactory,  MixerConfig
};
use std::sync::Arc;
use tauri::State;
use tracing::error;


#[tauri::command]
pub fn get_dj_mixer_config() -> MixerConfig {
    AudioConfigFactory::create_dj_config()
}