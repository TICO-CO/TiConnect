use std::time::{Duration, Instant};

use rand_core::{OsRng, RngCore};
use thiserror::Error;

pub const PIN_DIGITS: usize = 6;
pub const PIN_ATTEMPTS: u8 = 3;
pub const PIN_TTL: Duration = Duration::from_secs(120);

const PIN_MODULUS: u32 = 1_000_000;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PinError {
    #[error("pairing PIN must be exactly {PIN_DIGITS} digits")]
    InvalidFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinPolicy {
    pin: String,
    expires_at: Instant,
    attempts_remaining: u8,
}

impl PinPolicy {
    pub fn generate() -> Self {
        Self::generate_with(PIN_TTL)
    }

    pub fn generate_with(ttl: Duration) -> Self {
        let value = random_pin();
        Self {
            pin: format!("{value:06}"),
            expires_at: Instant::now() + ttl,
            attempts_remaining: PIN_ATTEMPTS,
        }
    }

    pub fn from_value(value: impl Into<String>) -> Result<Self, PinError> {
        Self::from_value_with(value, PIN_TTL)
    }

    pub fn from_value_with(value: impl Into<String>, ttl: Duration) -> Result<Self, PinError> {
        let pin = value.into();
        if pin.len() != PIN_DIGITS || !pin.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(PinError::InvalidFormat);
        }
        Ok(Self {
            pin,
            expires_at: Instant::now() + ttl,
            attempts_remaining: PIN_ATTEMPTS,
        })
    }

    pub fn value(&self) -> &str {
        &self.pin
    }

    pub fn expires_at(&self) -> Instant {
        self.expires_at
    }

    pub fn is_expired(&self, now: Instant) -> bool {
        now >= self.expires_at
    }

    pub fn attempts_remaining(&self) -> u8 {
        self.attempts_remaining
    }

    pub fn record_failure(&mut self) -> bool {
        self.attempts_remaining = self.attempts_remaining.saturating_sub(1);
        self.attempts_remaining > 0
    }

    pub fn reset_attempts(&mut self) {
        self.attempts_remaining = PIN_ATTEMPTS;
    }
}

fn random_pin() -> u32 {
    let limit = u32::MAX - (u32::MAX % PIN_MODULUS);
    loop {
        let value = OsRng.next_u32();
        if value < limit {
            return value % PIN_MODULUS;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_pins_are_six_digits() {
        for _ in 0..64 {
            let pin = PinPolicy::generate();
            assert_eq!(pin.value().len(), PIN_DIGITS);
            assert!(pin.value().bytes().all(|byte| byte.is_ascii_digit()));
        }
    }

    #[test]
    fn rejects_bad_pin_formats() {
        assert_eq!(PinPolicy::from_value("12345"), Err(PinError::InvalidFormat));
        assert_eq!(
            PinPolicy::from_value("1234567"),
            Err(PinError::InvalidFormat)
        );
        assert_eq!(
            PinPolicy::from_value("12a456"),
            Err(PinError::InvalidFormat)
        );
    }

    #[test]
    fn attempts_decrement_and_expire() {
        let mut pin = PinPolicy::from_value_with("123456", Duration::from_millis(5)).expect("pin");
        assert_eq!(pin.attempts_remaining(), PIN_ATTEMPTS);
        assert!(pin.record_failure());
        assert!(pin.record_failure());
        assert!(!pin.record_failure());
        assert_eq!(pin.attempts_remaining(), 0);

        std::thread::sleep(Duration::from_millis(10));
        assert!(pin.is_expired(Instant::now()));
    }
}
