use std::fmt::Write as _;

use crate::systems::model::{MAX_STAGE, Machine, Model, System};

use super::table::Table;
use super::text::{header, label, state_id};

pub fn render_registry(model: &Model) -> String {
    let mut out = header();
    out.push_str("# Component registry\n\n");

    let mut systems = Table::new(&["System", "Name", "Owner", "Layer", "Stage", "Doc"]);
    for system in model.systems.values() {
        systems.row([
            format!("`{}`", system.id),
            system.name.clone(),
            format!("`{}`", system.owner),
            format!("`{}`", system.layer.as_str()),
            system.stage.to_string(),
            format!("[{}](../{})", system.doc, system.doc),
        ]);
    }
    out.push_str(&systems.render());

    out.push_str("\n## Components\n\n");
    let mut components = Table::new(&[
        "ID", "System", "Name", "Kind", "Owner", "Stage", "State", "Contract", "Test",
    ]);
    for component in model.components.values() {
        components.row([
            format!("`{}`", component.id),
            format!("`{}`", component.system),
            component.name.clone(),
            format!("`{}`", component.kind.as_str()),
            format!("`{}`", component.owner),
            component.stage.to_string(),
            component.state.join(", "),
            component.contract.clone(),
            component.test.clone(),
        ]);
    }
    out.push_str(&components.render());

    out.push_str("\n## Stores\n\n");
    let mut stores = Table::new(&["Store", "System", "Kind", "Owner", "Path"]);
    for store in model.stores.values() {
        stores.row([
            format!("`{}`", store.id),
            format!("`{}`", store.system),
            format!("`{}`", store.kind.as_str()),
            format!("`{}`", store.owner_component),
            store.path.clone().unwrap_or_default(),
        ]);
    }
    out.push_str(&stores.render());

    out.push_str("\n## Invariants\n\n");
    let mut invariants = Table::new(&["ID", "Statement", "Test", "Stages"]);
    for invariant in model.invariants.values() {
        invariants.row([
            format!("`{}`", invariant.id),
            invariant.statement.clone(),
            invariant.test.clone(),
            invariant
                .stages
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        ]);
    }
    out.push_str(&invariants.render());
    out
}

pub fn render_stage_map(model: &Model) -> String {
    let mut out = header();
    out.push_str("# Stage component map\n\n");
    for stage in 1..=MAX_STAGE {
        let _ = writeln!(out, "## Stage {stage}\n");
        let mut any = false;
        for component in model
            .components
            .values()
            .filter(|component| component.stage == stage)
        {
            any = true;
            let _ = writeln!(
                out,
                "- `{}` — {} ({})",
                component.id, component.name, component.system
            );
        }
        if !any {
            out.push_str("- None\n");
        }
        out.push('\n');
    }
    out
}

pub fn render_system_doc(model: &Model, system: &System) -> String {
    let mut out = header();
    let _ = writeln!(out, "# {} — {}\n", system.id, system.name);
    let _ = writeln!(out, "- Owner: `{}`", system.owner);
    let _ = writeln!(out, "- Stage: {}", system.stage);
    let _ = writeln!(out, "- Layer: `{}`", system.layer.as_str());
    let _ = writeln!(
        out,
        "- Component graph: [generated/components/{}.md](generated/components/{}.md)\n",
        system.id.to_ascii_lowercase(),
        system.id.to_ascii_lowercase()
    );

    out.push_str("## Purpose\n\n");
    out.push_str(&system.purpose);
    out.push_str("\n\n## Boundary\n\n");
    out.push_str(&system.boundary);
    out.push_str("\n\n## Owned state\n\n");
    push_list(&mut out, &system.owned_state);

    out.push_str("\n## Components\n\n");
    let mut components = Table::new(&["ID", "Name", "Kind", "Owner", "State", "Contract", "Test"]);
    for component in model.components_in(&system.id) {
        components.row([
            format!("`{}`", component.id),
            component.name.clone(),
            format!("`{}`", component.kind.as_str()),
            format!("`{}`", component.owner),
            component.state.join(", "),
            component.contract.clone(),
            component.test.clone(),
        ]);
    }
    out.push_str(&components.render());

    out.push_str("\n## Interfaces\n\n");
    push_list(&mut out, &system.interfaces);

    out.push_str("\n## Relations\n\n");
    let mut relations = Table::new(&["From", "Kind", "Mode", "To", "Contract", "Gate", "Limit"]);
    for edge in model.edges_incident(&system.id) {
        relations.row([
            format!("`{}`", edge.from),
            format!("`{}`", edge.kind.as_str()),
            format!("`{}`", edge.mode.as_str()),
            format!("`{}`", edge.to),
            edge.contract.clone(),
            edge.gate.clone().unwrap_or_default(),
            edge.limit.clone().unwrap_or_default(),
        ]);
    }
    out.push_str(&relations.render());

    out.push_str("\n## Invariants\n\n");
    let relevant: Vec<_> = model
        .invariants
        .values()
        .filter(|invariant| invariant.stages.is_empty() || invariant.stages.contains(&system.stage))
        .collect();
    if relevant.is_empty() {
        out.push_str("- None\n");
    } else {
        for invariant in relevant {
            let _ = writeln!(
                out,
                "- `{}` — {} Test: {}",
                invariant.id, invariant.statement, invariant.test
            );
        }
    }

    out.push_str("\n## Failure semantics\n\n");
    push_list(&mut out, &system.failure_semantics);

    out.push_str("\n## Open decisions\n\n");
    push_list(&mut out, &system.open_decisions);
    out
}

pub fn render_machine(machine: &Machine) -> String {
    let mut out = header();
    let _ = writeln!(out, "# {} state machine\n", machine.id);
    let _ = writeln!(out, "- Owner: `{}`", machine.owner);
    let _ = writeln!(out, "- Contract: `{}`\n", machine.doc);

    out.push_str("```mermaid\nstateDiagram-v2\n");
    for state in &machine.states {
        let state_label = state
            .description
            .as_deref()
            .map(|description| format!("{}: {description}", state.name))
            .unwrap_or_else(|| state.name.clone());
        let _ = writeln!(
            out,
            "  state \"{}\" as {}",
            label(&state_label),
            state_id(&state.name)
        );
    }
    for transition in &machine.transitions {
        let mut transition_label = transition.event.clone();
        if let Some(guard) = &transition.guard {
            let _ = write!(transition_label, " [{guard}]");
        }
        if let Some(action) = &transition.action {
            let _ = write!(transition_label, " / {action}");
        }
        let _ = writeln!(
            out,
            "  {} --> {}: {}",
            state_id(&transition.from),
            state_id(&transition.to),
            label(&transition_label)
        );
    }
    out.push_str("```\n\n");

    let mut table = Table::new(&["From", "Event", "To", "Guard", "Action"]);
    for transition in &machine.transitions {
        table.row([
            format!("`{}`", transition.from),
            transition.event.clone(),
            format!("`{}`", transition.to),
            transition.guard.clone().unwrap_or_default(),
            transition.action.clone().unwrap_or_default(),
        ]);
    }
    out.push_str(&table.render());
    out
}

fn push_list(out: &mut String, values: &[String]) {
    if values.is_empty() {
        out.push_str("- None\n");
    } else {
        for value in values {
            let _ = writeln!(out, "- {value}");
        }
    }
}
