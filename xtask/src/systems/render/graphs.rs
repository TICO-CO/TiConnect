use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::systems::model::{Edge, EdgeKind, EdgeMode, Layer, Model, System};

use super::table::Table;
use super::text::{header, id, label, node_id};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphFilter {
    All,
    Runtime,
}

impl GraphFilter {
    fn matches(self, edge: &Edge) -> bool {
        match self {
            Self::All => true,
            Self::Runtime => edge.mode != EdgeMode::Build,
        }
    }
}

pub fn render_systems(model: &Model) -> String {
    let mut out = header();
    out.push_str("# System graph\n\n");
    out.push_str("```mermaid\n");
    out.push_str(&system_graph(model, GraphFilter::All));
    out.push_str("```\n\n");
    out.push_str("## Aggregated system edges\n\n");

    let mut table = Table::new(&["From", "To", "Kinds", "Modes", "Gates", "Edges"]);
    for edge in aggregate_system_edges(model, GraphFilter::All) {
        table.row([
            format!("`{}`", edge.from),
            format!("`{}`", edge.to),
            edge.kinds.into_iter().collect::<Vec<_>>().join(", "),
            edge.modes.into_iter().collect::<Vec<_>>().join(", "),
            edge.gates.into_iter().collect::<Vec<_>>().join(", "),
            edge.count.to_string(),
        ]);
    }
    out.push_str(&table.render());
    out
}

pub fn render_runtime(model: &Model) -> String {
    let mut out = header();
    out.push_str("# Runtime graph\n\n");
    out.push_str(
        "Cross-system runtime edges. Build-time dependencies live in [build-deps](build-deps.md).\n\n",
    );
    out.push_str("```mermaid\n");
    out.push_str(&system_graph(model, GraphFilter::Runtime));
    out.push_str("```\n\n");
    out.push_str("## Cross-system component edges\n\n");

    let mut table = Table::new(&[
        "From", "To", "Kind", "Mode", "Contract", "Gate", "Limit", "Stage",
    ]);
    for edge in model
        .edges
        .values()
        .filter(|edge| cross_system(model, edge))
    {
        table.row([
            format!("`{}`", edge.from),
            format!("`{}`", edge.to),
            edge.kind.as_str().to_string(),
            edge.mode.as_str().to_string(),
            edge.contract.clone(),
            edge.gate.clone().unwrap_or_default(),
            edge.limit.clone().unwrap_or_default(),
            edge.stage.to_string(),
        ]);
    }
    out.push_str(&table.render());
    out
}

pub fn render_build_deps(model: &Model) -> String {
    let mut out = header();
    out.push_str("# Build dependency graph\n\n");
    out.push_str("Generated from `docs/systems/graph/` and checked against `cargo metadata`.\n\n");
    out.push_str("```mermaid\nflowchart LR\n");
    for dep in &model.build_deps {
        let _ = writeln!(
            out,
            "  {}[\"{}\"] --> {}[\"{}\"]",
            id("p", &dep.from),
            label(&dep.from),
            id("p", &dep.to),
            label(&dep.to)
        );
    }
    out.push_str("```\n\n");

    let mut table = Table::new(&["From", "To"]);
    for dep in &model.build_deps {
        table.row([format!("`{}`", dep.from), format!("`{}`", dep.to)]);
    }
    out.push_str(&table.render());

    out.push_str("\n## Build rules\n\n");
    let mut rules = Table::new(&["Rule", "Packages", "Forbidden", "Forbidden prefix"]);
    for rule in &model.build_rules {
        rules.row([
            format!("`{}`", rule.id),
            rule.packages().collect::<Vec<_>>().join(", "),
            rule.forbid.join(", "),
            rule.forbid_prefix.clone().unwrap_or_default(),
        ]);
    }
    out.push_str(&rules.render());
    out
}

