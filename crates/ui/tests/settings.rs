use std::collections::BTreeMap;

use ticonnect_ui::model::SettingsForm;

#[test]
fn settings_form_parses_typed_values_with_defaults() {
    let form = SettingsForm::new(BTreeMap::from([
        ("discovery.enabled".to_owned(), "false".to_owned()),
        ("outbox.ttl_days".to_owned(), "12".to_owned()),
    ]));
    assert!(!form.bool_value("discovery.enabled", true));
    assert!(form.bool_value("discovery.mdns", true));
    assert_eq!(form.u32_value("outbox.ttl_days", 7), 12);
}

#[test]
fn settings_validation_rejects_out_of_range_values() {
    assert!(SettingsForm::validate_outbox_ttl(0).is_err());
    assert!(SettingsForm::validate_outbox_ttl(366).is_err());
    assert_eq!(SettingsForm::validate_outbox_ttl(30).expect("valid"), 30);
    assert!(SettingsForm::validate_beacon_interval(0).is_err());
    assert_eq!(SettingsForm::validate_beacon_interval(5).expect("valid"), 5);
}
