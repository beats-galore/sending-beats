// Bus routing: which inputs feed which mix, and which outputs take it
//
// A bus is a mix shared by the destinations that want the same sources. It is
// not stored — it is derived, every time the routing changes, from the one thing
// that is: what each output receives.
//
// That derivation is what makes the model hold together. Storing membership on
// the bus meant an output id could appear in two buses' output sets, and each
// bus would write its own mix into that output's queue — two mixes interleaved
// into one ring, which is audible as a jumbled mess. Here an output has exactly
// one entry in one map, so being fed twice is not a state that can be written.
//
// It also means there is only ever one bus per distinct set of sources. Two
// destinations asking for the same inputs are the same mix by construction,
// rather than by a lookup that has to remember to check.

use std::collections::{BTreeMap, BTreeSet};

/// The bus outputs take until they are routed somewhere else
///
/// Present only while some output is still unrouted. It is not a permanent row:
/// a main bus that outlived everything taking it is the orphan this model exists
/// to avoid.
pub const MAIN_BUS_ID: &str = "main";

/// Display name given to the bus unrouted outputs share
const MAIN_BUS_NAME: &str = "Main";

#[derive(Debug, PartialEq, Eq)]
pub enum BusError {
    UnknownBus(String),
    DuplicateBus(String),
    /// The main bus is where devices fall back to, so it cannot be removed
    MainBusRequired,
    /// Buses follow the routing rather than being made and destroyed directly
    Derived,
}

impl std::fmt::Display for BusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownBus(id) => write!(f, "no bus with id '{}'", id),
            Self::DuplicateBus(id) => write!(f, "a bus with id '{}' already exists", id),
            Self::MainBusRequired => write!(f, "the main bus cannot be removed"),
            Self::Derived => write!(
                f,
                "buses follow what each output receives; route an output instead"
            ),
        }
    }
}

/// A named mix, its members, and the trim applied to it
///
/// A view over the routing rather than something stored. `outputs` is never
/// shared between two of these, because it is built by partitioning the outputs.
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

/// What one output receives
#[derive(Debug, Clone, PartialEq, Eq)]
enum OutputSources {
    /// Never routed by hand, so it takes whatever is attached — including
    /// sources added later, which is what makes a new input audible by default.
    All,
    /// Exactly these, whatever else arrives
    Explicit(BTreeSet<String>),
}

/// What survives a bus being rebuilt: the parts the routing does not decide
#[derive(Debug, Clone)]
struct BusMeta {
    name: String,
    gain: f32,
    /// The outputs this id described last time, used to recognise it again
    outputs: BTreeSet<String>,
}

/// The routing, and the buses derived from it
#[derive(Clone)]
pub struct BusRegistry {
    /// Every attached input, so an unrouted output can be given all of them
    inputs: BTreeSet<String>,
    /// The source of truth. One entry per output, so one mix per output.
    outputs: BTreeMap<String, OutputSources>,
    /// Names and trims, carried across a rebuild by which outputs they described
    meta: BTreeMap<String, BusMeta>,
    /// Rebuilt on every change so the mixing thread can walk it without allocating
    derived: BTreeMap<String, Bus>,
    /// Counter behind the auto names, so two mixes never take the same one
    next_mix_number: usize,
}

