use ticonnect_ui::client::parse_activation_view;

#[test]
fn activation_views_are_typed() {
    assert_eq!(parse_activation_view("").expect("default"), "chat");
    assert_eq!(parse_activation_view("users").expect("users"), "users");
    assert_eq!(
        parse_activation_view("settings").expect("settings"),
        "settings"
    );
    assert!(parse_activation_view("invalid").is_err());
}
