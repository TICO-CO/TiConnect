use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::model::{GraphFragment, MachinesFile, Model};

pub const GRAPH_DIR: &str = "docs/systems/graph";
pub const MACHINES_FILE: &str = "docs/systems/state-machines.toml";
pub const DOCS_DIR: &str = "docs/systems";
pub const GENERATED_DIR: &str = "docs/systems/generated";

pub fn load(root: &Path) -> Result<Model> {
    let graph_dir = root.join(GRAPH_DIR);
    let mut fragment = GraphFragment::default();

    let mut files = fs::read_dir(&graph_dir)
        .with_context(|| format!("read {}", graph_dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    files.sort_by_key(|entry| entry.file_name());

    for entry in files {
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "toml") {
            continue;
        }
        let part: GraphFragment = toml::from_str(
            &fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?,
        )
        .with_context(|| format!("parse {}", path.display()))?;
        merge_fragment(&mut fragment, part);
    }

    if fragment.version != Some(1) {
        bail!(
            "unsupported graph version {}; expected 1",
            fragment.version.unwrap_or(0)
        );
    }

    let machines_path = root.join(MACHINES_FILE);
    let machines: MachinesFile = toml::from_str(
        &fs::read_to_string(&machines_path)
            .with_context(|| format!("read {}", machines_path.display()))?,
    )
    .with_context(|| format!("parse {}", machines_path.display()))?;

    Ok(Model::from_fragment(fragment, machines))
}

pub fn merge_fragment(into: &mut GraphFragment, from: GraphFragment) {
    if from.version.is_some() {
        into.version = from.version;
    }
    into.systems.extend(from.systems);
    into.components.extend(from.components);
    into.stores.extend(from.stores);
    into.edges.extend(from.edges);
    into.invariants.extend(from.invariants);
    into.build_deps.extend(from.build_deps);
    into.build_rules.extend(from.build_rules);
}

pub fn files_under(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !root.is_dir() {
        return Ok(files);
    }
    for entry in fs::read_dir(root).with_context(|| format!("read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(files_under(&path)?);
        } else if path.extension().is_some_and(|extension| extension == "md") {
            files.push(path);
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::model::{BuildDep, GraphFragment, System};

    #[test]
    fn merge_preserves_version_and_appends() {
        let mut target = GraphFragment {
            version: Some(1),
            systems: vec![system("A")],
            ..GraphFragment::default()
        };
        let source = GraphFragment {
            version: Some(2),
            systems: vec![system("B")],
            build_deps: vec![BuildDep {
                from: "a".into(),
                to: "b".into(),
            }],
            ..GraphFragment::default()
        };
        merge_fragment(&mut target, source);
        assert_eq!(target.version, Some(2));
        assert_eq!(target.systems.len(), 2);
        assert_eq!(target.build_deps.len(), 1);
    }

    fn system(id: &str) -> System {
        System {
            id: id.into(),
            name: id.into(),
            owner: "test".into(),
            layer: crate::systems::model::Layer::Leaves,
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
}
