use super::*;

#[test]
fn build_rule_matches_package_and_forbidden_dependency() {
    let rule = BuildRule {
        id: "r".into(),
        package: Some("ticonnect-core".into()),
        packages: vec!["ticonnect-ui".into()],
        forbid: vec!["ticonnect-daemon".into()],
        forbid_prefix: Some("ticonnect-".into()),
    };
    assert!(rule.forbids("ticonnect-daemon"));
    assert!(rule.forbids("ticonnect-core"));
    assert!(!rule.forbids("serde"));
}

#[test]
fn model_rejects_duplicate_node_ids() {
    let fragment = GraphFragment {
        systems: vec![system("A")],
        components: vec![component("A", "A")],
        ..GraphFragment::default()
    };
    let model = Model::from_fragment(fragment, MachinesFile::default());
    assert!(
        model
            .load_errors
            .iter()
            .any(|error| error.contains("collision"))
    );
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

fn component(id: &str, system: &str) -> Component {
    Component {
        id: id.into(),
        system: system.into(),
        name: id.into(),
        owner: "test".into(),
        kind: ComponentKind::Module,
        stage: 1,
        state: Vec::new(),
        contract: "contract".into(),
        test: "test".into(),
    }
}
