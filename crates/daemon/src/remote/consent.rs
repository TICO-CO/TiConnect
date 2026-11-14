use rand_core::{OsRng, RngCore};

use super::state::{RemoteEvent, RemoteSession, RemoteState, RemoteTransition};

pub const PIN_LEN: usize = 6;

pub fn generate_pin() -> String {
    let value = OsRng.next_u32() % 1_000_000;
    format!("{value:06}")
}

pub fn verify_pin(
    session: &mut RemoteSession,
    pin: &str,
) -> Result<RemoteTransition, super::state::RemoteStateError> {
    let expected = session.pin.as_deref().unwrap_or_default();
    let matched = pin.len() == PIN_LEN && constant_time_eq(expected.as_bytes(), pin.as_bytes());
    session.transition(if matched {
        RemoteEvent::PinOk
    } else {
        RemoteEvent::PinFail
    })
}

pub fn consent(
    session: &mut RemoteSession,
    pin: String,
) -> Result<RemoteTransition, super::state::RemoteStateError> {
    let transition = session.transition(RemoteEvent::Consent)?;
    session.pin = Some(pin);
    Ok(transition)
}

pub fn indicator_visible(session: &RemoteSession) -> bool {
    session.indicator_visible && session.state == RemoteState::Active
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (left, right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::state::{RemoteRole, RemoteSessionId};

    fn session() -> RemoteSession {
        RemoteSession {
            id: RemoteSessionId::generate(),
            peer_id: "peer".to_owned(),
            fingerprint: "fingerprint".to_owned(),
            role: RemoteRole::Host,
            state: RemoteState::Requested,
            pin: None,
            attempts: 0,
            max_attempts: 3,
            expires_at: 0,
            indicator_visible: false,
        }
    }

    #[test]
    fn pins_are_six_digits_and_unique_enough() {
        let pins: std::collections::HashSet<_> = (0..32).map(|_| generate_pin()).collect();
        assert!(pins.len() > 1);
        for pin in pins {
            assert_eq!(pin.len(), PIN_LEN);
            assert!(pin.chars().all(|character| character.is_ascii_digit()));
        }
    }

    #[test]
    fn verifies_only_the_session_pin() {
        let mut session = session();
        consent(&mut session, "123456".to_owned()).expect("consent");
        assert!(verify_pin(&mut session, "000000").expect("fail").pin_failed);
        assert!(!verify_pin(&mut session, "123456").expect("ok").pin_failed);
        assert_eq!(session.state, RemoteState::Active);
    }
}
