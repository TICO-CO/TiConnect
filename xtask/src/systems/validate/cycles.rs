use std::collections::{BTreeMap, BTreeSet};

use super::super::model::Model;

pub fn validate(model: &Model) -> Vec<String> {
    let mut nodes = BTreeSet::new();
    let mut adjacency: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for edge in model.edges.values() {
        nodes.insert(edge.from.clone());
        nodes.insert(edge.to.clone());
        adjacency
            .entry(edge.from.clone())
            .or_default()
            .push(edge.to.clone());
    }

    let mut errors = Vec::new();
    for component in strongly_connected(&nodes, &adjacency) {
        let members: BTreeSet<&str> = component.iter().map(String::as_str).collect();
        let self_loop = component.len() == 1
            && adjacency
                .get(&component[0])
                .into_iter()
                .flatten()
                .any(|to| to == &component[0]);
        if component.len() == 1 && !self_loop {
            continue;
        }
        for edge in model.edges.values() {
            if members.contains(edge.from.as_str())
                && members.contains(edge.to.as_str())
                && !edge.mode.is_async()
            {
                errors.push(format!(
                    "runtime cycle [{}] has non-async edge {}: {} -> {}",
                    component.join(", "),
                    edge.id,
                    edge.from,
                    edge.to
                ));
            }
        }
    }
    errors
}

fn strongly_connected(
    nodes: &BTreeSet<String>,
    adjacency: &BTreeMap<String, Vec<String>>,
) -> Vec<Vec<String>> {
    struct Tarjan<'a> {
        index: usize,
        indices: BTreeMap<String, usize>,
        low: BTreeMap<String, usize>,
        stack: Vec<String>,
        on_stack: BTreeSet<String>,
        adjacency: &'a BTreeMap<String, Vec<String>>,
        result: Vec<Vec<String>>,
    }

    fn connect(node: &str, state: &mut Tarjan<'_>) {
        state.indices.insert(node.to_string(), state.index);
        state.low.insert(node.to_string(), state.index);
        state.index += 1;
        state.stack.push(node.to_string());
        state.on_stack.insert(node.to_string());

        if let Some(nexts) = state.adjacency.get(node) {
            for next in nexts {
                if !state.indices.contains_key(next) {
                    connect(next, state);
                    let low = state.low[node].min(state.low[next]);
                    state.low.insert(node.to_string(), low);
                } else if state.on_stack.contains(next) {
                    let low = state.low[node].min(state.indices[next]);
                    state.low.insert(node.to_string(), low);
                }
            }
        }

        if state.low[node] == state.indices[node] {
            let mut component = Vec::new();
            loop {
                let item = state.stack.pop().expect("tarjan stack is non-empty");
                state.on_stack.remove(&item);
                component.push(item.clone());
                if item == node {
                    break;
                }
            }
            state.result.push(component);
        }
    }

    let mut state = Tarjan {
        index: 0,
        indices: BTreeMap::new(),
        low: BTreeMap::new(),
        stack: Vec::new(),
        on_stack: BTreeSet::new(),
        adjacency,
        result: Vec::new(),
    };
    for node in nodes {
        if !state.indices.contains_key(node) {
            connect(node, &mut state);
        }
    }
    state.result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::model::{Edge, EdgeKind, EdgeMode};

    #[test]
    fn sync_edge_in_cycle_is_reported() {
        let mut model = Model::default();
        model
            .edges
            .insert("E1".into(), edge("E1", "A", "B", EdgeMode::Async));
        model
            .edges
            .insert("E2".into(), edge("E2", "B", "A", EdgeMode::Sync));
        let errors = validate(&model);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("non-async edge E2"))
        );
    }

    #[test]
    fn async_cycle_is_allowed() {
        let mut model = Model::default();
        model
            .edges
            .insert("E1".into(), edge("E1", "A", "B", EdgeMode::Async));
        model
            .edges
            .insert("E2".into(), edge("E2", "B", "A", EdgeMode::Event));
        assert!(validate(&model).is_empty());
    }

    fn edge(id: &str, from: &str, to: &str, mode: EdgeMode) -> Edge {
        Edge {
            id: id.into(),
            from: from.into(),
            to: to.into(),
            kind: EdgeKind::Calls,
            mode,
            contract: "contract".into(),
            gate: None,
            limit: None,
            stage: 1,
        }
    }
}
