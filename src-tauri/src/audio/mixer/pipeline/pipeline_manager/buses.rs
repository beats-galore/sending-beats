// Bus routing on the pipeline
//
// A child module so these reach AudioPipeline's private mixing layer without
// growing pipeline_manager.rs, which is already past the size it should be.

use anyhow::Result;

use super::super::bus_routing::Bus;
use super::AudioPipeline;

impl AudioPipeline {
    // Bus routing
    //
    // Buses decide which inputs reach which outputs. Every device starts on the
    // main bus, so a pipeline nobody has routed behaves as one shared mix.

    pub fn create_bus(&mut self, bus_id: String, name: String) -> Result<()> {
        self.mixing_layer
            .create_bus(bus_id, name)
            .map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    pub fn remove_bus(&mut self, bus_id: String) -> Result<()> {
        self.mixing_layer
            .remove_bus(bus_id)
            .map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    pub fn set_bus_gain(&mut self, bus_id: String, gain: f32) -> Result<()> {
        self.mixing_layer
            .set_bus_gain(bus_id, gain)
            .map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    pub fn set_input_sends(&mut self, device_id: String, bus_ids: Vec<String>) -> Result<()> {
        self.mixing_layer
            .set_input_sends(device_id, bus_ids)
            .map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    pub fn set_output_bus(&mut self, device_id: String, bus_id: String) -> Result<()> {
        self.mixing_layer
            .set_output_bus(device_id, bus_id)
            .map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    pub fn list_buses(&self) -> Vec<Bus> {
        self.mixing_layer.buses()
    }
}