pub fn render_trust(model: &Model) -> String {
    let mut out = header();
    out.push_str("# Trust, policy, and consent gates\n\n");
    out.push_str("```mermaid\nflowchart LR\n");

    let graph_edges: BTreeSet<(&str, &str)> = model
        .edges
        .values()
        .filter_map(|edge| {
            edge.gate
                .as_deref()
                .map(|gate| (gate, edge.from.as_str()))
                .or_else(|| {
                    (edge.kind == EdgeKind::Gates).then_some((edge.from.as_str(), edge.to.as_str()))
                })
        })
        .collect();
    for (gate, target) in graph_edges {
        let _ = writeln!(
            out,
            "  g_{}[\"{}\"] -.->|gates| f_{}[\"{}\"]",
            node_id(gate),
            label(gate),
            node_id(target),
            label(target)
        );
    }
    out.push_str("```\n\n");

    let mut table = Table::new(&["Gate", "From", "To", "Edge", "Contract"]);
    for edge in model
        .edges
        .values()
        .filter(|edge| edge.gate.is_some() || edge.kind == EdgeKind::Gates)
    {
        table.row([
            format!("`{}`", edge.gate.as_deref().unwrap_or(edge.from.as_str())),
            format!("`{}`", edge.from),
            format!("`{}`", edge.to),
            format!("`{}`", edge.id),
            edge.contract.clone(),
        ]);
    }
    out.push_str(&table.render());
    out
}

pub fn render_state(model: &Model) -> String {
    let mut out = header();
    out.push_str("# State ownership\n\n");
    out.push_str("```mermaid\nflowchart LR\n");
    for store in model.stores.values() {
        let _ = writeln!(
            out,
            "  s_{}[\"{}\"] -->|owns| st_{}[(\"{}\")]",
            node_id(&store.owner_component),
            label(&store.owner_component),
            node_id(&store.id),
            label(&store.id)
        );
    }
    out.push_str("```\n\n");

    let mut table = Table::new(&["Store", "Kind", "Owner", "Path"]);
    for store in model.stores.values() {
        table.row([
            format!("`{}`", store.id),
            store.kind.as_str().to_string(),
            format!("`{}`", store.owner_component),
            store.path.clone().unwrap_or_default(),
        ]);
    }
    out.push_str(&table.render());
    out
}

pub fn render_component_graph(model: &Model, system: &System) -> String {
    let mut out = header();
    let _ = writeln!(out, "# {} component graph\n", system.name);
    let _ = writeln!(
        out,
        "Back to [{}](../../{}) and [registry](../registry.md).\n",
        system.name, system.doc
    );

    let local_components: BTreeSet<&str> = model
        .components_in(&system.id)
        .map(|component| component.id.as_str())
        .collect();
    let local_stores: BTreeSet<&str> = model
        .stores_in(&system.id)
        .map(|store| store.id.as_str())
        .collect();
    let local: BTreeSet<&str> = local_components
        .iter()
        .copied()
        .chain(local_stores.iter().copied())
        .collect();

    let incident: Vec<&Edge> = model
        .edges
        .values()
        .filter(|edge| local.contains(edge.from.as_str()) || local.contains(edge.to.as_str()))
        .collect();
    let mut external = BTreeSet::new();
    for edge in &incident {
        if !local.contains(edge.from.as_str()) {
            external.insert(edge.from.as_str());
        }
        if !local.contains(edge.to.as_str()) {
            external.insert(edge.to.as_str());
        }
    }

    out.push_str("```mermaid\nflowchart LR\n");
    let _ = writeln!(out, "  subgraph sys[\"{}\"]", label(&system.name));
    for component in model.components_in(&system.id) {
        let _ = writeln!(
            out,
            "    {}[\"{}\"]",
            node_id(&component.id),
            label(&format!("{}<br/>{}", component.id, component.name))
        );
    }
    for store in model.stores_in(&system.id) {
        let _ = writeln!(
            out,
            "    {}[(\"{}\")]",
            node_id(&store.id),
            label(&format!("{} ({})", store.id, store.kind.as_str()))
        );
    }
    out.push_str("  end\n");

    if !external.is_empty() {
        out.push_str("  subgraph ext[\"External\"]\n");
        for item in &external {
            let _ = writeln!(
                out,
                "    {}[\"{}\"]",
                node_id(item),
                label(&format!(
                    "{}<br/>{}",
                    item,
                    model.node_system(item).unwrap_or("UNKNOWN")
                ))
            );
        }
        out.push_str("  end\n");
    }

    for edge in &incident {
        let _ = writeln!(
            out,
            "  {} {}|{}| {}",
            node_id(&edge.from),
            arrow(edge),
            label(&format!(
                "{} / {}{}: {}",
                edge.kind.as_str(),
                edge.mode.as_str(),
                edge.gate
                    .as_deref()
                    .map(|gate| format!(" gate:{gate}"))
                    .unwrap_or_default(),
                edge.contract
            )),
            node_id(&edge.to)
        );
    }

    if !external.is_empty() {
        out.push_str("  classDef external fill:#eee,stroke:#999,stroke-dasharray: 4 3;\n");
        let names = external
            .iter()
            .map(|item| node_id(item))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "  class {names} external;");
    }
    out.push_str("```\n\n");

    let mut table = Table::new(&[
        "From", "Kind", "Mode", "To", "Contract", "Gate", "Limit", "Stage",
    ]);
    for edge in incident {
        table.row([
            format!("`{}`", edge.from),
            edge.kind.as_str().to_string(),
            edge.mode.as_str().to_string(),
            format!("`{}`", edge.to),
            edge.contract.clone(),
            edge.gate.clone().unwrap_or_default(),
            edge.limit.clone().unwrap_or_default(),
            edge.stage.to_string(),
        ]);
    }
    out.push_str(&table.render());
    out
}

