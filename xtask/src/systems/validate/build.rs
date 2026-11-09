use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use cargo_metadata::{DependencyKind, MetadataCommand};

use super::super::model::Model;

pub fn validate(root: &Path, model: &Model) -> Vec<String> {
    let mut errors = Vec::new();
    let actual = match actual_dependencies(root) {
        Ok(actual) => actual,
        Err(error) => return vec![error],
    };

    let mut declared = BTreeSet::new();
    for dep in &model.build_deps {
        declared.insert((normalize(&dep.from), normalize(&dep.to)));
    }

    for dep in &model.build_deps {
        let from = normalize(&dep.from);
        let to = normalize(&dep.to);
        let Some(deps) = actual.get(&from) else {
            errors.push(format!("build dependency source not found: {}", dep.from));
            continue;
        };
        if !deps.contains(&to) {
            errors.push(format!(
                "declared build dependency does not exist: {} -> {}",
                dep.from, dep.to
            ));
        }
    }

    for (package, deps) in &actual {
        for dep in deps {
            if !declared.contains(&(package.clone(), dep.clone())) {
                errors.push(format!(
                    "workspace dependency is missing from build-deps.toml: {} -> {}",
                    package, dep
                ));
            }
        }
    }

    check_rules(model, &actual, &mut errors);
    check_cycles(&actual, &mut errors);

    errors
}

fn actual_dependencies(root: &Path) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let metadata = MetadataCommand::new()
        .no_deps()
        .current_dir(root.to_path_buf())
        .exec()
        .map_err(|error| format!("cargo metadata failed: {error}"))?;

    let workspace: BTreeSet<_> = metadata.workspace_members.iter().collect();
    let workspace_names: BTreeSet<_> = metadata
        .packages
        .iter()
        .filter(|package| workspace.contains(&package.id))
        .map(|package| normalize(package.name.as_ref()))
        .filter(|name| name != "xtask")
        .collect();

    let mut packages = BTreeMap::new();
    for package in metadata.packages {
        if !workspace.contains(&package.id) {
            continue;
        }
        let name = normalize(package.name.as_ref());
        if name == "xtask" {
            continue;
        }
        let deps = package
            .dependencies
            .into_iter()
            .filter(|dependency| {
                matches!(
                    dependency.kind,
                    DependencyKind::Normal | DependencyKind::Build
                )
            })
            .map(|dependency| normalize(&dependency.name))
            .filter(|dependency| workspace_names.contains(dependency))
            .collect();
        packages.insert(name, deps);
    }
    Ok(packages)
}

fn check_rules(
    model: &Model,
    actual: &BTreeMap<String, BTreeSet<String>>,
    errors: &mut Vec<String>,
) {
    let mut ids = BTreeSet::new();
    for rule in &model.build_rules {
        if !ids.insert(rule.id.as_str()) {
            errors.push(format!("duplicate build rule id: {}", rule.id));
        }
        if rule
            .forbid_prefix
            .as_deref()
            .is_some_and(|prefix| prefix.trim().is_empty())
        {
            errors.push(format!("rule {} has an empty forbidden prefix", rule.id));
        }
        let mut package_count = 0;
        for package in rule.packages() {
            let package = normalize(package);
            if !actual.contains_key(&package) {
                errors.push(format!(
                    "rule {} references unknown package {}",
                    rule.id, package
                ));
                continue;
            }
            package_count += 1;
            if rule.forbid.is_empty() && rule.forbid_prefix.is_none() {
                errors.push(format!("rule {} has no forbidden dependencies", rule.id));
                continue;
            }
            for dep in &actual[&package] {
                if rule.forbids(dep) {
                    errors.push(format!(
                        "build rule {} forbids {} -> {}",
                        rule.id, package, dep
                    ));
                }
            }
        }
        if package_count == 0 {
            errors.push(format!("rule {} does not reference a package", rule.id));
        }
    }
}

fn check_cycles(actual: &BTreeMap<String, BTreeSet<String>>, errors: &mut Vec<String>) {
    let mut state = BTreeMap::new();
    let mut stack = Vec::new();
    for package in actual.keys() {
        detect_cycle(package, actual, &mut state, &mut stack, errors);
    }
}

fn detect_cycle(
    package: &str,
    graph: &BTreeMap<String, BTreeSet<String>>,
    state: &mut BTreeMap<String, u8>,
    stack: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    match state.get(package).copied().unwrap_or(0) {
        1 => {
            stack.push(package.to_string());
            errors.push(format!("build dependency cycle: {}", stack.join(" -> ")));
        }
        2 => {}
        _ => {
            state.insert(package.to_string(), 1);
            stack.push(package.to_string());
            if let Some(deps) = graph.get(package) {
                for dep in deps {
                    detect_cycle(dep, graph, state, stack, errors);
                }
            }
            stack.pop();
            state.insert(package.to_string(), 2);
        }
    }
}

fn normalize(name: &str) -> String {
    name.replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::model::BuildRule;

    #[test]
    fn empty_forbidden_prefix_is_reported() {
        let mut model = Model::default();
        model.build_rules.push(BuildRule {
            id: "r".into(),
            package: Some("ticonnect-core".into()),
            packages: Vec::new(),
            forbid: Vec::new(),
            forbid_prefix: Some(" ".into()),
        });
        let actual = BTreeMap::from([("ticonnect_core".to_string(), BTreeSet::new())]);
        let mut errors = Vec::new();
        check_rules(&model, &actual, &mut errors);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("empty forbidden prefix"))
        );
    }

    #[test]
    fn cycle_is_reported_once() {
        let mut graph = BTreeMap::new();
        graph.insert("a".to_string(), BTreeSet::from(["b".to_string()]));
        graph.insert("b".to_string(), BTreeSet::from(["a".to_string()]));
        let mut errors = Vec::new();
        check_cycles(&graph, &mut errors);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("build dependency cycle"))
        );
    }

    #[test]
    fn normalize_matches_cargo_names() {
        assert_eq!(normalize("ticonnect-core"), "ticonnect_core");
    }
}
