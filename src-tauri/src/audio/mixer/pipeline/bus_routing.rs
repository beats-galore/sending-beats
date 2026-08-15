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
#[derive(Clone)]
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

    /// Point an output at the exact set of inputs it should be receiving
    ///
    /// This is what the patchbay's tiles write: an output owns a list of
    /// sources, and toggling one from either the input's side or the output's
    /// side means the same edit. Buses stay the thing the mixer works from —
    /// the set is matched to a bus carrying exactly those inputs, and only when
    /// none does is a new one made. Two destinations asking for the same inputs
    /// therefore land on the same bus and are summed once between them.
    ///
    /// Returns the bus the output ended up on.
    pub fn set_output_sources(&mut self, output_id: &str, inputs: &[String]) -> String {
        let desired: BTreeSet<String> = inputs.iter().cloned().collect();

        let bus_id = match self.buses.values().find(|bus| bus.inputs == desired) {
            Some(bus) => bus.id.clone(),
            None => {
                let bus_id = uuid::Uuid::new_v4().to_string();
                let mut bus = Bus::new(bus_id.clone(), self.next_auto_name());
                bus.inputs = desired;
                self.buses.insert(bus_id.clone(), bus);
                bus_id
            }
        };

        // Cannot fail: the bus was just found or inserted
        let _ = self.set_output_bus(output_id, &bus_id);
        self.drop_unheard_buses();

        bus_id
    }

    /// Forget buses no output is taking any more
    ///
    /// Without this a bus is left behind every time a destination's sources
    /// change, and `list_audio_buses` would fill up with mixes nothing can
    /// hear. The main bus stays regardless — it is where a device with no
    /// routing of its own belongs.
    fn drop_unheard_buses(&mut self) {
        self.buses
            .retain(|id, bus| id == MAIN_BUS_ID || !bus.outputs.is_empty());
    }

    /// A name for a bus the routing made on its own
    ///
    /// The patchbay labels a group by what it contains rather than by this, so
    /// it only has to be something to show if a bus is ever listed raw.
    fn next_auto_name(&self) -> String {
        format!("Mix {}", self.buses.len() + 1)
    }

    /// Lay stored routing over whatever is currently attached
    ///
    /// Devices register before their routing is restored, and the set that
    /// registered need not match the set that was saved — hardware comes and
    /// goes. A device the stored routing says nothing about therefore keeps
    /// where it already is rather than being detached, so a source plugged in
    /// since the save stays on the main bus instead of falling silent.
    ///
    /// The cost of that rule is that an input deliberately left reaching
    /// nothing cannot be told apart from one that was never saved, and comes
    /// back on the main bus. Muting a channel expresses the same thing and
    /// does survive.
    pub fn restore(&mut self, stored: &[Bus]) {
        let attached_inputs = self.attached(|bus| &bus.inputs);
        let attached_outputs = self.attached(|bus| &bus.outputs);

        for bus in stored {
            match self.buses.get_mut(&bus.id) {
                Some(existing) => {
                    existing.name = bus.name.clone();
                    existing.gain = bus.gain;
                }
                None => {
                    let mut restored = Bus::new(bus.id.clone(), bus.name.clone());
                    restored.gain = bus.gain;
                    self.buses.insert(bus.id.clone(), restored);
                }
            }
        }

        for device_id in attached_inputs {
            let sends: Vec<String> = stored
                .iter()
                .filter(|bus| bus.inputs.contains(&device_id))
                .map(|bus| bus.id.clone())
                .collect();

            if !sends.is_empty() {
                let _ = self.set_input_sends(&device_id, &sends);
            }
        }

        for device_id in attached_outputs {
            if let Some(bus) = stored.iter().find(|bus| bus.outputs.contains(&device_id)) {
                let bus_id = bus.id.clone();
                let _ = self.set_output_bus(&device_id, &bus_id);
            }
        }
    }

    /// Every device currently on some bus, in the given direction
    fn attached(&self, side: impl Fn(&Bus) -> &BTreeSet<String>) -> BTreeSet<String> {
        self.buses
            .values()
            .flat_map(|bus| side(bus).iter().cloned())
            .collect()
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

    fn ids_of(registry: &BusRegistry, device_id: &str) -> Option<String> {
        registry.bus_of_output(device_id).map(|id| id.to_string())
    }

    #[test]
    fn destinations_wanting_the_same_inputs_share_one_bus() {
        let mut registry = BusRegistry::new();
        registry.attach_input("mic".to_string());
        registry.attach_input("deck".to_string());
        registry.attach_output("speakers".to_string());
        registry.attach_output("stream".to_string());

        let a = registry.set_output_sources("speakers", &ids(&["mic"]));
        let b = registry.set_output_sources("stream", &ids(&["mic"]));

        assert_eq!(a, b, "one mix serves both rather than two identical sums");
        assert_eq!(registry.get(&a).unwrap().outputs.len(), 2);
    }

    #[test]
    fn destinations_wanting_different_inputs_get_their_own_bus() {
        let mut registry = BusRegistry::new();
        registry.attach_input("mic".to_string());
        registry.attach_input("deck".to_string());
        registry.attach_output("speakers".to_string());
        registry.attach_output("headphones".to_string());

        registry.set_output_sources("speakers", &ids(&["mic"]));
        registry.set_output_sources("headphones", &ids(&["deck"]));

        assert_ne!(
            ids_of(&registry, "speakers"),
            ids_of(&registry, "headphones")
        );
        let cue = registry
            .get(&ids_of(&registry, "headphones").unwrap())
            .unwrap();
        assert_eq!(cue.inputs, ["deck".to_string()].into_iter().collect());
    }

    #[test]
    fn changing_one_destination_leaves_the_other_alone() {
        let mut registry = BusRegistry::new();
        registry.attach_input("mic".to_string());
        registry.attach_input("deck".to_string());
        registry.attach_output("speakers".to_string());
        registry.attach_output("stream".to_string());
        registry.set_output_sources("speakers", &ids(&["mic", "deck"]));
        registry.set_output_sources("stream", &ids(&["mic", "deck"]));

        // Take the deck off the stream only
        registry.set_output_sources("stream", &ids(&["mic"]));

        let speakers = registry
            .get(&ids_of(&registry, "speakers").unwrap())
            .unwrap();
        assert_eq!(
            speakers.inputs,
            ["deck".to_string(), "mic".to_string()]
                .into_iter()
                .collect(),
            "the shared mix is not edited out from under the other destination"
        );
        let stream = registry.get(&ids_of(&registry, "stream").unwrap()).unwrap();
        assert_eq!(stream.inputs, ["mic".to_string()].into_iter().collect());
    }

    #[test]
    fn a_bus_nothing_listens_to_any_more_is_dropped() {
        let mut registry = BusRegistry::new();
        registry.attach_input("mic".to_string());
        registry.attach_input("deck".to_string());
        registry.attach_output("speakers".to_string());

        registry.set_output_sources("speakers", &ids(&["mic"]));
        let abandoned = ids_of(&registry, "speakers").unwrap();
        registry.set_output_sources("speakers", &ids(&["deck"]));

        assert!(registry.get(&abandoned).is_none());
        assert_eq!(registry.len(), 2, "main and the one in use");
    }

    #[test]
    fn the_main_bus_survives_having_no_outputs() {
        let mut registry = BusRegistry::new();
        registry.attach_input("mic".to_string());
        registry.attach_input("deck".to_string());
        registry.attach_output("speakers".to_string());

        registry.set_output_sources("speakers", &ids(&["deck"]));

        assert!(
            registry.get(MAIN_BUS_ID).is_some(),
            "a device with no routing of its own still needs somewhere to land"
        );
    }

    #[test]
    fn asking_for_everything_lands_back_on_the_main_bus() {
        let mut registry = BusRegistry::new();
        registry.attach_input("mic".to_string());
        registry.attach_input("deck".to_string());
        registry.attach_output("speakers".to_string());
        registry.set_output_sources("speakers", &ids(&["mic"]));

        // Main already carries every input, so the full set matches it
        let bus_id = registry.set_output_sources("speakers", &ids(&["mic", "deck"]));

        assert_eq!(bus_id, MAIN_BUS_ID);
    }

    #[test]
    fn an_output_taking_nothing_gets_a_bus_with_no_inputs() {
        let mut registry = BusRegistry::new();
        registry.attach_input("mic".to_string());
        registry.attach_output("speakers".to_string());

        let bus_id = registry.set_output_sources("speakers", &[]);

        assert!(registry.get(&bus_id).unwrap().inputs.is_empty());
        assert_eq!(ids_of(&registry, "speakers").as_deref(), Some(&bus_id[..]));
    }

    #[test]
    fn bus_gain_follows_a_set_that_stays_in_use() {
        let mut registry = BusRegistry::new();
        registry.attach_input("mic".to_string());
        registry.attach_input("deck".to_string());
        registry.attach_output("speakers".to_string());
        registry.attach_output("stream".to_string());

        let bus_id = registry.set_output_sources("speakers", &ids(&["mic"]));
        registry.set_gain(&bus_id, 0.5).unwrap();
        // A second destination joining the same set joins the same trim
        registry.set_output_sources("stream", &ids(&["mic"]));

        assert_eq!(registry.get(&bus_id).unwrap().gain, 0.5);
    }

    fn stored(id: &str, name: &str, inputs: &[&str], outputs: &[&str]) -> Bus {
        Bus {
            id: id.to_string(),
            name: name.to_string(),
            gain: 1.0,
            inputs: inputs.iter().map(|s| s.to_string()).collect(),
            outputs: outputs.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn restoring_recreates_buses_and_puts_devices_back_on_them() {
        let mut registry = BusRegistry::new();
        registry.attach_input("mic".to_string());
        registry.attach_input("deck".to_string());
        registry.attach_output("speakers".to_string());
        registry.attach_output("headphones".to_string());

        registry.restore(&[
            stored(MAIN_BUS_ID, "Main", &["mic"], &["speakers"]),
            stored("cue", "Cue", &["deck"], &["headphones"]),
        ]);

        assert_eq!(registry.len(), 2);
        assert_eq!(registry.sends_of("deck"), vec!["cue"]);
        assert_eq!(registry.bus_of_output("headphones"), Some("cue"));
        assert_eq!(registry.sends_of("mic"), vec![MAIN_BUS_ID]);
    }

    #[test]
    fn restoring_carries_names_and_gains() {
        let mut registry = BusRegistry::new();
        let mut quiet = stored("cue", "Cue Mix", &[], &[]);
        quiet.gain = 0.25;

        registry.restore(&[quiet]);

        let cue = registry.get("cue").unwrap();
        assert_eq!(cue.name, "Cue Mix");
        assert_eq!(cue.gain, 0.25);
    }

    #[test]
    fn a_device_the_stored_routing_never_mentions_keeps_its_place() {
        let mut registry = BusRegistry::new();
        registry.attach_input("mic".to_string());
        // Plugged in since the routing was saved
        registry.attach_input("new-mic".to_string());
        registry.attach_output("new-speakers".to_string());

        registry.restore(&[stored(MAIN_BUS_ID, "Main", &["mic"], &[])]);

        assert_eq!(
            registry.sends_of("new-mic"),
            vec![MAIN_BUS_ID],
            "a new source stays on air rather than falling silent"
        );
        assert_eq!(registry.bus_of_output("new-speakers"), Some(MAIN_BUS_ID));
    }

    #[test]
    fn stored_routing_for_a_device_that_is_not_attached_is_ignored() {
        let mut registry = BusRegistry::new();
        registry.attach_input("mic".to_string());

        // "deck" was saved but is not plugged in this session
        registry.restore(&[
            stored(MAIN_BUS_ID, "Main", &["mic"], &[]),
            stored("cue", "Cue", &["deck"], &[]),
        ]);

        assert!(registry.get("cue").unwrap().inputs.is_empty());
        assert_eq!(registry.sends_of("mic"), vec![MAIN_BUS_ID]);
    }

    #[test]
    fn restoring_moves_a_device_off_the_bus_it_defaulted_to() {
        let mut registry = BusRegistry::new();
        registry.attach_input("deck".to_string());
        assert_eq!(registry.sends_of("deck"), vec![MAIN_BUS_ID]);

        registry.restore(&[
            stored(MAIN_BUS_ID, "Main", &[], &[]),
            stored("cue", "Cue", &["deck"], &[]),
        ]);

        assert_eq!(registry.sends_of("deck"), vec!["cue"]);
        assert!(registry.get(MAIN_BUS_ID).unwrap().inputs.is_empty());
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
