mod build;
mod cycles;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, bail};

use super::model::{EdgeKind, EdgeMode, MAX_STAGE, Model};
use super::render::text::{node_id, state_id};

pub fn validate(root: &Path, model: &Model) -> Result<()> {
    let mut diagnostics = Diagnostics::default();
    diagnostics.extend(model.load_errors.iter().cloned());

    check_systems(model, &mut diagnostics);
    check_components(model, &mut diagnostics);
    check_stores(model, &mut diagnostics);
    check_edges(model, &mut diagnostics);
    check_stages(model, &mut diagnostics);
    check_invariants(model, &mut diagnostics);
    check_machines(model, &mut diagnostics);
    check_render_ids(model, &mut diagnostics);

    diagnostics.extend(build::validate(root, model));
    diagnostics.extend(cycles::validate(model));

    diagnostics.finish()
}

#[derive(Default)]
pub struct Diagnostics {
    errors: Vec<String>,
}

impl Diagnostics {
    pub fn push(&mut self, error: impl Into<String>) {
        self.errors.push(error.into());
    }

    pub fn extend(&mut self, errors: impl IntoIterator<Item = String>) {
        self.errors.extend(errors);
    }

    pub fn finish(self) -> Result<()> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            bail!(
                "architecture graph validation failed:\n- {}",
                self.errors.join("\n- ")
            )
        }
    }
}

fn check_systems(model: &Model, diagnostics: &mut Diagnostics) {
    let mut docs = BTreeMap::new();
    for system in model.systems.values() {
        if !(1..=MAX_STAGE).contains(&system.stage) {
            diagnostics.push(format!("{} has invalid stage {}", system.id, system.stage));
        }
        if system.purpose.trim().is_empty() || system.boundary.trim().is_empty() {
            diagnostics.push(format!("{} needs purpose and boundary", system.id));
        }
        if let Some(other) = docs.insert(system.doc.as_str(), system.id.as_str()) {
            diagnostics.push(format!(
                "systems {} and {} share doc {}",
                other, system.id, system.doc
            ));
        }
    }
}

fn check_components(model: &Model, diagnostics: &mut Diagnostics) {
    for component in model.components.values() {
        if !model.systems.contains_key(&component.system) {
            diagnostics.push(format!(
                "{} references unknown system {}",
                component.id, component.system
            ));
        }
        if !(1..=MAX_STAGE).contains(&component.stage) {
            diagnostics.push(format!(
                "{} has invalid stage {}",
                component.id, component.stage
            ));
        }
        if component.contract.trim().is_empty() {
            diagnostics.push(format!("{} has no contract", component.id));
        }
        if component.test.trim().is_empty() {
            diagnostics.push(format!("{} has no test seam", component.id));
        }
    }
}

fn check_stores(model: &Model, diagnostics: &mut Diagnostics) {
    for store in model.stores.values() {
        if !model.systems.contains_key(&store.system) {
            diagnostics.push(format!(
                "{} references unknown system {}",
                store.id, store.system
            ));
        }
        if !model.components.contains_key(&store.owner_component) {
            diagnostics.push(format!(
                "{} references unknown owner {}",
                store.id, store.owner_component
            ));
        } else if let Some(owner) = model.components.get(&store.owner_component)
            && owner.system != store.system
        {
            diagnostics.push(format!(
                "{} is in system {} but owner {} is in system {}",
                store.id, store.system, store.owner_component, owner.system
            ));
        }
        let owns: Vec<_> = model
            .edges
            .values()
            .filter(|edge| edge.kind == EdgeKind::Owns && edge.to == store.id)
            .collect();
        if owns.len() != 1 {
            diagnostics.push(format!(
                "{} must have exactly one owns edge, found {}",
                store.id,
                owns.len()
            ));
        } else if owns[0].from != store.owner_component {
            diagnostics.push(format!(
                "{} is owned by {} but owns edge starts at {}",
                store.id, store.owner_component, owns[0].from
            ));
        }
    }
}

