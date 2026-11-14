pub mod store;
pub mod transfer;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::time::Duration;

use thiserror::Error;
use ticonnect_core::attachment::{AttachmentHash, AttachmentRef};
use ticonnect_core::id::NodeId;
use ticonnect_core::message::{MessageKind, MessageRecord};
use ticonnect_net::transfer::{Chunk, DEFAULT_CHUNK_SIZE, ResumeError, ResumePlan, TransferFrame};
use ticonnect_net::transport::{PeerInfo, Transport};
use ticonnect_settings::Settings;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use crate::messaging::{MessagingEvent, now_unix_ms};
use crate::storage::{AttachmentRecord, DatabaseHandle, PeerPolicy, StorageError, TransferPartial};

use store::{AttachmentStore, StoreError};
use transfer::InboundTransfer;

pub const TRANSFER_TIMEOUT: Duration = Duration::from_secs(30);
pub const MAX_CONCURRENT_TRANSFERS: usize = 8;
pub const MAX_TRANSFERS_PER_PEER: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferPhase {
    Requesting,
    Receiving,
    Verifying,
    Complete,
    Failed,
    Cancelled,
}

impl TransferPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Requesting => "requesting",
            Self::Receiving => "receiving",
            Self::Verifying => "verifying",
            Self::Complete => "complete",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferSnapshot {
    pub hash: String,
    pub peer_id: String,
    pub name: String,
    pub size: u64,
    pub bytes: u64,
    pub phase: TransferPhase,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub enum TransferEvent {
    Progress(TransferSnapshot),
    Done { hash: String },
    Failed { hash: String, reason: String },
}

#[derive(Clone)]
pub struct AttachmentService {
    inner: Arc<Inner>,
}

struct Inner {
    database: DatabaseHandle,
    store: AttachmentStore,
    transport: Arc<Transport>,
    settings: RwLock<Settings>,
    events: tokio::sync::broadcast::Sender<TransferEvent>,
    transfers: Mutex<HashMap<String, TransferSnapshot>>,
    cancelled: Mutex<HashMap<String, Arc<AtomicBool>>>,
    active: Mutex<HashMap<String, usize>>,
}

impl AttachmentService {
    pub fn new(
        database: DatabaseHandle,
        root: impl Into<PathBuf>,
        transport: Arc<Transport>,
        settings: Settings,
    ) -> Result<Self, AttachmentsError> {
        let (events, _) = tokio::sync::broadcast::channel(128);
        Ok(Self {
            inner: Arc::new(Inner {
                database,
                store: AttachmentStore::new(root)?,
                transport,
                settings: RwLock::new(settings),
                events,
                transfers: Mutex::new(HashMap::new()),
                cancelled: Mutex::new(HashMap::new()),
                active: Mutex::new(HashMap::new()),
            }),
        })
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<TransferEvent> {
        self.inner.events.subscribe()
    }

    pub fn apply_settings(&self, settings: Settings) {
        *self
            .inner
            .settings
            .write()
            .unwrap_or_else(PoisonError::into_inner) = settings;
    }

    pub fn settings(&self) -> Settings {
        self.inner
            .settings
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn store(&self) -> &AttachmentStore {
        &self.inner.store
    }

    pub fn database(&self) -> &DatabaseHandle {
        &self.inner.database
    }

    pub async fn prepare_file(
        &self,
        path: &std::path::Path,
        mime: Option<&str>,
    ) -> Result<AttachmentRef, AttachmentsError> {
        let mut attachment = self.inner.store.put_file(path)?;
        if let Some(mime) = mime.map(str::trim).filter(|mime| !mime.is_empty()) {
            attachment.mime = mime.to_owned();
            attachment
                .validate(self.max_size())
                .map_err(|error| AttachmentsError::Invalid(error.to_string()))?;
        }
        self.validate(&attachment)?;
        Ok(attachment)
    }

    pub fn validate(&self, attachment: &AttachmentRef) -> Result<(), AttachmentsError> {
        attachment
            .validate(self.max_size())
            .map_err(|error| AttachmentsError::Invalid(error.to_string()))?;
        Ok(())
    }

    pub fn max_size(&self) -> u64 {
        u64::from(self.settings().attachment_max_size_mib()) * 1024 * 1024
    }

    pub async fn record_local(
        &self,
        attachment: &AttachmentRef,
    ) -> Result<AttachmentRecord, AttachmentsError> {
        let record = AttachmentRecord {
            hash: attachment.hash.to_hex(),
            size: attachment.size,
            path: self
                .inner
                .store
                .path_for(attachment.hash)
                .display()
                .to_string(),
            mime: attachment.mime.clone(),
            name: attachment.name.clone(),
            created_at: now_unix_ms(),
        };
        self.inner
            .database
            .upsert_attachment(record.clone())
            .await?;
        Ok(record)
    }

    pub async fn get(&self, hash: &str) -> Result<Option<AttachmentRecord>, AttachmentsError> {
        Ok(self.inner.database.get_attachment(hash).await?)
    }

    pub async fn save(
        &self,
        hash: &str,
        destination: &std::path::Path,
    ) -> Result<bool, AttachmentsError> {
        let parsed = hash
            .parse::<AttachmentHash>()
            .map_err(|_| AttachmentsError::InvalidHash)?;
        let Some(record) = self.inner.database.get_attachment(hash).await? else {
            return Err(AttachmentsError::Unknown(hash.to_owned()));
        };
        if !self.inner.store.contains(parsed) {
            return Err(AttachmentsError::Unknown(hash.to_owned()));
        }
        let _ = record;
        self.inner.store.export(parsed, destination)?;
        Ok(true)
    }

    pub async fn path(&self, hash: &str) -> Result<Option<PathBuf>, AttachmentsError> {
        let parsed = hash
            .parse::<AttachmentHash>()
            .map_err(|_| AttachmentsError::InvalidHash)?;
        if !self.inner.store.contains(parsed) {
            return Ok(None);
        }
        Ok(Some(self.inner.store.path_for(parsed)))
    }

    pub fn transfer_state(&self, hash: &str) -> Option<TransferSnapshot> {
        self.inner
            .transfers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(hash)
            .cloned()
    }

    pub fn cancel(&self, hash: &str) -> bool {
        if let Some(flag) = self
            .inner
            .cancelled
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(hash)
        {
            flag.store(true, Ordering::Relaxed);
            return true;
        }
        false
    }

    pub fn cleanup_orphans(&self) -> Result<usize, AttachmentsError> {
        Ok(self.inner.store.cleanup_orphans()?)
    }

    async fn on_message(
        &self,
        peer_id: &str,
        record: &MessageRecord,
        notify: bool,
    ) -> Result<(), AttachmentsError> {
        let Some(attachment) = record
            .attachment_ref()
            .map_err(|error| AttachmentsError::Invalid(error.to_string()))?
        else {
            return Ok(());
        };
        self.validate(&attachment)?;
        if record.kind == MessageKind::Ack {
            return Ok(());
        }
        let Some(peer) = self.inner.database.get_peer(peer_id).await? else {
            return Ok(());
        };
        if peer.policy == PeerPolicy::Ignore {
            return Ok(());
        }
        if self.inner.store.contains(attachment.hash) {
            self.record_local(&attachment).await?;
            self.emit(TransferEvent::Done {
                hash: attachment.hash.to_hex(),
            });
            return Ok(());
        }
        let _ = notify;
        let service = self.clone();
        let peer_id = peer_id.to_owned();
        tokio::spawn(async move {
            if let Err(error) = service.receive(&peer_id, attachment).await {
                tracing::debug!(peer_id, %error, "attachment receive failed");
            }
        });
        Ok(())
    }

    pub async fn receive_attachment(
        &self,
        peer_id: &str,
        attachment: AttachmentRef,
    ) -> Result<(), AttachmentsError> {
        self.receive(peer_id, attachment).await
    }

    async fn receive(
        &self,
        peer_id: &str,
        attachment: AttachmentRef,
    ) -> Result<(), AttachmentsError> {
        let hash = attachment.hash.to_hex();
        self.reserve(peer_id, &hash)?;
        let cancel = Arc::new(AtomicBool::new(false));
        self.inner
            .cancelled
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(hash.clone(), cancel.clone());
        let result = self.receive_inner(peer_id, &attachment, &cancel).await;
        self.release(peer_id, &hash);
        self.inner
            .cancelled
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&hash);
        if result.is_err() {
            let reason = result
                .as_ref()
                .err()
                .map(ToString::to_string)
                .unwrap_or_default();
            self.set_phase(&hash, TransferPhase::Failed, reason.clone());
            self.emit(TransferEvent::Failed {
                hash: hash.clone(),
                reason,
            });
        }
        result.map(|_| ())
    }

    async fn receive_inner(
        &self,
        peer_id: &str,
        attachment: &AttachmentRef,
        cancel: &AtomicBool,
    ) -> Result<(), AttachmentsError> {
        let hash = attachment.hash.to_hex();
        if attachment.size == 0 {
            self.inner.store.put_bytes(attachment.hash, &[])?;
            self.record_local(attachment).await?;
            self.set_phase(&hash, TransferPhase::Complete, String::new());
            self.progress(peer_id, attachment, 0, TransferPhase::Complete);
            self.emit(TransferEvent::Done { hash });
            return Ok(());
        }
        let peer_node: NodeId = peer_id
            .parse()
            .map_err(|_| AttachmentsError::InvalidPeer(peer_id.to_owned()))?;
        let session = self
            .inner
            .transport
            .session_by_node_id(peer_node)
            .ok_or(AttachmentsError::PeerOffline)?;

        let mut plan = ResumePlan::new(attachment.size);
        if let Some(partial) = self.inner.database.get_transfer_partial(&hash).await?
            && partial.size == attachment.size
            && partial.peer_id == peer_id
        {
            for (start, end) in parse_ranges(&partial.ranges) {
                let _ = plan.add(start, end);
            }
        }
        let temp = self.inner.store.partial_path(attachment.hash);
        if !plan.received().is_empty()
            && (!temp.is_file()
                || std::fs::metadata(&temp)
                    .map(|metadata| metadata.len() != plan.received_bytes())
                    .unwrap_or(true))
        {
            plan = ResumePlan::new(attachment.size);
            self.inner.store.cleanup_partial(attachment.hash)?;
        }
        self.set_phase(&hash, TransferPhase::Requesting, String::new());
        self.progress(
            peer_id,
            attachment,
            plan.received_bytes(),
            TransferPhase::Requesting,
        );

        let (mut send, mut recv) = session.open_bi().await?;
        let ranges = plan.missing(DEFAULT_CHUNK_SIZE as u64);
        if ranges.is_empty() {
            plan = ResumePlan::new(attachment.size);
        }
        let request = TransferFrame::request(
            attachment.hash,
            attachment.size,
            if ranges.is_empty() {
                plan.missing(DEFAULT_CHUNK_SIZE as u64)
            } else {
                ranges
            },
        );
        ticonnect_net::transport::session::write_stream_frame(
            &mut send,
            &request
                .to_wire()
                .map_err(|error| AttachmentsError::Protocol(error.to_string()))?,
        )
        .await?;

        let mut file = open_partial(&temp)?;
        self.set_phase(&hash, TransferPhase::Receiving, String::new());
        loop {
            if cancel.load(Ordering::Relaxed) {
                self.set_phase(&hash, TransferPhase::Cancelled, "cancelled".to_owned());
                self.emit(TransferEvent::Failed {
                    hash: hash.clone(),
                    reason: "cancelled".to_owned(),
                });
                self.inner.store.cleanup_partial(attachment.hash)?;
                let _ = self.inner.database.remove_transfer_partial(&hash).await;
                return Err(AttachmentsError::Cancelled);
            }
            let frame = timeout(
                TRANSFER_TIMEOUT,
                ticonnect_net::transport::session::read_frame(&mut recv),
            )
            .await
            .map_err(|_| AttachmentsError::Timeout)??;
            let frame = TransferFrame::from_wire(&frame)
                .map_err(|error| AttachmentsError::Protocol(error.to_string()))?;
            match frame {
                TransferFrame::Chunk {
                    hash: chunk_hash,
                    offset,
                    data,
                } => {
                    if chunk_hash != *attachment.hash.as_bytes() {
                        return Err(AttachmentsError::Invalid(
                            "chunk hash does not match the attachment".to_owned(),
                        ));
                    }
                    if !plan.accepts(offset, data.len()) {
                        return Err(AttachmentsError::Invalid(
                            "chunk range is not missing or is out of bounds".to_owned(),
                        ));
                    }
                    let chunk = Chunk::new(attachment.hash, offset, data)
                        .map_err(|error| AttachmentsError::Invalid(error.to_string()))?;
                    transfer::write_chunk(&mut file, &chunk)?;
                    plan.add(offset, chunk.end())
                        .map_err(|error| AttachmentsError::Invalid(error.to_string()))?;
                    self.progress(
                        peer_id,
                        attachment,
                        plan.received_bytes(),
                        TransferPhase::Receiving,
                    );
                    self.persist_partial(peer_id, attachment, &plan, &temp)
                        .await?;
                }
                TransferFrame::Complete { .. } => break,
                TransferFrame::Error { code } => {
                    return Err(AttachmentsError::Protocol(format!(
                        "sender rejected transfer with code {code}"
                    )));
                }
                _ => {
                    return Err(AttachmentsError::Protocol(
                        "unexpected transfer frame".to_owned(),
                    ));
                }
            }
        }
        file.sync_all()?;
        self.set_phase(&hash, TransferPhase::Verifying, String::new());
        self.progress(
            peer_id,
            attachment,
            plan.received_bytes(),
            TransferPhase::Verifying,
        );
        if !plan.is_complete() {
            self.inner.store.cleanup_partial(attachment.hash)?;
            let _ = self.inner.database.remove_transfer_partial(&hash).await;
            return Err(AttachmentsError::Incomplete);
        }
        self.inner.store.publish_partial(attachment.hash)?;
        self.record_local(attachment).await?;
        let _ = self.inner.database.remove_transfer_partial(&hash).await;
        self.set_phase(&hash, TransferPhase::Complete, String::new());
        self.progress(
            peer_id,
            attachment,
            attachment.size,
            TransferPhase::Complete,
        );
        self.emit(TransferEvent::Done { hash });
        Ok(())
    }

    async fn persist_partial(
        &self,
        peer_id: &str,
        attachment: &AttachmentRef,
        plan: &ResumePlan,
        temp: &std::path::Path,
    ) -> Result<(), AttachmentsError> {
        self.inner
            .database
            .upsert_transfer_partial(TransferPartial {
                hash: attachment.hash.to_hex(),
                peer_id: peer_id.to_owned(),
                size: attachment.size,
                bytes_received: plan.received_bytes(),
                temp_path: temp.display().to_string(),
                ranges: encode_ranges(plan),
                updated_at: now_unix_ms(),
            })
            .await?;
        Ok(())
    }

    pub async fn serve_attachment(
        &self,
        peer: PeerInfo,
        send: quinn::SendStream,
        recv: quinn::RecvStream,
        frame: TransferFrame,
    ) -> Result<(), AttachmentsError> {
        self.serve(peer, send, recv, frame).await
    }

    async fn serve(
        &self,
        peer: PeerInfo,
        mut send: quinn::SendStream,
        _recv: quinn::RecvStream,
        frame: TransferFrame,
    ) -> Result<(), AttachmentsError> {
        let TransferFrame::Request { hash, size, ranges } = frame else {
            return Ok(());
        };
        let hash = AttachmentHash::from_bytes(hash);
        if !self.inner.store.contains(hash) {
            ticonnect_net::transport::session::write_stream_frame(
                &mut send,
                &TransferFrame::Error { code: 1 }
                    .to_wire()
                    .map_err(|error| AttachmentsError::Protocol(error.to_string()))?,
            )
            .await?;
            return Ok(());
        }
        let actual = self.inner.store.size(hash)?;
        if actual != size {
            ticonnect_net::transport::session::write_stream_frame(
                &mut send,
                &TransferFrame::Error { code: 2 }
                    .to_wire()
                    .map_err(|error| AttachmentsError::Protocol(error.to_string()))?,
            )
            .await?;
            return Ok(());
        }
        for (start, end) in ranges {
            if start >= end || end > size {
                continue;
            }
            let mut cursor = start;
            while cursor < end {
                let length = usize::try_from((end - cursor).min(DEFAULT_CHUNK_SIZE as u64))
                    .unwrap_or(DEFAULT_CHUNK_SIZE);
                let data = self.inner.store.read_chunk(hash, cursor, length)?;
                if data.is_empty() {
                    break;
                }
                let chunk = Chunk::new(hash, cursor, data)
                    .map_err(|error| AttachmentsError::Invalid(error.to_string()))?;
                let frame = TransferFrame::chunk(chunk);
                ticonnect_net::transport::session::write_stream_frame(
                    &mut send,
                    &frame
                        .to_wire()
                        .map_err(|error| AttachmentsError::Protocol(error.to_string()))?,
                )
                .await?;
                cursor += length as u64;
            }
        }
        ticonnect_net::transport::session::write_stream_frame(
            &mut send,
            &TransferFrame::complete(hash)
                .to_wire()
                .map_err(|error| AttachmentsError::Protocol(error.to_string()))?,
        )
        .await?;
        tracing::debug!(peer = %peer.node_id, hash = %hash, "served attachment transfer");
        Ok(())
    }

    fn reserve(&self, peer_id: &str, hash: &str) -> Result<(), AttachmentsError> {
        let mut active = self
            .inner
            .active
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let total: usize = active.values().sum();
        if total >= MAX_CONCURRENT_TRANSFERS {
            return Err(AttachmentsError::Busy);
        }
        let per_peer = active.get(peer_id).copied().unwrap_or(0);
        if per_peer >= MAX_TRANSFERS_PER_PEER {
            return Err(AttachmentsError::Busy);
        }
        active.insert(peer_id.to_owned(), per_peer + 1);
        self.inner
            .transfers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(
                hash.to_owned(),
                TransferSnapshot {
                    hash: hash.to_owned(),
                    peer_id: peer_id.to_owned(),
                    name: String::new(),
                    size: 0,
                    bytes: 0,
                    phase: TransferPhase::Requesting,
                    reason: String::new(),
                },
            );
        Ok(())
    }

    fn release(&self, peer_id: &str, hash: &str) {
        let mut active = self
            .inner
            .active
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(count) = active.get_mut(peer_id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                active.remove(peer_id);
            }
        }
        let _ = hash;
    }

    fn set_phase(&self, hash: &str, phase: TransferPhase, reason: String) {
        if let Some(snapshot) = self
            .inner
            .transfers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_mut(hash)
        {
            snapshot.phase = phase;
            snapshot.reason = reason;
        }
    }

    fn progress(
        &self,
        peer_id: &str,
        attachment: &AttachmentRef,
        bytes: u64,
        phase: TransferPhase,
    ) {
        let snapshot = TransferSnapshot {
            hash: attachment.hash.to_hex(),
            peer_id: peer_id.to_owned(),
            name: attachment.name.clone(),
            size: attachment.size,
            bytes,
            phase,
            reason: String::new(),
        };
        self.inner
            .transfers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(snapshot.hash.clone(), snapshot.clone());
        self.emit(TransferEvent::Progress(snapshot));
    }

    fn emit(&self, event: TransferEvent) {
        let _ = self.inner.events.send(event);
    }
}

pub fn spawn(
    service: AttachmentService,
    mut messaging: tokio::sync::broadcast::Receiver<MessagingEvent>,
    mut transfers: tokio::sync::mpsc::UnboundedReceiver<InboundTransfer>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                event = messaging.recv() => {
                    match event {
                        Ok(MessagingEvent::MessageReceived { peer_id, message, notify }) => {
                            if let Err(error) = service.on_message(&peer_id, &message.record, notify).await {
                                tracing::debug!(%error, "attachment message rejected");
                            }
                        }
                        Ok(_) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                transfer = transfers.recv() => {
                    let Some(transfer) = transfer else { break };
                    let InboundTransfer { peer, send, recv, frame } = transfer;
                    if let Err(error) = service.serve(peer, send, recv, frame).await {
                        tracing::debug!(%error, "attachment transfer serve failed");
                    }
                }
            }
        }
    })
}

