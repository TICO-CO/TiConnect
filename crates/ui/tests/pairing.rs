use ticonnect_ui::model::{format_countdown, format_pin};

#[test]
fn pairing_pin_and_countdown_render_cleanly() {
    assert_eq!(format_pin("123456"), "123 456");
    assert_eq!(format_pin("12345"), "12345");
    assert_eq!(format_countdown(100, 40), 60);
    assert_eq!(format_countdown(100, 140), 0);
}
