use ticonnect_ui::client::parse_activation_view;

#[test]
fn tray_activation_chat_reaches_chat_view() {
    assert_eq!(parse_activation_view("chat").expect("chat"), "chat");
}

#[test]
fn empty_activation_uses_chat() {
    assert_eq!(parse_activation_view("").expect("default"), "chat");
}