fn check_edges(model: &Model, diagnostics: &mut Diagnostics) {
    for edge in model.edges.values() {
        let from_exists =
            model.components.contains_key(&edge.from) || model.stores.contains_key(&edge.from);
        let to_exists =
            model.components.contains_key(&edge.to) || model.stores.contains_key(&edge.to);
        if !from_exists {
            diagnostics.push(format!("{} references unknown from {}", edge.id, edge.from));
        }
        if !to_exists {
            diagnostics.push(format!("{} references unknown to {}", edge.id, edge.to));
        }
        if edge.from == edge.to {
            diagnostics.push(format!("{} is a self-loop", edge.id));
        }
        if edge.contract.trim().is_empty() {
            diagnostics.push(format!("{} has no contract", edge.id));
        }
        if let Some(gate) = &edge.gate
            && !model.components.contains_key(gate)
        {
            diagnostics.push(format!("{} references unknown gate {}", edge.id, gate));
        }
        if !(1..=MAX_STAGE).contains(&edge.stage) {
            diagnostics.push(format!("{} has invalid stage {}", edge.id, edge.stage));
        }

        match edge.kind {
            EdgeKind::Owns => {
                if !model.components.contains_key(&edge.from) {
                    diagnostics.push(format!(
                        "{} owns edge must start at a component, not {}",
                        edge.id, edge.from
                    ));
                }
                if !model.stores.contains_key(&edge.to) {
                    diagnostics.push(format!(
                        "{} owns edge must end at a store, not {}",
                        edge.id, edge.to
                    ));
                }
                if edge.mode != EdgeMode::Sync {
                    diagnostics.push(format!(
                        "{} owns edge must use sync mode, not {}",
                        edge.id,
                        edge.mode.as_str()
                    ));
                }
            }
            EdgeKind::ConfigRead if edge.mode != EdgeMode::Sync => {
                diagnostics.push(format!(
                    "{} config-read edge must use sync mode, not {}",
                    edge.id,
                    edge.mode.as_str()
                ));
            }
            _ => {}
        }
    }
}

fn endpoint_stage(model: &Model, id: &str) -> Option<u32> {
    model
        .components
        .get(id)
        .map(|component| component.stage)
        .or_else(|| {
            model
                .stores
                .get(id)
                .and_then(|store| model.components.get(&store.owner_component))
                .map(|component| component.stage)
        })
        .or_else(|| model.systems.get(id).map(|system| system.stage))
}

fn check_stages(model: &Model, diagnostics: &mut Diagnostics) {
    for edge in model.edges.values() {
        for (side, id) in [("from", &edge.from), ("to", &edge.to)] {
            if let Some(stage) = endpoint_stage(model, id)
                && stage > edge.stage
            {
                diagnostics.push(format!(
                    "{} stage {} is earlier than {} endpoint {} stage {}",
                    edge.id, edge.stage, side, id, stage
                ));
            }
        }
    }
}

fn check_render_ids(model: &Model, diagnostics: &mut Diagnostics) {
    let mut nodes = BTreeMap::new();
    for id in model.components.keys().chain(model.stores.keys()) {
        let rendered = node_id(id);
        if let Some(other) = nodes.insert(rendered.clone(), id) {
            diagnostics.push(format!(
                "rendered node id collision: {} and {} both render as {}",
                other, id, rendered
            ));
        }
    }

    for machine in model.machines.values() {
        let mut states = BTreeMap::new();
        for state in &machine.states {
            let rendered = state_id(&state.name);
            if let Some(other) = states.insert(rendered.clone(), &state.name) {
                diagnostics.push(format!(
                    "machine {} rendered state id collision: {} and {} both render as {}",
                    machine.id, other, state.name, rendered
                ));
            }
        }
    }
}

fn check_invariants(model: &Model, diagnostics: &mut Diagnostics) {
    for invariant in model.invariants.values() {
        if invariant.statement.trim().is_empty() {
            diagnostics.push(format!("{} has no statement", invariant.id));
        }
        if invariant.test.trim().is_empty() {
            diagnostics.push(format!("{} has no test", invariant.id));
        }
        for stage in &invariant.stages {
            if !(1..=MAX_STAGE).contains(stage) {
                diagnostics.push(format!("{} lists invalid stage {}", invariant.id, stage));
            }
        }
    }
}

