use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

pub const MAX_STAGE: u32 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Layer {
    Leaves,
    Domain,
    Services,
    Daemon,
    Clients,
    Assurance,
}

impl Layer {
    pub const ORDER: [Self; 6] = [
        Self::Leaves,
        Self::Domain,
        Self::Services,
        Self::Daemon,
        Self::Clients,
        Self::Assurance,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Leaves => "leaves",
            Self::Domain => "domain",
            Self::Services => "services",
            Self::Daemon => "daemon",
            Self::Clients => "clients",
            Self::Assurance => "assurance",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Leaves => "Leaves",
            Self::Domain => "Domain",
            Self::Services => "Services",
            Self::Daemon => "Daemon",
            Self::Clients => "Clients",
            Self::Assurance => "Assurance",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComponentKind {
    Module,
    Crate,
    Process,
    Store,
    External,
}

impl ComponentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::Crate => "crate",
            Self::Process => "process",
            Self::Store => "store",
            Self::External => "external",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StoreKind {
    TomlFile,
    KeyFile,
    SqliteTable,
    ContentDirectory,
}

impl StoreKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TomlFile => "toml-file",
            Self::KeyFile => "key-file",
            Self::SqliteTable => "sqlite-table",
            Self::ContentDirectory => "content-directory",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeKind {
    Owns,
    Calls,
    Invokes,
    Emits,
    Consumes,
    Persists,
    ConfigRead,
    Gates,
    Speaks,
    Observes,
    BuildDep,
}

impl EdgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owns => "owns",
            Self::Calls => "calls",
            Self::Invokes => "invokes",
            Self::Emits => "emits",
            Self::Consumes => "consumes",
            Self::Persists => "persists",
            Self::ConfigRead => "config-read",
            Self::Gates => "gates",
            Self::Speaks => "speaks",
            Self::Observes => "observes",
            Self::BuildDep => "build-dep",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeMode {
    Sync,
    Async,
    Event,
    Build,
}

impl EdgeMode {
    pub fn is_async(self) -> bool {
        matches!(self, Self::Async | Self::Event)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sync => "sync",
            Self::Async => "async",
            Self::Event => "event",
            Self::Build => "build",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct System {
    pub id: String,
    pub name: String,
    pub owner: String,
    pub layer: Layer,
    pub stage: u32,
    pub doc: String,
    pub purpose: String,
    pub boundary: String,
    #[serde(default)]
    pub owned_state: Vec<String>,
    #[serde(default)]
    pub interfaces: Vec<String>,
    #[serde(default)]
    pub failure_semantics: Vec<String>,
    #[serde(default)]
    pub open_decisions: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Component {
    pub id: String,
    pub system: String,
    pub name: String,
    pub owner: String,
    pub kind: ComponentKind,
    pub stage: u32,
    #[serde(default)]
    pub state: Vec<String>,
    pub contract: String,
    pub test: String,
}

#[derive(Debug, Deserialize)]
pub struct Store {
    pub id: String,
    pub system: String,
    pub kind: StoreKind,
    pub owner_component: String,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Edge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    pub mode: EdgeMode,
    pub contract: String,
    #[serde(default)]
    pub gate: Option<String>,
    #[serde(default)]
    pub limit: Option<String>,
    pub stage: u32,
}

#[derive(Debug, Deserialize)]
pub struct Invariant {
    pub id: String,
    pub statement: String,
    pub test: String,
    #[serde(default)]
    pub stages: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub struct BuildDep {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Deserialize)]
pub struct BuildRule {
    pub id: String,
    #[serde(default)]
    pub package: Option<String>,
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub forbid: Vec<String>,
    #[serde(default)]
    pub forbid_prefix: Option<String>,
}

impl BuildRule {
    pub fn packages(&self) -> impl Iterator<Item = &str> {
        self.package
            .iter()
            .map(String::as_str)
            .chain(self.packages.iter().map(String::as_str))
    }

    pub fn forbids(&self, dependency: &str) -> bool {
        self.forbid.iter().any(|candidate| candidate == dependency)
            || self
                .forbid_prefix
                .as_deref()
                .is_some_and(|prefix| dependency.starts_with(prefix))
    }
}

#[derive(Debug, Deserialize)]
pub struct MachineState {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Transition {
    pub from: String,
    pub event: String,
    pub to: String,
    #[serde(default)]
    pub guard: Option<String>,
    #[serde(default)]
    pub action: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Machine {
    pub id: String,
    pub owner: String,
    pub doc: String,
    pub states: Vec<MachineState>,
    pub transitions: Vec<Transition>,
}

#[derive(Debug, Deserialize, Default)]
pub struct GraphFragment {
    pub version: Option<u32>,
    #[serde(default)]
    pub systems: Vec<System>,
    #[serde(default)]
    pub components: Vec<Component>,
    #[serde(default)]
    pub stores: Vec<Store>,
    #[serde(default)]
    pub edges: Vec<Edge>,
    #[serde(default)]
    pub invariants: Vec<Invariant>,
    #[serde(default)]
    pub build_deps: Vec<BuildDep>,
    #[serde(default)]
    pub build_rules: Vec<BuildRule>,
}

#[derive(Debug, Deserialize, Default)]
pub struct MachinesFile {
    #[serde(default)]
    pub machines: Vec<Machine>,
}

#[derive(Debug, Default)]
pub struct Model {
    pub systems: BTreeMap<String, System>,
    pub components: BTreeMap<String, Component>,
    pub stores: BTreeMap<String, Store>,
    pub edges: BTreeMap<String, Edge>,
    pub invariants: BTreeMap<String, Invariant>,
    pub machines: BTreeMap<String, Machine>,
    pub build_deps: BTreeSet<BuildDep>,
    pub build_rules: Vec<BuildRule>,
    pub load_errors: Vec<String>,
}

impl Model {
    pub fn from_fragment(fragment: GraphFragment, machines: MachinesFile) -> Self {
        let mut model = Self {
            build_rules: fragment.build_rules,
            ..Self::default()
        };
        for value in fragment.systems {
            if model.components.contains_key(&value.id) || model.stores.contains_key(&value.id) {
                model
                    .load_errors
                    .push(format!("node id collision: {}", value.id));
            }
            insert_unique(
                &mut model.systems,
                value.id.clone(),
                value,
                "system",
                &mut model.load_errors,
            );
        }
        for value in fragment.components {
            if model.systems.contains_key(&value.id) || model.stores.contains_key(&value.id) {
                model
                    .load_errors
                    .push(format!("node id collision: {}", value.id));
            }
            insert_unique(
                &mut model.components,
                value.id.clone(),
                value,
                "component",
                &mut model.load_errors,
            );
        }
        for value in fragment.stores {
            if model.systems.contains_key(&value.id) || model.components.contains_key(&value.id) {
                model
                    .load_errors
                    .push(format!("node id collision: {}", value.id));
            }
            insert_unique(
                &mut model.stores,
                value.id.clone(),
                value,
                "store",
                &mut model.load_errors,
            );
        }
        for value in fragment.edges {
            insert_unique(
                &mut model.edges,
                value.id.clone(),
                value,
                "edge",
                &mut model.load_errors,
            );
        }
        for value in fragment.invariants {
            insert_unique(
                &mut model.invariants,
                value.id.clone(),
                value,
                "invariant",
                &mut model.load_errors,
            );
        }
        for value in machines.machines {
            insert_unique(
                &mut model.machines,
                value.id.clone(),
                value,
                "machine",
                &mut model.load_errors,
            );
        }
        for value in fragment.build_deps {
            if !model.build_deps.insert(value.clone()) {
                model.load_errors.push(format!(
                    "duplicate build dependency: {} -> {}",
                    value.from, value.to
                ));
            }
        }
        model
    }

    pub fn node_system(&self, id: &str) -> Option<&str> {
        self.components
            .get(id)
            .map(|component| component.system.as_str())
            .or_else(|| self.stores.get(id).map(|store| store.system.as_str()))
            .or_else(|| self.systems.get(id).map(|system| system.id.as_str()))
    }

    pub fn components_in<'a>(&'a self, system: &'a str) -> impl Iterator<Item = &'a Component> {
        self.components
            .values()
            .filter(move |component| component.system == system)
    }

    pub fn stores_in<'a>(&'a self, system: &'a str) -> impl Iterator<Item = &'a Store> {
        self.stores
            .values()
            .filter(move |store| store.system == system)
    }

    pub fn edges_incident<'a>(&'a self, system: &'a str) -> impl Iterator<Item = &'a Edge> {
        self.edges.values().filter(move |edge| {
            self.node_system(&edge.from) == Some(system)
                || self.node_system(&edge.to) == Some(system)
        })
    }
}

fn insert_unique<T>(
    map: &mut BTreeMap<String, T>,
    id: String,
    value: T,
    kind: &str,
    errors: &mut Vec<String>,
) {
    if map.insert(id.clone(), value).is_some() {
        errors.push(format!("duplicate {kind} id: {id}"));
    }
}

#[cfg(test)]
mod tests;
