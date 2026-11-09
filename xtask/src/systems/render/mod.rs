mod docs;
mod graphs;
mod table;
pub(crate) mod text;

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::systems::load::{DOCS_DIR, GENERATED_DIR};
use crate::systems::model::Model;

pub fn render_all(model: &Model) -> BTreeMap<PathBuf, String> {
    let mut outputs = BTreeMap::new();

    outputs.insert(
        PathBuf::from(format!("{GENERATED_DIR}/registry.md")),
        docs::render_registry(model),
    );
    outputs.insert(
        PathBuf::from(format!("{GENERATED_DIR}/systems.md")),
        graphs::render_systems(model),
    );
    outputs.insert(
        PathBuf::from(format!("{GENERATED_DIR}/build-deps.md")),
        graphs::render_build_deps(model),
    );
    outputs.insert(
        PathBuf::from(format!("{GENERATED_DIR}/runtime.md")),
        graphs::render_runtime(model),
    );
    outputs.insert(
        PathBuf::from(format!("{GENERATED_DIR}/trust.md")),
        graphs::render_trust(model),
    );
    outputs.insert(
        PathBuf::from(format!("{GENERATED_DIR}/state.md")),
        graphs::render_state(model),
    );
    outputs.insert(
        PathBuf::from(format!("{GENERATED_DIR}/stage-map.md")),
        docs::render_stage_map(model),
    );

    for system in model.systems.values() {
        outputs.insert(
            PathBuf::from(format!("{DOCS_DIR}/{}", system.doc)),
            docs::render_system_doc(model, system),
        );
        outputs.insert(
            PathBuf::from(format!(
                "{GENERATED_DIR}/components/{}.md",
                system.id.to_ascii_lowercase()
            )),
            graphs::render_component_graph(model, system),
        );
    }

    for machine in model.machines.values() {
        outputs.insert(
            PathBuf::from(format!("{GENERATED_DIR}/state-machines/{}.md", machine.id)),
            docs::render_machine(machine),
        );
    }

    outputs
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::systems::model::{
        BuildDep, Component, ComponentKind, Edge, EdgeKind, EdgeMode, GraphFragment, Invariant,
        Layer, Machine, MachineState, MachinesFile, Model, Store, StoreKind, System, Transition,
    };

    use super::*;

    #[test]
    fn render_all_covers_graphs_and_state_machines() {
        let outputs = render_all(&fixture());

        assert_eq!(outputs.len(), 12);
        assert!(text(&outputs, "docs/systems/01-a.md").contains("# S1 — System one"));
        assert!(text(&outputs, "docs/systems/generated/systems.md").contains("subgraph leaves"));
        assert!(text(&outputs, "docs/systems/generated/registry.md").contains("| `S1` |"));

        let component_graph = text(&outputs, "docs/systems/generated/components/s1.md");
        assert!(component_graph.contains("n_c1"));
        assert!(component_graph.contains("n_c2"));
        assert!(component_graph.contains("class n_c2 external;"));
        assert!(!component_graph.contains("UNKNOWN"));

        let machine = text(&outputs, "docs/systems/generated/state-machines/m.md");
        assert!(machine.contains("s_idle --> s_active: start"));

        let build = text(&outputs, "docs/systems/generated/build-deps.md");
        assert!(build.contains("ticonnect-core"));
        assert!(build.contains("ticonnect-net"));
    }

    fn text<'a>(outputs: &'a BTreeMap<PathBuf, String>, path: &str) -> &'a str {
        outputs
            .get(Path::new(path))
            .map(String::as_str)
            .unwrap_or_else(|| panic!("missing output {path}"))
    }

    fn fixture() -> Model {
        Model::from_fragment(
            GraphFragment {
                version: Some(1),
                systems: vec![
                    system("S1", "System one", "01-a.md"),
                    system("S2", "System two", "02-b.md"),
                ],
                components: vec![
                    component("C1", "S1", "Component one"),
                    component("C2", "S2", "Component two"),
                ],
                stores: vec![Store {
                    id: "ST".into(),
                    system: "S1".into(),
                    kind: StoreKind::KeyFile,
                    owner_component: "C1".into(),
                    path: None,
                }],
                edges: vec![
                    edge("E1", "C1", "ST", EdgeKind::Owns, EdgeMode::Sync),
                    edge("E2", "C1", "C2", EdgeKind::Calls, EdgeMode::Sync),
                ],
                invariants: vec![Invariant {
                    id: "INV".into(),
                    statement: "statement".into(),
                    test: "test".into(),
                    stages: vec![1],
                }],
                build_deps: vec![BuildDep {
                    from: "ticonnect-core".into(),
                    to: "ticonnect-net".into(),
                }],
                build_rules: Vec::new(),
            },
            MachinesFile {
                machines: vec![Machine {
                    id: "m".into(),
                    owner: "C1".into(),
                    doc: "01-a.md".into(),
                    states: vec![
                        MachineState {
                            name: "idle".into(),
                            description: None,
                        },
                        MachineState {
                            name: "active".into(),
                            description: None,
                        },
                    ],
                    transitions: vec![Transition {
                        from: "idle".into(),
                        event: "start".into(),
                        to: "active".into(),
                        guard: None,
                        action: None,
                    }],
                }],
            },
        )
    }

    fn system(id: &str, name: &str, doc: &str) -> System {
        System {
            id: id.into(),
            name: name.into(),
            owner: "test".into(),
            layer: Layer::Leaves,
            stage: 1,
            doc: doc.into(),
            purpose: "purpose".into(),
            boundary: "boundary".into(),
            owned_state: Vec::new(),
            interfaces: Vec::new(),
            failure_semantics: Vec::new(),
            open_decisions: Vec::new(),
        }
    }

    fn component(id: &str, system: &str, name: &str) -> Component {
        Component {
            id: id.into(),
            system: system.into(),
            name: name.into(),
            owner: "test".into(),
            kind: ComponentKind::Module,
            stage: 1,
            state: Vec::new(),
            contract: "contract".into(),
            test: "test".into(),
        }
    }

    fn edge(id: &str, from: &str, to: &str, kind: EdgeKind, mode: EdgeMode) -> Edge {
        Edge {
            id: id.into(),
            from: from.into(),
            to: to.into(),
            kind,
            mode,
            contract: "contract".into(),
            gate: None,
            limit: None,
            stage: 1,
        }
    }
}