fn check_machines(model: &Model, diagnostics: &mut Diagnostics) {
    for machine in model.machines.values() {
        let names: BTreeSet<&str> = machine
            .states
            .iter()
            .map(|state| state.name.as_str())
            .collect();
        if names.is_empty() {
            diagnostics.push(format!("machine {} has no states", machine.id));
        }
        if names.len() != machine.states.len() {
            diagnostics.push(format!("machine {} has duplicate states", machine.id));
        }

        let mut transitions = BTreeSet::new();
        for transition in &machine.transitions {
            if !names.contains(transition.from.as_str()) {
                diagnostics.push(format!(
                    "machine {} transition references unknown from {}",
                    machine.id, transition.from
                ));
            }
            if !names.contains(transition.to.as_str()) {
                diagnostics.push(format!(
                    "machine {} transition references unknown to {}",
                    machine.id, transition.to
                ));
            }
            if transition.event.trim().is_empty() {
                diagnostics.push(format!(
                    "machine {} has a transition without an event",
                    machine.id
                ));
            }
            if !transitions.insert((
                transition.from.as_str(),
                transition.event.as_str(),
                transition.to.as_str(),
            )) {
                diagnostics.push(format!(
                    "machine {} has duplicate transition {} -> {} on {}",
                    machine.id, transition.from, transition.to, transition.event
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::model::{
        Component, ComponentKind, Edge, EdgeMode, Layer, Store, StoreKind, System,
    };

    #[test]
    fn ownership_mismatch_is_reported() {
        let model = fixture();
        let errors = collect(&model);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("is owned by") && error.contains("owns edge starts at"))
        );
    }

    #[test]
    fn self_loop_is_reported() {
        let mut model = fixture();
        let edge = model.edges.get_mut("E1").expect("edge");
        edge.to = edge.from.clone();
        let errors = collect(&model);
        assert!(errors.iter().any(|error| error.contains("self-loop")));
    }

    #[test]
    fn stage_violation_is_reported() {
        let mut model = fixture();
        model.components.get_mut("C").expect("component").stage = 2;
        let errors = collect(&model);
        assert!(
            errors.iter().any(|error| {
                error.contains("E1 stage 1 is earlier than from endpoint C stage 2")
            })
        );
    }

    #[test]
    fn store_owner_in_other_system_is_reported() {
        let mut model = fixture();
        model.systems.insert("T".into(), system("T"));
        model.components.insert("D".into(), component("D", "T", 1));
        model.stores.get_mut("ST").expect("store").owner_component = "D".into();
        model.edges.get_mut("E1").expect("edge").from = "D".into();
        let errors = collect(&model);
        assert!(
            errors
                .iter()
                .any(|error| { error.contains("ST is in system S but owner D is in system T") })
        );
    }

    #[test]
    fn rendered_id_collision_is_reported() {
        let mut model = fixture();
        model
            .components
            .insert("C_1".into(), component("C_1", "S", 1));
        model
            .components
            .insert("C-1".into(), component("C-1", "S", 1));
        let errors = collect(&model);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("both render as n_c_1"))
        );
    }

    fn collect(model: &Model) -> Vec<String> {
        let mut diagnostics = Diagnostics::default();
        check_systems(model, &mut diagnostics);
        check_components(model, &mut diagnostics);
        check_stores(model, &mut diagnostics);
        check_edges(model, &mut diagnostics);
        check_stages(model, &mut diagnostics);
        check_invariants(model, &mut diagnostics);
        check_machines(model, &mut diagnostics);
        check_render_ids(model, &mut diagnostics);
        diagnostics.errors
    }

    fn system(id: &str) -> System {
        System {
            id: id.into(),
            name: id.into(),
            owner: "test".into(),
            layer: Layer::Leaves,
            stage: 1,
            doc: format!("{id}.md"),
            purpose: "purpose".into(),
            boundary: "boundary".into(),
            owned_state: Vec::new(),
            interfaces: Vec::new(),
            failure_semantics: Vec::new(),
            open_decisions: Vec::new(),
        }
    }

    fn component(id: &str, system: &str, stage: u32) -> Component {
        Component {
            id: id.into(),
            system: system.into(),
            name: id.into(),
            owner: "test".into(),
            kind: ComponentKind::Module,
            stage,
            state: Vec::new(),
            contract: "contract".into(),
            test: "test".into(),
        }
    }

    fn fixture() -> Model {
        let mut model = Model::default();
        model.systems.insert(
            "S".into(),
            System {
                id: "S".into(),
                name: "System".into(),
                owner: "test".into(),
                layer: Layer::Leaves,
                stage: 1,
                doc: "S.md".into(),
                purpose: "purpose".into(),
                boundary: "boundary".into(),
                owned_state: Vec::new(),
                interfaces: Vec::new(),
                failure_semantics: Vec::new(),
                open_decisions: Vec::new(),
            },
        );
        model.components.insert(
            "C".into(),
            Component {
                id: "C".into(),
                system: "S".into(),
                name: "Component".into(),
                owner: "test".into(),
                kind: ComponentKind::Module,
                stage: 1,
                state: Vec::new(),
                contract: "contract".into(),
                test: "test".into(),
            },
        );
        model.stores.insert(
            "ST".into(),
            Store {
                id: "ST".into(),
                system: "S".into(),
                kind: StoreKind::KeyFile,
                owner_component: "OTHER".into(),
                path: None,
            },
        );
        model.edges.insert(
            "E1".into(),
            Edge {
                id: "E1".into(),
                from: "C".into(),
                to: "ST".into(),
                kind: EdgeKind::Owns,
                mode: EdgeMode::Sync,
                contract: "owns".into(),
                gate: None,
                limit: None,
                stage: 1,
            },
        );
        model
    }
}
