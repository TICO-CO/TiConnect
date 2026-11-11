pub mod endpoint;
pub mod frames;
pub mod pin;
pub mod qr;
pub mod spake2;

use thiserror::Error;

pub use endpoint::{
    EndpointError, InitiatorRequest, PairingOutcome, PairingPeer, ResponderRequest, run_initiator,
    run_responder,
};
pub use frames::{
    FrameError, PairingMessage, decode_frame, decode_message, encode_frame, encode_message,
};
pub use pin::{PIN_ATTEMPTS, PIN_DIGITS, PIN_TTL, PinError, PinPolicy};
pub use qr::{QrError, QrPayload, TOKEN_BYTES, TOKEN_TTL};
pub use spake2::{
    Confirmation, PairingMode, PairingSecret, PairingTranscript, PeerIdentity, Role, Spake2Error,
    Spake2Session,
};

pub const PAIRING_ALPN: &[u8] = b"ticonnect-pair/1";
pub const PAIRING_TIMEOUT_SECS: u64 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingState {
    Idle,
    Waiting,
    Confirming,
    Trusted,
    Failed,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingEvent {
    Start,
    Message,
    Confirm,
    Mismatch,
    Timeout,
    Cancel,
    Retry,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PairingError {
    #[error("pairing transition from {from:?} on {event:?} is not allowed")]
    InvalidTransition {
        from: PairingState,
        event: PairingEvent,
    },

    #[error("pairing attempts are exhausted")]
    AttemptsExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingMachine {
    state: PairingState,
    attempts_remaining: u8,
}

impl Default for PairingMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl PairingMachine {
    pub fn new() -> Self {
        Self {
            state: PairingState::Idle,
            attempts_remaining: PIN_ATTEMPTS,
        }
    }

    pub fn state(&self) -> PairingState {
        self.state
    }

    pub fn attempts_remaining(&self) -> u8 {
        self.attempts_remaining
    }

    pub fn start(&mut self) -> Result<(), PairingError> {
        self.transition(
            PairingEvent::Start,
            PairingState::Idle,
            PairingState::Waiting,
        )
    }

    pub fn receive_message(&mut self) -> Result<(), PairingError> {
        self.transition(
            PairingEvent::Message,
            PairingState::Waiting,
            PairingState::Confirming,
        )
    }

    pub fn confirm(&mut self, matches: bool) -> Result<(), PairingError> {
        self.require(PairingEvent::Confirm, PairingState::Confirming)?;
        if matches {
            self.state = PairingState::Trusted;
        } else {
            self.attempts_remaining = self.attempts_remaining.saturating_sub(1);
            self.state = PairingState::Failed;
        }
        Ok(())
    }

    pub fn timeout(&mut self) -> Result<(), PairingError> {
        self.transition(
            PairingEvent::Timeout,
            PairingState::Waiting,
            PairingState::Expired,
        )
    }

    pub fn cancel(&mut self) -> Result<(), PairingError> {
        match self.state {
            PairingState::Waiting | PairingState::Confirming => {
                self.state = PairingState::Idle;
                Ok(())
            }
            from => Err(PairingError::InvalidTransition {
                from,
                event: PairingEvent::Cancel,
            }),
        }
    }

    pub fn retry(&mut self) -> Result<(), PairingError> {
        self.require(PairingEvent::Retry, PairingState::Failed)?;
        if self.attempts_remaining == 0 {
            return Err(PairingError::AttemptsExhausted);
        }
        self.state = PairingState::Waiting;
        Ok(())
    }

    fn transition(
        &mut self,
        event: PairingEvent,
        from: PairingState,
        to: PairingState,
    ) -> Result<(), PairingError> {
        self.require(event, from)?;
        self.state = to;
        Ok(())
    }

    fn require(&self, event: PairingEvent, expected: PairingState) -> Result<(), PairingError> {
        if self.state != expected {
            return Err(PairingError::InvalidTransition {
                from: self.state,
                event,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_the_declared_pairing_state_machine() {
        let mut machine = PairingMachine::new();
        assert_eq!(machine.state(), PairingState::Idle);
        machine.start().expect("start");
        assert_eq!(machine.state(), PairingState::Waiting);
        machine.receive_message().expect("message");
        assert_eq!(machine.state(), PairingState::Confirming);
        machine.confirm(true).expect("confirm");
        assert_eq!(machine.state(), PairingState::Trusted);
    }

    #[test]
    fn mismatch_fails_and_retry_is_bounded() {
        let mut machine = PairingMachine::new();
        machine.start().expect("start");
        machine.receive_message().expect("message");
        machine.confirm(false).expect("mismatch");
        assert_eq!(machine.state(), PairingState::Failed);
        assert_eq!(machine.attempts_remaining(), PIN_ATTEMPTS - 1);

        machine.retry().expect("retry");
        assert_eq!(machine.state(), PairingState::Waiting);
    }

    #[test]
    fn timeout_and_cancel_are_terminal_or_idle() {
        let mut machine = PairingMachine::new();
        machine.start().expect("start");
        machine.timeout().expect("timeout");
        assert_eq!(machine.state(), PairingState::Expired);

        let mut machine = PairingMachine::new();
        machine.start().expect("start");
        machine.cancel().expect("cancel");
        assert_eq!(machine.state(), PairingState::Idle);
    }

    #[test]
    fn rejects_invalid_transitions() {
        let mut machine = PairingMachine::new();
        assert!(matches!(
            machine.receive_message(),
            Err(PairingError::InvalidTransition { .. })
        ));

        let mut machine = PairingMachine::new();
        machine.start().expect("start");
        machine.receive_message().expect("message");
        machine.confirm(false).expect("mismatch");
        machine.attempts_remaining = 0;
        assert_eq!(machine.retry(), Err(PairingError::AttemptsExhausted));
    }
}
