mod docs;
mod graphs;
mod table;
mod text;

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
