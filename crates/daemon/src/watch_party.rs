use std::sync::{Arc, Mutex, PoisonError, RwLock};

use thiserror::Error;
use ticonnect_core::id::NodeId;
use ticonnect_core::watch_party::{WatchPartyCommand, WatchPartyFrame, valid_video_id};
use ticonnect_net::transport::{Session, Transport, TransportError, UserInfo};
use tokio::task::JoinHandle;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchPartySnapshot {
    pub party_id: String,
    pub user_id: String,
    pub video_id: String,
    pub role: String,
    pub state: String,
    pub position_ms: u64,
    pub playing: bool,
    pub revision: u64,
    pub actor: String,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchPartyEvent {
    Requested(WatchPartySnapshot),
    Changed(WatchPartySnapshot),
    Ended { party_id: String, reason: String },
}

#[derive(Debug)]
pub struct InboundWatchPartyFrame {
    pub user: UserInfo,
    pub frame: WatchPartyFrame,
}

#[derive(Debug, Error)]
pub enum WatchPartyError {
    #[error("user is offline")]
    UserOffline,

    #[error("invalid user id")]
    InvalidUser,

    #[error("invalid video id")]
    InvalidVideo,

    #[error("watch party was not found")]
    NotFound,

    #[error("watch party belongs to another user")]
    WrongUser,

    #[error("watch party protocol failed: {0}")]
    Protocol(String),

    #[error(transparent)]
    Transport(#[from] TransportError),

    #[error(transparent)]
    Core(#[from] ticonnect_core::watch_party::WatchPartyError),
}

#[derive(Debug, Clone)]
struct Party {
    party_id: Uuid,
    user_id: String,
    video_id: String,
    role: String,
    state: String,
    position_ms: u64,
    playing: bool,
    revision: u64,
    actor: String,
    updated_at: u64,
}

impl Party {
    fn snapshot(&self) -> WatchPartySnapshot {
        WatchPartySnapshot {
            party_id: self.party_id.to_string(),
            user_id: self.user_id.clone(),
            video_id: self.video_id.clone(),
            role: self.role.clone(),
            state: self.state.clone(),
            position_ms: self.position_ms,
            playing: self.playing,
            revision: self.revision,
            actor: self.actor.clone(),
            updated_at: self.updated_at,
        }
    }
}

pub struct WatchPartyService {
    local: NodeId,
    transport: Arc<Transport>,
    party: RwLock<Option<Party>>,
    events: tokio::sync::broadcast::Sender<WatchPartyEvent>,
    _lock: Mutex<()>,
}

impl WatchPartyService {
    pub fn new(local: NodeId, transport: Arc<Transport>) -> Self {
        let (events, _) = tokio::sync::broadcast::channel(64);
        Self {
            local,
            transport,
            party: RwLock::new(None),
            events,
            _lock: Mutex::new(()),
        }
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<WatchPartyEvent> {
        self.events.subscribe()
    }

    pub fn snapshot(&self) -> Option<WatchPartySnapshot> {
        self.party
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(Party::snapshot)
    }

    pub async fn start(
        &self,
        user_id: &str,
        video_id: &str,
    ) -> Result<WatchPartySnapshot, WatchPartyError> {
        if !valid_video_id(video_id) {
            return Err(WatchPartyError::InvalidVideo);
        }
        let node_id = user_id
            .parse::<NodeId>()
            .map_err(|_| WatchPartyError::InvalidUser)?;
        if self.snapshot().is_some() {
            self.end("replaced").await?;
        }
        let party = new_party(user_id, video_id, "host");
        self.send_to(
            node_id,
            &party,
            WatchPartyFrame::Invite {
                party_id: *party.party_id.as_bytes(),
                video_id: video_id.to_owned(),
            },
        )
        .await?;
        let snapshot = self.store(party, "ringing");
        let _ = self.events.send(WatchPartyEvent::Changed(snapshot.clone()));
        Ok(snapshot)
    }

    pub async fn accept(&self, party_id: &str) -> Result<WatchPartySnapshot, WatchPartyError> {
        let party = self.party_for(party_id)?;
        let node_id = party
            .user_id
            .parse::<NodeId>()
            .map_err(|_| WatchPartyError::InvalidUser)?;
        self.send_to(
            node_id,
            &party,
            WatchPartyFrame::Accept {
                party_id: *party.party_id.as_bytes(),
            },
        )
        .await?;
        self.update_state(party_id, "active")
    }

    pub async fn decline(&self, party_id: &str) -> Result<(), WatchPartyError> {
        let party = self.party_for(party_id)?;
        let node_id = party
            .user_id
            .parse::<NodeId>()
            .map_err(|_| WatchPartyError::InvalidUser)?;
        self.send_to(
            node_id,
            &party,
            WatchPartyFrame::Decline {
                party_id: *party.party_id.as_bytes(),
            },
        )
        .await?;
        self.remove(party_id, "declined")
    }

    pub async fn publish(
        &self,
        party_id: &str,
        position_ms: u64,
        playing: bool,
    ) -> Result<WatchPartySnapshot, WatchPartyError> {
        let party = self.party_for(party_id)?;
        let node_id = party
            .user_id
            .parse::<NodeId>()
            .map_err(|_| WatchPartyError::InvalidUser)?;
        let revision = party.revision.saturating_add(1);
        let command = WatchPartyCommand {
            revision,
            actor: self.local.to_string(),
            position_ms,
            playing,
        };
        self.send_to(
            node_id,
            &party,
            WatchPartyFrame::Command {
                party_id: *party.party_id.as_bytes(),
                command: command.clone(),
            },
        )
        .await?;
        let snapshot = self.apply_command(party_id, command)?;
        let _ = self.events.send(WatchPartyEvent::Changed(snapshot.clone()));
        Ok(snapshot)
    }

    pub async fn end(&self, party_id: &str) -> Result<(), WatchPartyError> {
        let party = self.party_for(party_id)?;
        let node_id = party
            .user_id
            .parse::<NodeId>()
            .map_err(|_| WatchPartyError::InvalidUser)?;
        let _ = self
            .send_to(
                node_id,
                &party,
                WatchPartyFrame::End {
                    party_id: *party.party_id.as_bytes(),
                },
            )
            .await;
        self.remove(party_id, "ended")
    }

    pub async fn handle_frame(
        &self,
        user: UserInfo,
        frame: WatchPartyFrame,
    ) -> Result<(), WatchPartyError> {
        let node_id = user.node_id;
        match frame {
            WatchPartyFrame::Invite { party_id, video_id } => {
                if !valid_video_id(&video_id) {
                    return Err(WatchPartyError::InvalidVideo);
                }
                let party = Party {
                    party_id: Uuid::from_bytes(party_id),
                    user_id: node_id.to_string(),
                    video_id,
                    role: "guest".to_owned(),
                    state: "ringing".to_owned(),
                    position_ms: 0,
                    playing: false,
                    revision: 0,
                    actor: node_id.to_string(),
                    updated_at: now_unix_ms(),
                };
                let snapshot = self.store(party, "ringing");
                let _ = self.events.send(WatchPartyEvent::Requested(snapshot));
            }
            WatchPartyFrame::Accept { party_id } => {
                let id = Uuid::from_bytes(party_id).to_string();
                let snapshot = self.update_state(&id, "active")?;
                let _ = self.events.send(WatchPartyEvent::Changed(snapshot));
            }
            WatchPartyFrame::Decline { party_id } => {
                let id = Uuid::from_bytes(party_id).to_string();
                self.remove(&id, "declined")?;
            }
            WatchPartyFrame::Command { party_id, command } => {
                let id = Uuid::from_bytes(party_id).to_string();
                let snapshot = self.apply_command(&id, command)?;
                let _ = self.events.send(WatchPartyEvent::Changed(snapshot));
            }
            WatchPartyFrame::End { party_id } => {
                let id = Uuid::from_bytes(party_id).to_string();
                self.remove(&id, "ended")?;
            }
        }
        Ok(())
    }

    pub fn handle_disconnect(&self, user: &UserInfo) {
        let party_id = {
            let party = self.party.read().unwrap_or_else(PoisonError::into_inner);
            party
                .as_ref()
                .filter(|party| party.user_id == user.node_id.to_string())
                .map(|party| party.party_id.to_string())
        };
        if let Some(party_id) = party_id {
            let _ = self.remove(&party_id, "disconnected");
        }
    }

    fn party_for(&self, party_id: &str) -> Result<Party, WatchPartyError> {
        let party = self.party.read().unwrap_or_else(PoisonError::into_inner);
        party
            .as_ref()
            .filter(|party| party.party_id.to_string() == party_id)
            .cloned()
            .ok_or(WatchPartyError::NotFound)
    }

    fn store(&self, party: Party, state: &str) -> WatchPartySnapshot {
        let party = Party {
            state: state.to_owned(),
            ..party
        };
        let snapshot = party.snapshot();
        *self.party.write().unwrap_or_else(PoisonError::into_inner) = Some(party);
        snapshot
    }

    fn update_state(
        &self,
        party_id: &str,
        state: &str,
    ) -> Result<WatchPartySnapshot, WatchPartyError> {
        let mut party = self.party.write().unwrap_or_else(PoisonError::into_inner);
        let party = party
            .as_mut()
            .filter(|party| party.party_id.to_string() == party_id)
            .ok_or(WatchPartyError::NotFound)?;
        party.state = state.to_owned();
        party.updated_at = now_unix_ms();
        Ok(party.snapshot())
    }

    fn apply_command(
        &self,
        party_id: &str,
        command: WatchPartyCommand,
    ) -> Result<WatchPartySnapshot, WatchPartyError> {
        let mut guard = self.party.write().unwrap_or_else(PoisonError::into_inner);
        let party = guard
            .as_mut()
            .filter(|party| party.party_id.to_string() == party_id)
            .ok_or(WatchPartyError::NotFound)?;
        let newer = command.revision > party.revision
            || (command.revision == party.revision && command.actor > party.actor);
        if !newer {
            return Ok(party.snapshot());
        }
        party.position_ms = command.position_ms;
        party.playing = command.playing;
        party.revision = command.revision;
        party.actor = command.actor;
        party.updated_at = now_unix_ms();
        Ok(party.snapshot())
    }

    fn remove(&self, party_id: &str, reason: &str) -> Result<(), WatchPartyError> {
        let mut guard = self.party.write().unwrap_or_else(PoisonError::into_inner);
        let matches = guard
            .as_ref()
            .is_some_and(|party| party.party_id.to_string() == party_id);
        if !matches {
            return Err(WatchPartyError::NotFound);
        }
        let _ = guard.take();
        let _ = self.events.send(WatchPartyEvent::Ended {
            party_id: party_id.to_owned(),
            reason: reason.to_owned(),
        });
        Ok(())
    }

    async fn send_to(
        &self,
        node_id: NodeId,
        _party: &Party,
        frame: WatchPartyFrame,
    ) -> Result<(), WatchPartyError> {
        let session = self
            .transport
            .session_by_node_id(node_id)
            .ok_or(WatchPartyError::UserOffline)?;
        send_frame(&session, &frame).await
    }
}

async fn send_frame(session: &Session, frame: &WatchPartyFrame) -> Result<(), WatchPartyError> {
    let mut send = session.open_uni().await?;
    ticonnect_net::transport::session::write_frame(&mut send, &frame.to_wire()?)
        .await
        .map_err(WatchPartyError::Transport)
}

fn new_party(user_id: &str, video_id: &str, role: &str) -> Party {
    Party {
        party_id: Uuid::now_v7(),
        user_id: user_id.to_owned(),
        video_id: video_id.to_owned(),
        role: role.to_owned(),
        state: "ringing".to_owned(),
        position_ms: 0,
        playing: false,
        revision: 0,
        actor: String::new(),
        updated_at: now_unix_ms(),
    }
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

pub fn spawn(
    service: Arc<WatchPartyService>,
    mut frames: tokio::sync::mpsc::UnboundedReceiver<InboundWatchPartyFrame>,
) -> JoinHandle<()> {
    let mut transport_events = service.transport.subscribe();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                frame = frames.recv() => {
                    let Some(frame) = frame else { break };
                    if let Err(error) = service.handle_frame(frame.user, frame.frame).await {
                        tracing::debug!(%error, "watch party frame rejected");
                    }
                }
                event = transport_events.recv() => {
                    match event {
                        Ok(ticonnect_net::transport::TransportEvent::UserDisconnected(user)) => {
                            service.handle_disconnect(&user);
                        }
                        Ok(_) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    })
}
