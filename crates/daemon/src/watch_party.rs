use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use thiserror::Error;
use ticonnect_core::id::NodeId;
use ticonnect_core::watch_party::{MAX_PARTY_PARTICIPANTS, WatchPartyFrame, valid_video_id};
use ticonnect_net::transport::{Session, Transport, TransportError, UserInfo};
use tokio::task::JoinHandle;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchPartySnapshot {
    pub party_id: String,
    pub host_id: String,
    pub video_id: String,
    pub role: String,
    pub state: String,
    pub participants: Vec<String>,
    pub invited: Vec<String>,
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

    #[error("select at least one other participant")]
    NoParticipants,

    #[error("watch party supports at most {MAX_PARTY_PARTICIPANTS} participants")]
    TooManyParticipants,

    #[error("watch party was not found")]
    NotFound,

    #[error("only the host can do that")]
    NotHost,

    #[error("only a guest can do that")]
    NotGuest,

    #[error("watch party frame came from the wrong user")]
    WrongUser,

    #[error("user is not part of this watch party")]
    NotParticipant,

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
    host_id: String,
    video_id: String,
    role: String,
    state: String,
    participants: BTreeSet<String>,
    invited: BTreeSet<String>,
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
            host_id: self.host_id.clone(),
            video_id: self.video_id.clone(),
            role: self.role.clone(),
            state: self.state.clone(),
            participants: self.participants.iter().cloned().collect(),
            invited: self.invited.iter().cloned().collect(),
            position_ms: self.position_ms,
            playing: self.playing,
            revision: self.revision,
            actor: self.actor.clone(),
            updated_at: self.updated_at,
        }
    }

    fn is_host(&self) -> bool {
        self.role == "host"
    }

    fn refresh_state(&mut self) {
        self.state = if self.participants.len() > 1 {
            "active".to_owned()
        } else {
            "ringing".to_owned()
        };
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
        user_ids: &[&str],
        video_id: &str,
    ) -> Result<WatchPartySnapshot, WatchPartyError> {
        if !valid_video_id(video_id) {
            return Err(WatchPartyError::InvalidVideo);
        }
        let local_id = self.local.to_string();
        let mut invited = BTreeSet::new();
        for user_id in user_ids {
            if *user_id == local_id {
                continue;
            }
            let node_id = NodeId::from_str(user_id).map_err(|_| WatchPartyError::InvalidUser)?;
            if self.transport.session_by_node_id(node_id).is_none() {
                return Err(WatchPartyError::UserOffline);
            }
            invited.insert((*user_id).to_owned());
        }
        if invited.is_empty() {
            return Err(WatchPartyError::NoParticipants);
        }
        if invited.len() + 1 > MAX_PARTY_PARTICIPANTS {
            return Err(WatchPartyError::TooManyParticipants);
        }
        if self.snapshot().is_some() {
            self.finish("replaced", true).await;
        }
        let mut participants = BTreeSet::new();
        participants.insert(local_id.clone());
        let party_id = Uuid::now_v7();
        let party = Party {
            party_id,
            host_id: local_id.clone(),
            video_id: video_id.to_owned(),
            role: "host".to_owned(),
            state: "ringing".to_owned(),
            participants,
            invited: invited.clone(),
            position_ms: 0,
            playing: false,
            revision: 0,
            actor: String::new(),
            updated_at: now_unix_ms(),
        };
        let snapshot = self.store(party);
        self.emit(WatchPartyEvent::Changed(snapshot.clone()));
        let invite = WatchPartyFrame::Invite {
            party_id: *party_id.as_bytes(),
            video_id: video_id.to_owned(),
        };
        let mut failed = Vec::new();
        for user_id in &invited {
            if self.send_to_id(user_id, &invite).await.is_err() {
                failed.push(user_id.clone());
            }
        }
        if !failed.is_empty() {
            self.remove_users(&failed).await;
        }
        Ok(self.snapshot().unwrap_or(snapshot))
    }

    pub async fn invite(
        &self,
        party_id: &str,
        user_ids: &[&str],
    ) -> Result<WatchPartySnapshot, WatchPartyError> {
        let local_id = self.local.to_string();
        let mut users = BTreeSet::new();
        for user_id in user_ids {
            if *user_id == local_id {
                continue;
            }
            let node_id = NodeId::from_str(user_id).map_err(|_| WatchPartyError::InvalidUser)?;
            if self.transport.session_by_node_id(node_id).is_none() {
                return Err(WatchPartyError::UserOffline);
            }
            users.insert((*user_id).to_owned());
        }
        let party = self.party_for_host(party_id)?;
        let existing: BTreeSet<String> = party
            .participants
            .iter()
            .chain(party.invited.iter())
            .cloned()
            .collect();
        users.retain(|user| !existing.contains(user));
        if users.is_empty() {
            return Ok(party.snapshot());
        }
        if party.participants.len() + party.invited.len() + users.len() > MAX_PARTY_PARTICIPANTS {
            return Err(WatchPartyError::TooManyParticipants);
        }
        let party_id_bytes = party_id_bytes(party_id)?;
        {
            let mut guard = self.party.write().unwrap_or_else(PoisonError::into_inner);
            let Some(stored) = guard.as_mut() else {
                return Err(WatchPartyError::NotFound);
            };
            stored.invited.extend(users.iter().cloned());
            stored.updated_at = now_unix_ms();
        }
        let frame = WatchPartyFrame::Invite {
            party_id: party_id_bytes,
            video_id: party.video_id.clone(),
        };
        let mut failed = Vec::new();
        for user in &users {
            if self.send_to_id(user, &frame).await.is_err() {
                failed.push(user.clone());
            }
        }
        if !failed.is_empty() {
            self.remove_users(&failed).await;
        }
        let snapshot = self.broadcast_sync().await?;
        Ok(snapshot)
    }

    pub async fn accept(&self, party_id: &str) -> Result<WatchPartySnapshot, WatchPartyError> {
        let mut party = self.party_for_guest(party_id)?;
        let local_id = self.local.to_string();
        party.participants.insert(local_id);
        party.refresh_state();
        party.updated_at = now_unix_ms();
        let snapshot = self.store(party.clone());
        self.emit(WatchPartyEvent::Changed(snapshot.clone()));
        let frame = WatchPartyFrame::Accept {
            party_id: party_id_bytes(party_id)?,
        };
        if let Err(error) = self.send_to_id(&party.host_id, &frame).await {
            self.clear();
            return Err(error);
        }
        Ok(snapshot)
    }

    pub async fn decline(&self, party_id: &str) -> Result<(), WatchPartyError> {
        let party = self.party_for_guest(party_id)?;
        let _ = self
            .send_to_id(
                &party.host_id,
                &WatchPartyFrame::Decline {
                    party_id: party_id_bytes(party_id)?,
                },
            )
            .await;
        self.clear();
        self.emit(WatchPartyEvent::Ended {
            party_id: party_id.to_owned(),
            reason: "declined".to_owned(),
        });
        Ok(())
    }

    pub async fn leave(&self, party_id: &str) -> Result<(), WatchPartyError> {
        let party = self.party_for_guest(party_id)?;
        let _ = self
            .send_to_id(
                &party.host_id,
                &WatchPartyFrame::Leave {
                    party_id: party_id_bytes(party_id)?,
                },
            )
            .await;
        self.clear();
        self.emit(WatchPartyEvent::Ended {
            party_id: party_id.to_owned(),
            reason: "left".to_owned(),
        });
        Ok(())
    }

    pub async fn publish(
        &self,
        party_id: &str,
        position_ms: u64,
        playing: bool,
    ) -> Result<WatchPartySnapshot, WatchPartyError> {
        let mut party = self.party_for(party_id)?;
        let local_id = self.local.to_string();
        party.position_ms = position_ms;
        party.playing = playing;
        party.revision = party.revision.saturating_add(1);
        party.actor = local_id.clone();
        party.updated_at = now_unix_ms();
        if party.is_host() {
            self.store(party);
            let snapshot = self.broadcast_sync().await?;
            Ok(snapshot)
        } else {
            let host_id = party.host_id.clone();
            let snapshot = self.store(party);
            self.emit(WatchPartyEvent::Changed(snapshot.clone()));
            self.send_to_id(
                &host_id,
                &WatchPartyFrame::Command {
                    party_id: party_id_bytes(party_id)?,
                    position_ms,
                    playing,
                },
            )
            .await?;
            Ok(snapshot)
        }
    }

    pub async fn end(&self, party_id: &str) -> Result<(), WatchPartyError> {
        let party = self.party_for(party_id)?;
        if party.is_host() {
            self.finish("ended", true).await;
        } else {
            self.leave(party_id).await?;
        }
        Ok(())
    }

    pub async fn handle_frame(
        &self,
        user: UserInfo,
        frame: WatchPartyFrame,
    ) -> Result<(), WatchPartyError> {
        let node_id = user.node_id.to_string();
        match frame {
            WatchPartyFrame::Invite { party_id, video_id } => {
                if !valid_video_id(&video_id) {
                    return Err(WatchPartyError::InvalidVideo);
                }
                let existing = self.snapshot();
                if let Some(existing) = existing {
                    if existing.party_id != Uuid::from_bytes(party_id).to_string() {
                        let _ = self
                            .send_to_id(&node_id, &WatchPartyFrame::Decline { party_id })
                            .await;
                    }
                    return Ok(());
                }
                let local_id = self.local.to_string();
                let mut participants = BTreeSet::new();
                participants.insert(node_id.clone());
                let party = Party {
                    party_id: Uuid::from_bytes(party_id),
                    host_id: node_id,
                    video_id,
                    role: "guest".to_owned(),
                    state: "ringing".to_owned(),
                    participants,
                    invited: BTreeSet::new(),
                    position_ms: 0,
                    playing: false,
                    revision: 0,
                    actor: local_id,
                    updated_at: now_unix_ms(),
                };
                let snapshot = self.store(party);
                self.emit(WatchPartyEvent::Requested(snapshot));
            }
            WatchPartyFrame::Accept { party_id } => {
                let id = Uuid::from_bytes(party_id).to_string();
                let mut party = self.party_for_host(&id)?;
                if !party.invited.contains(&node_id) && !party.participants.contains(&node_id) {
                    return Err(WatchPartyError::NotParticipant);
                }
                party.invited.remove(&node_id);
                party.participants.insert(node_id);
                party.refresh_state();
                party.updated_at = now_unix_ms();
                self.store(party);
                self.broadcast_sync().await?;
            }
            WatchPartyFrame::Decline { party_id } => {
                let id = Uuid::from_bytes(party_id).to_string();
                self.remove_remote_user(&id, &node_id).await?;
            }
            WatchPartyFrame::Leave { party_id } => {
                let id = Uuid::from_bytes(party_id).to_string();
                self.remove_remote_user(&id, &node_id).await?;
            }
            WatchPartyFrame::Command {
                party_id,
                position_ms,
                playing,
            } => {
                let id = Uuid::from_bytes(party_id).to_string();
                let mut party = self.party_for_host(&id)?;
                if !party.participants.contains(&node_id) {
                    return Err(WatchPartyError::NotParticipant);
                }
                party.position_ms = position_ms;
                party.playing = playing;
                party.revision = party.revision.saturating_add(1);
                party.actor = node_id;
                party.updated_at = now_unix_ms();
                self.store(party);
                self.broadcast_sync().await?;
            }
            WatchPartyFrame::Sync {
                party_id,
                state,
                participants,
                invited,
                position_ms,
                playing,
                revision,
                actor,
                updated_at,
            } => {
                let id = Uuid::from_bytes(party_id).to_string();
                let mut party = self.party_for_guest(&id)?;
                if party.host_id != node_id {
                    return Err(WatchPartyError::WrongUser);
                }
                if revision < party.revision {
                    return Ok(());
                }
                party.state = state;
                party.participants = participants.into_iter().collect();
                party.invited = invited.into_iter().collect();
                party.position_ms = position_ms;
                party.playing = playing;
                party.revision = revision;
                party.actor = actor;
                party.updated_at = updated_at;
                let snapshot = self.store(party);
                self.emit(WatchPartyEvent::Changed(snapshot));
            }
            WatchPartyFrame::End { party_id } => {
                let id = Uuid::from_bytes(party_id).to_string();
                let party = self.party_for_guest(&id)?;
                if party.host_id != node_id {
                    return Err(WatchPartyError::WrongUser);
                }
                self.clear();
                self.emit(WatchPartyEvent::Ended {
                    party_id: id,
                    reason: "host ended".to_owned(),
                });
            }
        }
        Ok(())
    }

    pub async fn handle_disconnect(&self, user: &UserInfo) {
        let node_id = user.node_id.to_string();
        let Some(party) = self.party_for_snapshot() else {
            return;
        };
        if party.is_host() {
            if (party.participants.contains(&node_id) || party.invited.contains(&node_id))
                && let Err(error) = self
                    .remove_remote_user(&party.party_id.to_string(), &node_id)
                    .await
            {
                tracing::debug!(%error, "watch party disconnect cleanup failed");
            }
        } else if party.host_id == node_id {
            self.clear();
            self.emit(WatchPartyEvent::Ended {
                party_id: party.party_id.to_string(),
                reason: "host disconnected".to_owned(),
            });
        }
    }

    fn party_for_snapshot(&self) -> Option<Party> {
        self.party
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn party_for(&self, party_id: &str) -> Result<Party, WatchPartyError> {
        self.party_for_snapshot()
            .filter(|party| party.party_id.to_string() == party_id)
            .ok_or(WatchPartyError::NotFound)
    }

    fn party_for_host(&self, party_id: &str) -> Result<Party, WatchPartyError> {
        let party = self.party_for(party_id)?;
        if !party.is_host() {
            return Err(WatchPartyError::NotHost);
        }
        Ok(party)
    }

    fn party_for_guest(&self, party_id: &str) -> Result<Party, WatchPartyError> {
        let party = self.party_for(party_id)?;
        if party.is_host() {
            return Err(WatchPartyError::NotGuest);
        }
        Ok(party)
    }

    fn store(&self, party: Party) -> WatchPartySnapshot {
        let snapshot = party.snapshot();
        *self.party.write().unwrap_or_else(PoisonError::into_inner) = Some(party);
        snapshot
    }

    fn clear(&self) {
        *self.party.write().unwrap_or_else(PoisonError::into_inner) = None;
    }

    fn emit(&self, event: WatchPartyEvent) {
        let _ = self.events.send(event);
    }

    async fn remove_remote_user(
        &self,
        party_id: &str,
        user_id: &str,
    ) -> Result<(), WatchPartyError> {
        let mut party = self.party_for_host(party_id)?;
        party.participants.remove(user_id);
        party.invited.remove(user_id);
        party.refresh_state();
        party.updated_at = now_unix_ms();
        self.store(party);
        self.broadcast_sync().await?;
        Ok(())
    }

    async fn remove_users(&self, user_ids: &[String]) {
        let Some(mut party) = self.party_for_snapshot() else {
            return;
        };
        for user_id in user_ids {
            party.participants.remove(user_id);
            party.invited.remove(user_id);
        }
        party.refresh_state();
        party.updated_at = now_unix_ms();
        self.store(party);
        if let Err(error) = self.broadcast_sync().await {
            tracing::debug!(%error, "watch party sync after invite failure failed");
        }
    }

    async fn finish(&self, reason: &str, notify: bool) {
        let Some(party) = self.party_for_snapshot() else {
            return;
        };
        if notify {
            let frame = WatchPartyFrame::End {
                party_id: *party.party_id.as_bytes(),
            };
            let local = self.local.to_string();
            let recipients: BTreeSet<&String> = party
                .participants
                .iter()
                .chain(party.invited.iter())
                .filter(|id| id.as_str() != local.as_str())
                .collect();
            for user in recipients {
                let _ = self.send_to_id(user, &frame).await;
            }
        }
        self.clear();
        self.emit(WatchPartyEvent::Ended {
            party_id: party.party_id.to_string(),
            reason: reason.to_owned(),
        });
    }

    async fn broadcast_sync(&self) -> Result<WatchPartySnapshot, WatchPartyError> {
        let party = self.party_for_snapshot().ok_or(WatchPartyError::NotFound)?;
        let initial = party.snapshot();
        self.emit(WatchPartyEvent::Changed(initial.clone()));
        let frame = sync_frame(&party);
        let local = self.local.to_string();
        let participants: Vec<String> = party
            .participants
            .iter()
            .filter(|id| id.as_str() != local.as_str())
            .cloned()
            .collect();
        let frame = &frame;
        let results =
            futures_util::future::join_all(participants.into_iter().map(|user| async move {
                let result = self.send_to_id(&user, frame).await;
                (user, result)
            }))
            .await;
        let failed: Vec<String> = results
            .into_iter()
            .filter(|(_, result)| result.is_err())
            .map(|(user, _)| user)
            .collect();
        if failed.is_empty() {
            return Ok(initial);
        }
        let mut updated = party;
        for user in &failed {
            updated.participants.remove(user);
            updated.invited.remove(user);
        }
        updated.refresh_state();
        updated.updated_at = now_unix_ms();
        let snapshot = self.store(updated);
        self.emit(WatchPartyEvent::Changed(snapshot.clone()));
        Ok(snapshot)
    }

    async fn send_to_id(
        &self,
        user_id: &str,
        frame: &WatchPartyFrame,
    ) -> Result<(), WatchPartyError> {
        let node_id = NodeId::from_str(user_id).map_err(|_| WatchPartyError::InvalidUser)?;
        let session = self
            .transport
            .session_by_node_id(node_id)
            .ok_or(WatchPartyError::UserOffline)?;
        send_frame(&session, frame).await
    }
}

fn sync_frame(party: &Party) -> WatchPartyFrame {
    WatchPartyFrame::Sync {
        party_id: *party.party_id.as_bytes(),
        state: party.state.clone(),
        participants: party.participants.iter().cloned().collect(),
        invited: party.invited.iter().cloned().collect(),
        position_ms: party.position_ms,
        playing: party.playing,
        revision: party.revision,
        actor: party.actor.clone(),
        updated_at: party.updated_at,
    }
}

fn party_id_bytes(party_id: &str) -> Result<[u8; 16], WatchPartyError> {
    let uuid = Uuid::from_str(party_id).map_err(|_| WatchPartyError::NotFound)?;
    Ok(*uuid.as_bytes())
}

async fn send_frame(session: &Session, frame: &WatchPartyFrame) -> Result<(), WatchPartyError> {
    let mut send = session.open_uni().await?;
    ticonnect_net::transport::session::write_frame(&mut send, &frame.to_wire()?)
        .await
        .map_err(WatchPartyError::Transport)
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
                            service.handle_disconnect(&user).await;
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