struct Aggregate {
    from: String,
    to: String,
    kinds: BTreeSet<&'static str>,
    modes: BTreeSet<&'static str>,
    gates: BTreeSet<String>,
    count: usize,
}

fn aggregate_system_edges(model: &Model, filter: GraphFilter) -> Vec<Aggregate> {
    let mut map: BTreeMap<(String, String), Aggregate> = BTreeMap::new();
    for edge in model.edges.values().filter(|edge| filter.matches(edge)) {
        let (Some(from), Some(to)) = (model.node_system(&edge.from), model.node_system(&edge.to))
        else {
            continue;
        };
        if from == to {
            continue;
        }
        let entry = map
            .entry((from.to_string(), to.to_string()))
            .or_insert_with(|| Aggregate {
                from: from.to_string(),
                to: to.to_string(),
                kinds: BTreeSet::new(),
                modes: BTreeSet::new(),
                gates: BTreeSet::new(),
                count: 0,
            });
        entry.kinds.insert(edge.kind.as_str());
        entry.modes.insert(edge.mode.as_str());
        if let Some(gate) = &edge.gate {
            entry.gates.insert(gate.clone());
        }
        entry.count += 1;
    }
    map.into_values().collect()
}

fn system_graph(model: &Model, filter: GraphFilter) -> String {
    let mut out = String::from("flowchart LR\n");
    for layer in Layer::ORDER {
        let systems: Vec<&System> = model
            .systems
            .values()
            .filter(|system| system.layer == layer)
            .collect();
        if systems.is_empty() {
            continue;
        }
        let _ = writeln!(out, "  subgraph {}[\"{}\"]", layer.as_str(), layer.title());
        for system in systems {
            let _ = writeln!(
                out,
                "    {}[\"{}<br/>{} {}\"]",
                node_id(&system.id),
                label(&system.id),
                label(&system.name),
                system.stage
            );
        }
        out.push_str("  end\n");
    }

    for aggregated in aggregate_system_edges(model, filter) {
        let mut edge_label = format!(
            "{} / {}",
            aggregated.kinds.into_iter().collect::<Vec<_>>().join(", "),
            aggregated.modes.into_iter().collect::<Vec<_>>().join(", ")
        );
        if !aggregated.gates.is_empty() {
            let _ = write!(
                edge_label,
                " gate:{}",
                aggregated.gates.into_iter().collect::<Vec<_>>().join(", ")
            );
        }
        let _ = writeln!(
            out,
            "  {} -->|{}| {}",
            node_id(&aggregated.from),
            label(&edge_label),
            node_id(&aggregated.to)
        );
    }
    out
}

fn cross_system(model: &Model, edge: &Edge) -> bool {
    match (model.node_system(&edge.from), model.node_system(&edge.to)) {
        (Some(from), Some(to)) => from != to,
        _ => false,
    }
}

fn arrow(edge: &Edge) -> &'static str {
    match (edge.kind, edge.mode) {
        (EdgeKind::ConfigRead, _) => "-.->",
        (_, EdgeMode::Async | EdgeMode::Event) => "-.->",
        (_, EdgeMode::Build) => "==>",
        _ => "-->",
    }
}
