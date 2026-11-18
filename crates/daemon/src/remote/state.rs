use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RemoteSessionId(Uuid);

impl RemoteSessionId {
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }

    pub fn as_bytes(self) -> [u8; 16] {
        *self.0.as_bytes()
    }
}

impl std::fmt::Display for RemoteSessionId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::str::FromStr for RemoteSessionId {
    type Err = RemoteStateError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value)
            .map(Self)
            .map_err(|_| RemoteStateError::InvalidSession)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteState {
    Idle,
    Requested,
    Active,
    Ending,
    Ended,
    Denied,
}

impl RemoteState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Requested => "requested",
            Self::Active => "active",
            Self::Ending => "ending",
            Self::Ended => "ended",
            Self::Denied => "denied",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteRole {
    Host,
    Viewer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSession {
    pub id: RemoteSessionId,
    pub user_id: String,
    pub fingerprint: String,
    pub role: RemoteRole,
    pub state: RemoteState,
    pub expires_at: i64,
    pub indicator_visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteEvent {
    Request,
    Consent,
    Deny,
    KillSwitch,
    Disconnect,
    Hangup,
    CleanupDone,
}

impl RemoteSession {
    pub fn transition(&mut self, event: RemoteEvent) -> Result<RemoteTransition, RemoteStateError> {
        let next = match (self.state, event) {
            (RemoteState::Idle, RemoteEvent::Request) => RemoteState::Requested,
            (RemoteState::Requested, RemoteEvent::Consent) => RemoteState::Active,
            (RemoteState::Requested, RemoteEvent::Deny) => RemoteState::Denied,
            (RemoteState::Active, RemoteEvent::KillSwitch)
            | (RemoteState::Active, RemoteEvent::Disconnect)
            | (RemoteState::Active, RemoteEvent::Hangup) => RemoteState::Ending,
            (RemoteState::Ending, RemoteEvent::CleanupDone) => RemoteState::Ended,
            (RemoteState::Requested, RemoteEvent::KillSwitch) => RemoteState::Ending,
            _ => return Err(RemoteStateError::InvalidTransition),
        };
        self.state = next;
        if next == RemoteState::Active {
            self.indicator_visible = true;
        }
        if next == RemoteState::Ending || next == RemoteState::Denied {
            self.indicator_visible = false;
        }
        Ok(RemoteTransition { state: next })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteTransition {
    pub state: RemoteState,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RemoteStateError {
    #[error("remote session transition is invalid")]
    InvalidTransition,

    #[error("remote session id is invalid")]
    InvalidSession,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(role: RemoteRole) -> RemoteSession {
        RemoteSession {
            id: RemoteSessionId::generate(),
            user_id: "user".to_owned(),
            fingerprint: "fingerprint".to_owned(),
            role,
            state: RemoteState::Idle,
            expires_at: 0,
            indicator_visible: false,
        }
    }

    #[test]
    fn follows_the_documented_state_machine() {
        let mut session = session(RemoteRole::Host);
        assert_eq!(
            session
                .transition(RemoteEvent::Request)
                .expect("request")
                .state,
            RemoteState::Requested
        );
        assert_eq!(
            session
                .transition(RemoteEvent::Consent)
                .expect("consent")
                .state,
            RemoteState::Active
        );
        assert!(session.indicator_visible);
        assert_eq!(
            session
                .transition(RemoteEvent::KillSwitch)
                .expect("kill")
                .state,
            RemoteState::Ending
        );
        assert_eq!(
            session
                .transition(RemoteEvent::CleanupDone)
                .expect("cleanup")
                .state,
            RemoteState::Ended
        );
        assert!(!session.indicator_visible);
    }

    #[test]
    fn invalid_transitions_are_rejected() {
        let mut session = session(RemoteRole::Host);
        assert!(session.transition(RemoteEvent::Consent).is_err());
        session.transition(RemoteEvent::Request).expect("request");
        assert!(session.transition(RemoteEvent::CleanupDone).is_err());
    }
}
