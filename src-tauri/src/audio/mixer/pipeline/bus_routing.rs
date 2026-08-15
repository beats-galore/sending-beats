// Bus routing: which inputs feed which mix, and which outputs take it
//
// A bus is a named mix. An input sends to any number of buses, an output takes
// exactly one, and outputs sharing a bus share its mix — so a configuration is
// summed once however many destinations it reaches.
//
// Membership lives on the bus itself rather than in a separate index, so an
// output can only ever be attached to one bus by construction.

use std::collections::{BTreeMap, BTreeSet};

/// The bus an input or output is attached to until told otherwise
///
/// Every device joins this on registration, which is what makes the default
/// configuration identical to the single shared mix that preceded buses.
pub const MAIN_BUS_ID: &str = "main";

/// Display name given to the main bus when the registry is created
const MAIN_BUS_NAME: &str = "Main";

#[derive(Debug, PartialEq, Eq)]
pub enum BusError {
    UnknownBus(String),
    DuplicateBus(String),
    /// The main bus is where devices fall back to, so it cannot be removed
    MainBusRequired,
}

impl std::fmt::Display for BusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownBus(id) => write!(f, "no bus with id '{}'", id),
            Self::DuplicateBus(id) => write!(f, "a bus with id '{}' already exists", id),
            Self::MainBusRequired => write!(f, "the main bus cannot be removed"),
        }
    }
}

/// A named mix, its members, and the trim applied to it
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Bus {
    pub id: String,
    pub name: String,
    pub gain: f32,
    /// Input device IDs summed into this bus
    pub inputs: BTreeSet<String>,
    /// Output device IDs that receive this bus
    pub outputs: BTreeSet<String>,
}

impl Bus {
    fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            gain: 1.0,
            inputs: BTreeSet::new(),
            outputs: BTreeSet::new(),
        }
    }
}

/// Every bus and the devices attached to it
pub struct BusRegistry {
    /// Ordered so the mixing thread visits buses the same way every cycle
    buses: BTreeMap<String, Bus>,
}

impl BusRegistry {
    /// A registry holding only the main bus
    pub fn new() -> Self {
        let mut buses = BTreeMap::new();
        buses.insert(
            MAIN_BUS_ID.to_string(),
            Bus::new(MAIN_BUS_ID.to_string(), MAIN_BUS_NAME.to_string()),
        );
        Self { buses }
    }

    /// Every bus, in a stable order
    ///
    /// The mixing thread walks this directly each cycle, so it borrows rather
    /// than collecting.
    pub fn buses(&self) -> impl Iterator<Item = &Bus> {
        self.buses.values()
    }

    pub fn get(&self, bus_id: &str) -> Option<&Bus> {
        self.buses.get(bus_id)
    }

    pub fn len(&self) -> usize {
        self.buses.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buses.is_empty()
    }

    pub fn create(&mut self, bus_id: String, name: String) -> Result<(), BusError> {
        if self.buses.contains_key(&bus_id) {
            return Err(BusError::DuplicateBus(bus_id));
        }

        self.buses.insert(bus_id.clone(), Bus::new(bus_id, name));
        Ok(())
    }

    /// Remove a bus, moving anything that took it back to the main bus
    ///
    /// Outputs are reassigned rather than dropped: an output left attached to
    /// nothing would receive no audio at all and its worker would underrun.
    pub fn remove(&mut self, bus_id: &str) -> Result<(), BusError> {
        if bus_id == MAIN_BUS_ID {
            return Err(BusError::MainBusRequired);
        }

        let bus = self
            .buses
            .remove(bus_id)
            .ok_or_else(|| BusError::UnknownBus(bus_id.to_string()))?;

        if let Some(main) = self.buses.get_mut(MAIN_BUS_ID) {
            main.outputs.extend(bus.outputs);
        }

        Ok(())
    }

    pub fn set_gain(&mut self, bus_id: &str, gain: f32) -> Result<(), BusError> {
        let bus = self
            .buses
            .get_mut(bus_id)
            .ok_or_else(|| BusError::UnknownBus(bus_id.to_string()))?;

        bus.gain = gain;
        Ok(())
    }

    /// Start tracking an input, sending to the main bus
    pub fn attach_input(&mut self, device_id: String) {
        if let Some(main) = self.buses.get_mut(MAIN_BUS_ID) {
            main.inputs.insert(device_id);
        }
    }

    /// Stop tracking an input, removing it from every bus it sent to
    pub fn detach_input(&mut self, device_id: &str) {
        for bus in self.buses.values_mut() {
            bus.inputs.remove(device_id);
        }
    }

