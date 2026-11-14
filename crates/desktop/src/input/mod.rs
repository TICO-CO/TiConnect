pub mod libei;
pub mod xtest;

use std::time::{Duration, Instant};

use thiserror::Error;

pub const MAX_MOTION_RATE: u32 = 250;
pub const MAX_EVENT_RATE: u32 = 500;
pub const MAX_COORD: i32 = 65_535;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    PointerMotion { x: i32, y: i32 },
    PointerButton { button: u8, pressed: bool },
    PointerAxis { horizontal: i32, vertical: i32 },
    Key { keysym: u32, pressed: bool },
}

impl InputEvent {
    pub fn validate(&self) -> Result<(), InputError> {
        match self {
            Self::PointerMotion { x, y } => {
                if !(0..=MAX_COORD).contains(x) || !(0..=MAX_COORD).contains(y) {
                    return Err(InputError::OutOfBounds);
                }
            }
            Self::PointerButton { button, .. } => {
                if *button == 0 || *button > 9 {
                    return Err(InputError::InvalidButton);
                }
            }
            Self::PointerAxis {
                horizontal,
                vertical,
            } => {
                if horizontal.unsigned_abs() > 10_000 || vertical.unsigned_abs() > 10_000 {
                    return Err(InputError::OutOfBounds);
                }
            }
            Self::Key { keysym, .. } => {
                if *keysym == 0 || *keysym > 0x10_ffff {
                    return Err(InputError::InvalidKey);
                }
            }
        }
        Ok(())
    }
}

pub trait InputBackend: Send {
    fn inject(&mut self, event: InputEvent) -> Result<(), InputError>;
    fn release(&mut self);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    Wayland,
    X11,
    Unknown,
}

pub fn detect_session() -> SessionKind {
    session_kind(std::env::var("XDG_SESSION_TYPE").ok().as_deref())
}

pub fn session_kind(value: Option<&str>) -> SessionKind {
    match value {
        Some(value) if value.eq_ignore_ascii_case("wayland") => SessionKind::Wayland,
        Some(value) if value.eq_ignore_ascii_case("x11") => SessionKind::X11,
        _ => SessionKind::Unknown,
    }
}

pub struct InputRouter {
    backend: Box<dyn InputBackend>,
    window: RateWindow,
}

impl InputRouter {
    pub fn new(backend: impl InputBackend + 'static) -> Self {
        Self {
            backend: Box::new(backend),
            window: RateWindow::default(),
        }
    }

    pub fn inject(&mut self, event: InputEvent) -> Result<(), InputError> {
        event.validate()?;
        let limit = match event {
            InputEvent::PointerMotion { .. } => MAX_MOTION_RATE,
            _ => MAX_EVENT_RATE,
        };
        if !self.window.allow(limit) {
            return Ok(());
        }
        self.backend.inject(event)
    }

    pub fn release(&mut self) {
        self.backend.release();
    }
}

#[derive(Debug)]
struct RateWindow {
    start: Instant,
    count: u32,
}

impl Default for RateWindow {
    fn default() -> Self {
        Self {
            start: Instant::now(),
            count: 0,
        }
    }
}

impl RateWindow {
    fn allow(&mut self, limit: u32) -> bool {
        if self.start.elapsed() >= Duration::from_secs(1) {
            self.start = Instant::now();
            self.count = 0;
        }
        if self.count >= limit {
            return false;
        }
        self.count = self.count.saturating_add(1);
        true
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InputError {
    #[error("input event is out of bounds")]
    OutOfBounds,

    #[error("pointer button is invalid")]
    InvalidButton,

    #[error("key symbol is invalid")]
    InvalidKey,

    #[error("input backend is unavailable")]
    Unavailable,

    #[error("input backend failed: {0}")]
    Backend(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    #[derive(Default)]
    struct Counters {
        count: u32,
        released: bool,
    }

    struct CountingBackend(Arc<std::sync::Mutex<Counters>>);

    impl InputBackend for CountingBackend {
        fn inject(&mut self, _event: InputEvent) -> Result<(), InputError> {
            self.0.lock().expect("counter").count += 1;
            Ok(())
        }

        fn release(&mut self) {
            self.0.lock().expect("counter").released = true;
        }
    }

    #[test]
    fn validates_events() {
        assert!(
            InputEvent::PointerMotion { x: 10, y: 20 }
                .validate()
                .is_ok()
        );
        assert!(
            InputEvent::PointerMotion { x: -1, y: 20 }
                .validate()
                .is_err()
        );
        assert!(
            InputEvent::PointerButton {
                button: 0,
                pressed: true
            }
            .validate()
            .is_err()
        );
        assert!(
            InputEvent::PointerButton {
                button: 1,
                pressed: true
            }
            .validate()
            .is_ok()
        );
        assert!(
            InputEvent::Key {
                keysym: 0,
                pressed: true
            }
            .validate()
            .is_err()
        );
        assert!(
            InputEvent::PointerAxis {
                horizontal: 100_000,
                vertical: 0
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn router_limits_and_releases() {
        let counters = Arc::new(std::sync::Mutex::new(Counters::default()));
        let mut router = InputRouter::new(CountingBackend(counters.clone()));
        for _ in 0..MAX_EVENT_RATE + 10 {
            router
                .inject(InputEvent::PointerButton {
                    button: 1,
                    pressed: true,
                })
                .expect("inject");
        }
        router.release();
        let counters = counters.lock().expect("counter");
        assert_eq!(counters.count, MAX_EVENT_RATE);
        assert!(counters.released);
    }

    #[test]
    fn detects_common_sessions() {
        assert_eq!(session_kind(Some("wayland")), SessionKind::Wayland);
        assert_eq!(session_kind(Some("x11")), SessionKind::X11);
        assert_eq!(session_kind(None), SessionKind::Unknown);
    }
}
