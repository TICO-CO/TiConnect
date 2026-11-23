use thiserror::Error;
use ticonnect_core::call::{CallDirection, CallId, CallState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveCall {
    pub id: CallId,
    pub user_id: String,
    pub direction: CallDirection,
    pub state: CallState,
    pub created_at: i64,
    pub connected_at: Option<i64>,
    pub input_muted: bool,
    pub output_muted: bool,
}

impl ActiveCall {
    pub fn info(&self) -> CallInfo {
        CallInfo {
            id: self.id,
            user_id: self.user_id.clone(),
            direction: self.direction,
            state: self.state,
            created_at: self.created_at,
            connected_at: self.connected_at,
            input_muted: self.input_muted,
            output_muted: self.output_muted,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallInfo {
    pub id: CallId,
    pub user_id: String,
    pub direction: CallDirection,
    pub state: CallState,
    pub created_at: i64,
    pub connected_at: Option<i64>,
    pub input_muted: bool,
    pub output_muted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallTransition {
    Connecting(ActiveCall),
    Ringing(ActiveCall),
    InCall(ActiveCall),
    Ended(ActiveCall, EndReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    Declined,
    Hangup,
    Timeout,
    Error,
    Disconnected,
}

impl EndReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Declined => "declined",
            Self::Hangup => "hangup",
            Self::Timeout => "timeout",
            Self::Error => "error",
            Self::Disconnected => "disconnected",
        }
    }
}

#[derive(Debug, Default)]
pub struct CallRegistry {
    active: Option<ActiveCall>,
}

impl CallRegistry {
    pub fn active(&self) -> Option<&ActiveCall> {
        self.active.as_ref()
    }

    pub fn start(&mut self, user_id: &str, now: i64) -> Result<CallTransition, CallStateError> {
        self.begin(user_id, CallDirection::Outgoing, now)
    }

    pub fn incoming(&mut self, user_id: &str, now: i64) -> Result<CallTransition, CallStateError> {
        self.begin(user_id, CallDirection::Incoming, now)
    }

    pub fn incoming_with_id(
        &mut self,
        id: CallId,
        user_id: &str,
        now: i64,
    ) -> Result<CallTransition, CallStateError> {
        self.begin_with_id(id, user_id, CallDirection::Incoming, now)
    }

    fn begin(
        &mut self,
        user_id: &str,
        direction: CallDirection,
        now: i64,
    ) -> Result<CallTransition, CallStateError> {
        self.begin_with_id(CallId::generate(), user_id, direction, now)
    }

    fn begin_with_id(
        &mut self,
        id: CallId,
        user_id: &str,
        direction: CallDirection,
        now: i64,
    ) -> Result<CallTransition, CallStateError> {
        if self.active.is_some() {
            return Err(CallStateError::Busy);
        }
        let state = match direction {
            CallDirection::Outgoing => CallState::Connecting,
            CallDirection::Incoming => CallState::Ringing,
        };
        let call = ActiveCall {
            id,
            user_id: user_id.to_owned(),
            direction,
            state,
            created_at: now,
            connected_at: None,
            input_muted: false,
            output_muted: false,
        };
        self.active = Some(call.clone());
        let transition = match direction {
            CallDirection::Outgoing => CallTransition::Connecting(call),
            CallDirection::Incoming => CallTransition::Ringing(call),
        };
        Ok(transition)
    }

    pub fn mark_ringing(
        &mut self,
        id: CallId,
        user_id: &str,
    ) -> Result<Option<CallTransition>, CallStateError> {
        let call = self.active_mut(id)?;
        if call.user_id != user_id {
            return Err(CallStateError::InvalidTransition);
        }
        if call.state == CallState::Ringing {
            return Ok(None);
        }
        if call.state != CallState::Connecting {
            return Err(CallStateError::InvalidTransition);
        }
        call.state = CallState::Ringing;
        Ok(Some(CallTransition::Ringing(call.clone())))
    }

    pub fn accept(&mut self, id: CallId, now: i64) -> Result<CallTransition, CallStateError> {
        let call = self.active_mut(id)?;
        if call.state != CallState::Ringing {
            return Err(CallStateError::InvalidTransition);
        }
        call.state = CallState::InCall;
        call.connected_at = Some(now);
        Ok(CallTransition::InCall(call.clone()))
    }

    pub fn accept_or_current(
        &mut self,
        id: CallId,
        now: i64,
    ) -> Result<(ActiveCall, bool), CallStateError> {
        let call = self.active_mut(id)?;
        if call.state == CallState::InCall {
            return Ok((call.clone(), false));
        }
        if call.state != CallState::Ringing {
            return Err(CallStateError::InvalidTransition);
        }
        call.state = CallState::InCall;
        call.connected_at = Some(now);
        Ok((call.clone(), true))
    }

    pub fn set_muted(
        &mut self,
        id: CallId,
        input_muted: bool,
        output_muted: bool,
    ) -> Result<ActiveCall, CallStateError> {
        let call = self.active_mut(id)?;
        call.input_muted = input_muted;
        call.output_muted = output_muted;
        Ok(call.clone())
    }

    pub fn decline(&mut self, id: CallId) -> Result<CallTransition, CallStateError> {
        self.end(id, EndReason::Declined, true)
    }

    pub fn hangup(&mut self, id: CallId) -> Result<CallTransition, CallStateError> {
        self.end(id, EndReason::Hangup, false)
    }

    pub fn timeout(&mut self, id: CallId) -> Result<CallTransition, CallStateError> {
        self.end(id, EndReason::Timeout, true)
    }

    pub fn error(&mut self, id: CallId) -> Result<CallTransition, CallStateError> {
        self.end(id, EndReason::Error, false)
    }

    pub fn disconnected(&mut self, user_id: &str) -> Option<(ActiveCall, EndReason)> {
        if self
            .active
            .as_ref()
            .is_some_and(|call| call.user_id == user_id)
        {
            let mut call = self.active.take().expect("active call");
            call.state = CallState::Ended;
            return Some((call, EndReason::Disconnected));
        }
        None
    }

    pub fn clear(&mut self) -> Option<ActiveCall> {
        self.active.take()
    }

    fn end(
        &mut self,
        id: CallId,
        reason: EndReason,
        ringing_only: bool,
    ) -> Result<CallTransition, CallStateError> {
        let call = self.active_mut(id)?;
        if ringing_only && !matches!(call.state, CallState::Connecting | CallState::Ringing) {
            return Err(CallStateError::InvalidTransition);
        }
        if call.state == CallState::Ended {
            return Err(CallStateError::InvalidTransition);
        }
        call.state = CallState::Ended;
        let call = self.active.take().expect("active call");
        Ok(CallTransition::Ended(call, reason))
    }

    fn active_mut(&mut self, id: CallId) -> Result<&mut ActiveCall, CallStateError> {
        match self.active.as_mut() {
            Some(call) if call.id == id => Ok(call),
            _ => Err(CallStateError::UnknownCall(id)),
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CallStateError {
    #[error("another call is already active")]
    Busy,

    #[error("call transition is invalid")]
    InvalidTransition,

    #[error("unknown call: {0}")]
    UnknownCall(CallId),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_the_documented_state_machine() {
        let mut registry = CallRegistry::default();
        let CallTransition::Connecting(call) = registry.start("user", 1).expect("start") else {
            panic!("expected connecting");
        };
        assert_eq!(call.state, CallState::Connecting);
        assert!(matches!(
            registry.start("user", 2),
            Err(CallStateError::Busy)
        ));

        let Some(CallTransition::Ringing(call)) = registry
            .mark_ringing(call.id, "user")
            .expect("mark ringing")
        else {
            panic!("expected ringing");
        };
        assert_eq!(call.state, CallState::Ringing);

        let CallTransition::InCall(call) = registry.accept(call.id, 2).expect("accept") else {
            panic!("expected in call");
        };
        assert_eq!(call.state, CallState::InCall);
        assert_eq!(call.connected_at, Some(2));

        let CallTransition::Ended(call, reason) = registry.hangup(call.id).expect("hangup") else {
            panic!("expected ended");
        };
        assert_eq!(reason, EndReason::Hangup);
        assert!(registry.active().is_none());
        assert!(registry.hangup(call.id).is_err());
    }

    #[test]
    fn incoming_decline_timeout_and_disconnect_end_calls() {
        let mut registry = CallRegistry::default();
        let CallTransition::Ringing(call) = registry.incoming("user", 1).expect("incoming") else {
            panic!("expected ringing");
        };
        assert!(matches!(
            registry.decline(call.id),
            Ok(CallTransition::Ended(_, EndReason::Declined))
        ));

        let CallTransition::Ringing(call) = registry.incoming("user", 1).expect("incoming") else {
            panic!("expected ringing");
        };
        assert!(matches!(
            registry.timeout(call.id),
            Ok(CallTransition::Ended(_, EndReason::Timeout))
        ));

        let CallTransition::Ringing(_) = registry.incoming("user", 1).expect("incoming") else {
            panic!("expected ringing");
        };
        assert_eq!(
            registry.disconnected("user").map(|(_, reason)| reason),
            Some(EndReason::Disconnected)
        );
    }

    #[test]
    fn hangup_cancels_ringing_calls() {
        let mut registry = CallRegistry::default();
        let CallTransition::Connecting(call) = registry.start("user", 1).expect("start") else {
            panic!("expected connecting");
        };
        assert!(matches!(
            registry.hangup(call.id),
            Ok(CallTransition::Ended(_, EndReason::Hangup))
        ));
        assert!(registry.active().is_none());
    }

    #[test]
    fn repeated_accept_is_idempotent() {
        let mut registry = CallRegistry::default();
        let CallTransition::Ringing(call) = registry.incoming("user", 1).expect("incoming") else {
            panic!("expected ringing");
        };
        let (accepted, started) = registry.accept_or_current(call.id, 2).expect("accept");
        assert!(started);
        assert_eq!(accepted.state, CallState::InCall);
        let (current, started) = registry
            .accept_or_current(call.id, 3)
            .expect("repeat accept");
        assert!(!started);
        assert_eq!(current.state, CallState::InCall);
        assert_eq!(current.connected_at, Some(2));
    }

    #[test]
    fn invalid_transitions_are_rejected() {
        let mut registry = CallRegistry::default();
        assert!(registry.decline(CallId::generate()).is_err());
        let CallTransition::Ringing(call) = registry.incoming("user", 1).expect("incoming") else {
            panic!("expected ringing");
        };
        let CallTransition::InCall(call) = registry.accept(call.id, 2).expect("accept") else {
            panic!("expected in call");
        };
        assert!(registry.decline(call.id).is_err());
        assert!(registry.accept(call.id, 3).is_err());
    }
}