    /// Replace the set of buses an input sends to
    ///
    /// Nothing is changed unless every named bus exists, so a partly-applied
    /// send list cannot leave an input routed somewhere it was never meant to
    /// reach.
    pub fn set_input_sends(&mut self, device_id: &str, bus_ids: &[String]) -> Result<(), BusError> {
        if let Some(unknown) = bus_ids.iter().find(|id| !self.buses.contains_key(*id)) {
            return Err(BusError::UnknownBus(unknown.clone()));
        }

        for bus in self.buses.values_mut() {
            if bus_ids.contains(&bus.id) {
                bus.inputs.insert(device_id.to_string());
            } else {
                bus.inputs.remove(device_id);
            }
        }

        Ok(())
    }

    /// The buses an input currently sends to
    pub fn sends_of(&self, device_id: &str) -> Vec<&str> {
        self.buses
            .values()
            .filter(|bus| bus.inputs.contains(device_id))
            .map(|bus| bus.id.as_str())
            .collect()
    }

    /// Start tracking an output, taking the main bus
    pub fn attach_output(&mut self, device_id: String) {
        if let Some(main) = self.buses.get_mut(MAIN_BUS_ID) {
            main.outputs.insert(device_id);
        }
    }

    /// Stop tracking an output, removing it from whichever bus it took
    pub fn detach_output(&mut self, device_id: &str) {
        for bus in self.buses.values_mut() {
            bus.outputs.remove(device_id);
        }
    }

    /// Move an output onto a bus, taking it off the one it was on
    pub fn set_output_bus(&mut self, device_id: &str, bus_id: &str) -> Result<(), BusError> {
        if !self.buses.contains_key(bus_id) {
            return Err(BusError::UnknownBus(bus_id.to_string()));
        }

        for bus in self.buses.values_mut() {
            if bus.id == bus_id {
                bus.outputs.insert(device_id.to_string());
            } else {
                bus.outputs.remove(device_id);
            }
        }

        Ok(())
    }

    /// The bus an output takes, if it is attached to one
    pub fn bus_of_output(&self, device_id: &str) -> Option<&str> {
        self.buses
            .values()
            .find(|bus| bus.outputs.contains(device_id))
            .map(|bus| bus.id.as_str())
    }
}