impl BusRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            inputs: BTreeSet::new(),
            outputs: BTreeMap::new(),
            meta: BTreeMap::new(),
            derived: BTreeMap::new(),
            next_mix_number: 2,
        };
        registry.rebuild();
        registry
    }

    /// Every bus, in a stable order
    ///
    /// The mixing thread walks this each cycle, so it borrows the derived view
    /// rather than deriving on the spot.
    pub fn buses(&self) -> impl Iterator<Item = &Bus> {
        self.derived.values()
    }

    pub fn get(&self, bus_id: &str) -> Option<&Bus> {
        self.derived.get(bus_id)
    }

    pub fn len(&self) -> usize {
        self.derived.len()
    }

    pub fn is_empty(&self) -> bool {
        self.derived.is_empty()
    }

    /// Buses are made by routing an output, not on their own
    pub fn create(&mut self, _bus_id: String, _name: String) -> Result<(), BusError> {
        Err(BusError::Derived)
    }

    /// Buses go away when nothing takes them, rather than being removed
    ///
    /// Everything that took this bus goes back to receiving every input, which
    /// is what an output with no routing of its own gets.
    pub fn remove(&mut self, bus_id: &str) -> Result<(), BusError> {
        if bus_id == MAIN_BUS_ID {
            return Err(BusError::MainBusRequired);
        }

        let Some(bus) = self.derived.get(bus_id) else {
            return Err(BusError::UnknownBus(bus_id.to_string()));
        };

        let taken_by: Vec<String> = bus.outputs.iter().cloned().collect();
        for output_id in taken_by {
            self.outputs.insert(output_id, OutputSources::All);
        }

        self.rebuild();
        Ok(())
    }

    pub fn set_gain(&mut self, bus_id: &str, gain: f32) -> Result<(), BusError> {
        if !self.derived.contains_key(bus_id) {
            return Err(BusError::UnknownBus(bus_id.to_string()));
        }

        self.meta
            .entry(bus_id.to_string())
            .and_modify(|meta| meta.gain = gain);
        self.rebuild();
        Ok(())
    }

    /// Start tracking an input
    ///
    /// Unrouted outputs pick it up on the next rebuild; ones pointed at a named
    /// set do not, because that set said what it wanted.
    pub fn attach_input(&mut self, device_id: String) {
        self.inputs.insert(device_id);
        self.rebuild();
    }

    /// Stop tracking an input, taking it out of everything that asked for it
    pub fn detach_input(&mut self, device_id: &str) {
        self.inputs.remove(device_id);
        for sources in self.outputs.values_mut() {
            if let OutputSources::Explicit(set) = sources {
                set.remove(device_id);
            }
        }
        self.rebuild();
    }

    /// Replace the set of buses an input sends to
    ///
    /// Written through the outputs, since that is where membership lives: the
    /// input is added to or removed from the source list of everything taking
    /// each bus. Nothing changes unless every named bus exists.
    pub fn set_input_sends(&mut self, device_id: &str, bus_ids: &[String]) -> Result<(), BusError> {
        if let Some(unknown) = bus_ids.iter().find(|id| !self.derived.contains_key(*id)) {
            return Err(BusError::UnknownBus(unknown.clone()));
        }

        let wanted: BTreeSet<String> = bus_ids.iter().cloned().collect();
        let mut edits: Vec<(String, bool)> = Vec::new();
        for bus in self.derived.values() {
            let sending = wanted.contains(&bus.id);
            for output_id in bus.outputs.iter() {
                edits.push((output_id.clone(), sending));
            }
        }

        for (output_id, sending) in edits {
            let resolved = self.resolved_sources(&output_id);
            let mut set = resolved;
            if sending {
                set.insert(device_id.to_string());
            } else {
                set.remove(device_id);
            }
            self.outputs.insert(output_id, OutputSources::Explicit(set));
        }

        self.rebuild();
        Ok(())
    }

    /// The buses an input currently sends to
    pub fn sends_of(&self, device_id: &str) -> Vec<&str> {
        self.derived
            .values()
            .filter(|bus| bus.inputs.contains(device_id))
            .map(|bus| bus.id.as_str())
            .collect()
    }

    /// Start tracking an output
    ///
    /// Idempotent on purpose. A device that re-registers — hotplug, or its
    /// source being switched — keeps the routing it already had. Overwriting it
    /// here is what used to leave an output on two buses at once.
    pub fn attach_output(&mut self, device_id: String) {
        self.outputs.entry(device_id).or_insert(OutputSources::All);
        self.rebuild();
    }

    /// Stop tracking an output
    pub fn detach_output(&mut self, device_id: &str) {
        self.outputs.remove(device_id);
        self.rebuild();
    }

    /// Move an output onto a bus, by giving it that bus's sources
    pub fn set_output_bus(&mut self, device_id: &str, bus_id: &str) -> Result<(), BusError> {
        let Some(bus) = self.derived.get(bus_id) else {
            return Err(BusError::UnknownBus(bus_id.to_string()));
        };

        let sources = bus.inputs.clone();
        self.outputs
            .insert(device_id.to_string(), OutputSources::Explicit(sources));
        self.rebuild();
        Ok(())
    }

    /// Point an output at the exact set of inputs it should be receiving
    ///
    /// What the patchbay's tiles write, from either side of a connection.
    /// Destinations asking for the same inputs come out on the same bus because
    /// the buses are grouped by that set, not because a lookup matched.
    ///
    /// Returns the bus the output ended up on.
    pub fn set_output_sources(&mut self, output_id: &str, inputs: &[String]) -> String {
        let desired: BTreeSet<String> = inputs.iter().cloned().collect();
        self.outputs
            .insert(output_id.to_string(), OutputSources::Explicit(desired));
        self.rebuild();

        self.bus_of_output(output_id)
            .unwrap_or(MAIN_BUS_ID)
            .to_string()
    }

    /// Lay stored routing over whatever is currently attached
    ///
    /// Devices register before their routing is restored, and the set that
    /// registered need not match the set that was saved — hardware comes and
    /// goes. An output the stored routing says nothing about keeps what it has,
    /// so a destination added since the save still receives audio.
    pub fn restore(&mut self, stored: &[Bus]) {
        for bus in stored {
            self.meta.insert(
                bus.id.clone(),
                BusMeta {
                    name: bus.name.clone(),
                    gain: bus.gain,
                    outputs: bus.outputs.clone(),
                },
            );
        }

        let attached: Vec<String> = self.outputs.keys().cloned().collect();
        for output_id in attached {
            if let Some(bus) = stored.iter().find(|bus| bus.outputs.contains(&output_id)) {
                self.outputs
                    .insert(output_id, OutputSources::Explicit(bus.inputs.clone()));
            }
        }

        self.rebuild();
    }

    /// The bus an output takes, if it is attached to one
    pub fn bus_of_output(&self, device_id: &str) -> Option<&str> {
        self.derived
            .values()
            .find(|bus| bus.outputs.contains(device_id))
            .map(|bus| bus.id.as_str())
    }

    /// What an output receives right now, with departed inputs dropped
    fn resolved_sources(&self, output_id: &str) -> BTreeSet<String> {
        match self.outputs.get(output_id) {
            Some(OutputSources::Explicit(set)) => set.intersection(&self.inputs).cloned().collect(),
            // Unrouted, or not attached at all
            _ => self.inputs.clone(),
        }
    }

    /// Rebuild the derived buses from the routing
    ///
    /// Outputs wanting the same sources are one bus. Ids are carried over by
    /// which outputs a bus described last time, so a name and a trim survive the
    /// routing around them changing.
    ///
    /// A mix needs something going into it and something taking it out. A group
    /// with no inputs is not a quiet bus, it is a destination that was told to
    /// receive nothing, and drawing it as a mix says there is a signal path
    /// where there is none. Those outputs are listed by `unfed_outputs` instead
    /// and handed silence, which is what keeps their workers from underrunning.
    fn rebuild(&mut self) {
        let mut groups: BTreeMap<BTreeSet<String>, BTreeSet<String>> = BTreeMap::new();
        let mut unrouted: BTreeSet<String> = BTreeSet::new();

        for (output_id, sources) in self.outputs.iter() {
            let resolved = match sources {
                OutputSources::All => {
                    unrouted.insert(output_id.clone());
                    self.inputs.clone()
                }
                OutputSources::Explicit(set) => set.intersection(&self.inputs).cloned().collect(),
            };

            if resolved.is_empty() {
                continue;
            }

            groups
                .entry(resolved)
                .or_default()
                .insert(output_id.clone());
        }

        let mut derived: BTreeMap<String, Bus> = BTreeMap::new();
        let mut claimed: BTreeSet<String> = BTreeSet::new();

        for (sources, outputs) in groups {
            let holds_unrouted = outputs.iter().any(|id| unrouted.contains(id));
            let id = if holds_unrouted {
                MAIN_BUS_ID.to_string()
            } else {
                self.claim_id(&outputs, &mut claimed)
            };
            claimed.insert(id.clone());

            let meta = self.meta.entry(id.clone()).or_insert_with(|| BusMeta {
                name: String::new(),
                gain: 1.0,
                outputs: BTreeSet::new(),
            });
            meta.outputs = outputs.clone();

            derived.insert(
                id.clone(),
                Bus {
                    id,
                    name: meta.name.clone(),
                    gain: meta.gain,
                    inputs: sources,
                    outputs,
                },
            );
        }

        // Named by where they sit, once the whole set is known.
        //
        // A stored counter drifted: a rebuild runs on every attach and every
        // routing edit, and a group that momentarily disappeared came back as a
        // fresh id with the next number, so two mixes could be called "Mix 7"
        // and "Mix 10". Nothing renames a bus, so the label may as well be read
        // off the result and be right every time.
        let mut mix_number = 2;
        for bus in derived.values_mut() {
            if bus.id == MAIN_BUS_ID {
                bus.name = MAIN_BUS_NAME.to_string();
            } else {
                bus.name = format!("Mix {}", mix_number);
                mix_number += 1;
            }

            if let Some(meta) = self.meta.get_mut(&bus.id) {
                meta.name = bus.name.clone();
            }
        }

        // Names and trims outlive a rebuild but not the bus itself, or a mix
        // that came and went would hand its name to an unrelated one later.
        self.meta.retain(|id, _| derived.contains_key(id));
        self.derived = derived;
    }

    /// Outputs on no bus at all, which are owed silence rather than a mix
    ///
    /// An output told to take nothing still has a worker draining its queue, so
    /// it has to be written to every cycle or it underruns.
    pub fn unfed_outputs(&self) -> impl Iterator<Item = &str> {
        self.outputs
            .keys()
            .filter(|id| self.bus_of_output(id).is_none())
            .map(|id| id.as_str())
    }

    /// The id that best describes this set of outputs, or a new one
    fn claim_id(&self, outputs: &BTreeSet<String>, claimed: &mut BTreeSet<String>) -> String {
        let best = self
            .meta
            .iter()
            .filter(|(id, _)| *id != MAIN_BUS_ID && !claimed.contains(*id))
            .map(|(id, meta)| (meta.outputs.intersection(outputs).count(), id))
            .filter(|(overlap, _)| *overlap > 0)
            .max_by_key(|(overlap, id)| (*overlap, (*id).clone()));

        match best {
            Some((_, id)) => id.clone(),
            None => uuid::Uuid::new_v4().to_string(),
        }
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
            .field("buses", &self.derived.len())
            .field("outputs", &self.outputs.len())
            .finish()
    }
}
