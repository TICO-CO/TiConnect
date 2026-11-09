mod build;
mod cycles;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, bail};

use super::model::{EdgeKind, MAX_STAGE, Model};

pub fn validate(root: &Path, model: &Model) -> Result<()> {
    let mut diagnostics = Diagnostics::default();
    diagnostics.extend(model.load_errors.iter().cloned());

    check_systems(model, &mut diagnostics);
    check_components(model, &mut diagnostics);
    check_stores(model, &mut diagnostics);
    check_edges(model, &mut diagnostics);
    check_invariants(model, &mut diagnostics);
    check_machines(model, &mut diagnostics);

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

    fn collect(model: &Model) -> Vec<String> {
        let mut diagnostics = Diagnostics::default();
        check_systems(model, &mut diagnostics);
        check_components(model, &mut diagnostics);
        check_stores(model, &mut diagnostics);
        check_edges(model, &mut diagnostics);
        check_invariants(model, &mut diagnostics);
        check_machines(model, &mut diagnostics);
        diagnostics.errors
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
