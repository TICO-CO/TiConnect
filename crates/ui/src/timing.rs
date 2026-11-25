use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();

pub fn mark(label: &str) {
    if std::env::var_os("TICONNECT_UI_TIMING").is_none() {
        return;
    }
    let start = *START.get_or_init(Instant::now);
    eprintln!("ui-timing +{}ms {label}", start.elapsed().as_millis());
}