fn open_partial(path: &std::path::Path) -> Result<std::fs::File, std::io::Error> {
    if let Ok(metadata) = std::fs::symlink_metadata(path)
        && metadata.file_type().is_symlink()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "partial path is a symlink",
        ));
    }
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}

fn encode_ranges(plan: &ResumePlan) -> String {
    plan.received()
        .iter()
        .map(|range| format!("{}:{}", range.start, range.end))
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_ranges(value: &str) -> Vec<(u64, u64)> {
    value
        .split(',')
        .filter_map(|entry| {
            let (start, end) = entry.split_once(':')?;
            Some((start.parse().ok()?, end.parse().ok()?))
        })
        .collect()
}

#[derive(Debug, Error)]
pub enum AttachmentsError {
    #[error(transparent)]
    Storage(#[from] StorageError),

    #[error(transparent)]
    Store(#[from] StoreError),

    #[error(transparent)]
    Transport(#[from] ticonnect_net::transport::TransportError),

    #[error(transparent)]
    Resume(#[from] ResumeError),

    #[error("attachment hash is invalid")]
    InvalidHash,

    #[error("attachment is invalid: {0}")]
    Invalid(String),

    #[error("unknown attachment: {0}")]
    Unknown(String),

    #[error("invalid peer id: {0}")]
    InvalidPeer(String),

    #[error("peer is offline")]
    PeerOffline,

    #[error("transfer timed out")]
    Timeout,

    #[error("transfer was cancelled")]
    Cancelled,

    #[error("transfer is incomplete")]
    Incomplete,

    #[error("too many concurrent transfers")]
    Busy,

    #[error("attachment I/O failed: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Chunk(#[from] ticonnect_net::transfer::ChunkError),

    #[error("transfer protocol error: {0}")]
    Protocol(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_round_trip() {
        let mut plan = ResumePlan::new(100);
        plan.add(0, 10).expect("add");
        plan.add(30, 40).expect("add");
        assert_eq!(encode_ranges(&plan), "0:10,30:40");
        assert_eq!(parse_ranges("0:10,30:40"), vec![(0, 10), (30, 40)]);
        assert!(parse_ranges("bad").is_empty());
    }
}