impl Default for BusRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for BusRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BusRegistry")
            .field("buses", &self.buses.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn a_new_registry_holds_only_the_main_bus() {
        let registry = BusRegistry::new();

        assert_eq!(registry.len(), 1);
        let main = registry.get(MAIN_BUS_ID).unwrap();
        assert_eq!(main.gain, 1.0);
        assert!(main.inputs.is_empty());
        assert!(main.outputs.is_empty());
    }

    #[test]
    fn registered_devices_default_to_the_main_bus() {
        let mut registry = BusRegistry::new();
        registry.attach_input("mic".to_string());
        registry.attach_output("speakers".to_string());

        // Matches the single shared mix that preceded buses
        assert_eq!(registry.sends_of("mic"), vec![MAIN_BUS_ID]);
        assert_eq!(registry.bus_of_output("speakers"), Some(MAIN_BUS_ID));
    }

    #[test]
    fn an_input_can_reach_one_output_and_not_another() {
        let mut registry = BusRegistry::new();
        registry
            .create("cue".to_string(), "Cue".to_string())
            .unwrap();

        registry.attach_input("mic".to_string());
        registry.attach_input("deck".to_string());
        registry.attach_output("speakers".to_string());
        registry.attach_output("headphones".to_string());

        registry.set_input_sends("deck", &ids(&["cue"])).unwrap();
        registry.set_output_bus("headphones", "cue").unwrap();

        let main = registry.get(MAIN_BUS_ID).unwrap();
        assert_eq!(main.inputs, ["mic".to_string()].into_iter().collect());
        assert_eq!(main.outputs, ["speakers".to_string()].into_iter().collect());

        let cue = registry.get("cue").unwrap();
        assert_eq!(cue.inputs, ["deck".to_string()].into_iter().collect());
        assert_eq!(
            cue.outputs,
            ["headphones".to_string()].into_iter().collect()
        );
    }

    #[test]
    fn an_input_can_send_to_several_buses_at_once() {
        let mut registry = BusRegistry::new();
        registry
            .create("stream".to_string(), "Stream".to_string())
            .unwrap();
        registry.attach_input("mic".to_string());

        registry
            .set_input_sends("mic", &ids(&[MAIN_BUS_ID, "stream"]))
            .unwrap();

        let mut sends = registry.sends_of("mic");
        sends.sort();
        assert_eq!(sends, vec![MAIN_BUS_ID, "stream"]);
    }

    #[test]
    fn an_output_takes_exactly_one_bus() {
        let mut registry = BusRegistry::new();
        registry
            .create("cue".to_string(), "Cue".to_string())
            .unwrap();
        registry.attach_output("headphones".to_string());

        registry.set_output_bus("headphones", "cue").unwrap();

        assert_eq!(registry.bus_of_output("headphones"), Some("cue"));
        assert!(registry.get(MAIN_BUS_ID).unwrap().outputs.is_empty());
        let holders = registry
            .buses()
            .filter(|bus| bus.outputs.contains("headphones"))
            .count();
        assert_eq!(holders, 1);
    }

    #[test]
    fn clearing_an_inputs_sends_leaves_it_reaching_nothing() {
        let mut registry = BusRegistry::new();
        registry.attach_input("mic".to_string());

        registry.set_input_sends("mic", &[]).unwrap();

        assert!(registry.sends_of("mic").is_empty());
    }

    #[test]
    fn removing_a_bus_moves_its_outputs_back_to_main() {
        let mut registry = BusRegistry::new();
        registry
            .create("cue".to_string(), "Cue".to_string())
            .unwrap();
        registry.attach_output("headphones".to_string());
        registry.set_output_bus("headphones", "cue").unwrap();

        registry.remove("cue").unwrap();

        // An output left on no bus would receive nothing and underrun
        assert_eq!(registry.bus_of_output("headphones"), Some(MAIN_BUS_ID));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn the_main_bus_cannot_be_removed() {
        let mut registry = BusRegistry::new();

        assert_eq!(registry.remove(MAIN_BUS_ID), Err(BusError::MainBusRequired));
        assert!(registry.get(MAIN_BUS_ID).is_some());
    }

    #[test]
    fn unknown_and_duplicate_buses_are_rejected() {
        let mut registry = BusRegistry::new();
        registry
            .create("cue".to_string(), "Cue".to_string())
            .unwrap();

        assert_eq!(
            registry.create("cue".to_string(), "Cue Again".to_string()),
            Err(BusError::DuplicateBus("cue".to_string()))
        );
        assert_eq!(
            registry.remove("nope"),
            Err(BusError::UnknownBus("nope".to_string()))
        );
        assert_eq!(
            registry.set_output_bus("headphones", "nope"),
            Err(BusError::UnknownBus("nope".to_string()))
        );
        assert_eq!(
            registry.set_gain("nope", 0.5),
            Err(BusError::UnknownBus("nope".to_string()))
        );
    }

    #[test]
    fn a_rejected_send_list_changes_nothing() {
        let mut registry = BusRegistry::new();
        registry
            .create("cue".to_string(), "Cue".to_string())
            .unwrap();
        registry.attach_input("mic".to_string());

        let result = registry.set_input_sends("mic", &ids(&["cue", "nope"]));

        assert_eq!(result, Err(BusError::UnknownBus("nope".to_string())));
        assert_eq!(
            registry.sends_of("mic"),
            vec![MAIN_BUS_ID],
            "the original sends survive a rejected change"
        );
    }

    #[test]
    fn detaching_a_device_clears_it_from_every_bus() {
        let mut registry = BusRegistry::new();
        registry
            .create("cue".to_string(), "Cue".to_string())
            .unwrap();
        registry.attach_input("mic".to_string());
        registry.attach_output("headphones".to_string());
        registry
            .set_input_sends("mic", &ids(&[MAIN_BUS_ID, "cue"]))
            .unwrap();
        registry.set_output_bus("headphones", "cue").unwrap();

        registry.detach_input("mic");
        registry.detach_output("headphones");

        assert!(registry.sends_of("mic").is_empty());
        assert_eq!(registry.bus_of_output("headphones"), None);
        assert!(registry.buses().all(|bus| bus.inputs.is_empty()));
        assert!(registry.buses().all(|bus| bus.outputs.is_empty()));
    }

    #[test]
    fn bus_gain_is_stored_per_bus() {
        let mut registry = BusRegistry::new();
        registry
            .create("cue".to_string(), "Cue".to_string())
            .unwrap();

        registry.set_gain("cue", 0.5).unwrap();

        assert_eq!(registry.get("cue").unwrap().gain, 0.5);
        assert_eq!(registry.get(MAIN_BUS_ID).unwrap().gain, 1.0);
    }
}
