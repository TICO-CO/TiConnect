pub mod conversations;
mod error;
pub mod messages;
pub mod migrations;
pub mod outbox;
pub mod peer_state;
pub mod peers;

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use rusqlite::Connection;
use ticonnect_core::message::MessageRecord;
use tokio::sync::{mpsc as tokio_mpsc, oneshot};

pub use conversations::Conversation;
pub use error::StorageError;
pub use messages::{InsertOutcome, MessageState, StoredMessage};
pub use outbox::OutboxEntry;
pub use peer_state::PeerState;
pub use peers::{NewPeer, Peer, PeerPolicy};

type Reply<T> = oneshot::Sender<Result<T, StorageError>>;

#[derive(Debug)]
enum Command {
    CountPeers(Reply<u32>),
    OutboxDepth(Reply<u32>),
    ListPeers(Reply<Vec<Peer>>),
    GetPeer(String, Reply<Option<Peer>>),
    GetPeerByFingerprint(String, Reply<Option<Peer>>),
    InsertPeer(NewPeer, Reply<Peer>),
    UpdatePeerPolicy(String, PeerPolicy, Reply<bool>),
    UpdatePeerLastSeen(String, i64, Reply<bool>),
    DeletePeer(String, Reply<bool>),
    UpdatePeerPublicKey(String, [u8; 32], Reply<bool>),
    IntegrityCheck(Reply<String>),
    EnsureConversation {
        id: String,
        peer_id: String,
        created_at: i64,
        reply: Reply<Conversation>,
    },
    ConversationByPeer(String, Reply<Option<Conversation>>),
    AllocateSeq(String, Reply<u64>),
    InsertMessage(
        Box<MessageRecord>,
        i64,
        Vec<u8>,
        Option<MessageState>,
        Reply<InsertOutcome>,
    ),
    InsertInbound(
        Box<MessageRecord>,
        i64,
        Vec<u8>,
        String,
        Reply<InsertOutcome>,
    ),
    GetMessage(String, Reply<Option<StoredMessage>>),
    ListHistory {
        conversation_id: String,
        limit: u32,
        offset: u32,
        reply: Reply<Vec<StoredMessage>>,
    },
    CountHistory(String, Reply<u32>),
    RecordsBetween {
        conversation_id: String,
        sender_id: String,
        from_seq: u64,
        to_seq: u64,
        limit: u32,
        reply: Reply<Vec<StoredMessage>>,
    },
    SetMessageState(String, MessageState, Reply<bool>),
    MessageState(String, Reply<Option<MessageState>>),
    GetPeerState(String, Reply<PeerState>),
    AdvanceRecvSeq(String, u64, Reply<()>),
    EnqueueOutbox {
        message_id: String,
        next_try: i64,
        expires_at: i64,
        reply: Reply<()>,
    },
    DueOutbox(String, i64, Reply<Vec<OutboxEntry>>),
    DeferOutbox(String, i64, Reply<bool>),
    RemoveOutbox(String, Reply<bool>),
    ExpireOutbox(i64, Reply<Vec<String>>),
}

#[derive(Clone)]
pub struct DatabaseHandle {
    sender: tokio_mpsc::UnboundedSender<Command>,
}

impl DatabaseHandle {
    pub async fn spawn(path: impl Into<PathBuf>) -> Result<Self, StorageError> {
        let path = path.into();
        let (sender, receiver) = tokio_mpsc::unbounded_channel();
        let (ready_sender, ready_receiver) = oneshot::channel();
        let (thread_sender, thread_receiver) = mpsc::channel();

        std::thread::Builder::new()
            .name("ticonnect-db".to_owned())
            .spawn(move || {
                let _ = thread_sender.send(());
                run_worker(path, receiver, ready_sender);
            })?;

        let _ = thread_receiver.recv();
        match ready_receiver.await {
            Ok(Ok(())) => Ok(Self { sender }),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(StorageError::WorkerStopped),
        }
    }

    pub async fn count_peers(&self) -> Result<u32, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::CountPeers(reply))?;
        receive(receiver).await
    }

    pub async fn outbox_depth(&self) -> Result<u32, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::OutboxDepth(reply))?;
        receive(receiver).await
    }

    pub async fn list_peers(&self) -> Result<Vec<Peer>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::ListPeers(reply))?;
        receive(receiver).await
    }

    pub async fn get_peer(&self, id: &str) -> Result<Option<Peer>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::GetPeer(id.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn get_peer_by_fingerprint(
        &self,
        fingerprint: &str,
    ) -> Result<Option<Peer>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::GetPeerByFingerprint(fingerprint.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn insert_peer(&self, peer: NewPeer) -> Result<Peer, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::InsertPeer(peer, reply))?;
        receive(receiver).await
    }

    pub async fn update_peer_policy(
        &self,
        id: &str,
        policy: PeerPolicy,
    ) -> Result<bool, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::UpdatePeerPolicy(id.to_owned(), policy, reply))?;
        receive(receiver).await
    }

    pub async fn update_peer_last_seen(
        &self,
        id: &str,
        last_seen: i64,
    ) -> Result<bool, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::UpdatePeerLastSeen(id.to_owned(), last_seen, reply))?;
        receive(receiver).await
    }

    pub async fn delete_peer(&self, id: &str) -> Result<bool, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::DeletePeer(id.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn update_peer_public_key(
        &self,
        id: &str,
        public_key: [u8; 32],
    ) -> Result<bool, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::UpdatePeerPublicKey(
            id.to_owned(),
            public_key,
            reply,
        ))?;
        receive(receiver).await
    }

    pub async fn ensure_conversation(
        &self,
        id: &str,
        peer_id: &str,
        created_at: i64,
    ) -> Result<Conversation, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::EnsureConversation {
            id: id.to_owned(),
            peer_id: peer_id.to_owned(),
            created_at,
            reply,
        })?;
        receive(receiver).await
    }

    pub async fn conversation_by_peer(
        &self,
        peer_id: &str,
    ) -> Result<Option<Conversation>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::ConversationByPeer(peer_id.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn allocate_seq(&self, peer_id: &str) -> Result<u64, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::AllocateSeq(peer_id.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn insert_message(
        &self,
        record: MessageRecord,
        received_at: i64,
        signature: Vec<u8>,
        state: Option<MessageState>,
    ) -> Result<InsertOutcome, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::InsertMessage(
            Box::new(record),
            received_at,
            signature,
            state,
            reply,
        ))?;
        receive(receiver).await
    }

    pub async fn insert_inbound(
        &self,
        record: MessageRecord,
        received_at: i64,
        signature: Vec<u8>,
        peer_id: &str,
    ) -> Result<InsertOutcome, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::InsertInbound(
            Box::new(record),
            received_at,
            signature,
            peer_id.to_owned(),
            reply,
        ))?;
        receive(receiver).await
    }

    pub async fn get_message(&self, id: &str) -> Result<Option<StoredMessage>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::GetMessage(id.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn list_history(
        &self,
        conversation_id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<StoredMessage>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::ListHistory {
            conversation_id: conversation_id.to_owned(),
            limit,
            offset,
            reply,
        })?;
        receive(receiver).await
    }

    pub async fn count_history(&self, conversation_id: &str) -> Result<u32, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::CountHistory(conversation_id.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn records_between(
        &self,
        conversation_id: &str,
        sender_id: &str,
        from_seq: u64,
        to_seq: u64,
        limit: u32,
    ) -> Result<Vec<StoredMessage>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::RecordsBetween {
            conversation_id: conversation_id.to_owned(),
            sender_id: sender_id.to_owned(),
            from_seq,
            to_seq,
            limit,
            reply,
        })?;
        receive(receiver).await
    }

    pub async fn set_message_state(
        &self,
        id: &str,
        state: MessageState,
    ) -> Result<bool, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::SetMessageState(id.to_owned(), state, reply))?;
        receive(receiver).await
    }

    pub async fn message_state(&self, id: &str) -> Result<Option<MessageState>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::MessageState(id.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn peer_state(&self, peer_id: &str) -> Result<PeerState, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::GetPeerState(peer_id.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn advance_recv_seq(&self, peer_id: &str, seq: u64) -> Result<(), StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::AdvanceRecvSeq(peer_id.to_owned(), seq, reply))?;
        receive(receiver).await
    }

    pub async fn enqueue_outbox(
        &self,
        message_id: &str,
        next_try: i64,
        expires_at: i64,
    ) -> Result<(), StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::EnqueueOutbox {
            message_id: message_id.to_owned(),
            next_try,
            expires_at,
            reply,
        })?;
        receive(receiver).await
    }

    pub async fn due_outbox(
        &self,
        peer_id: &str,
        now: i64,
    ) -> Result<Vec<OutboxEntry>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::DueOutbox(peer_id.to_owned(), now, reply))?;
        receive(receiver).await
    }

    pub async fn defer_outbox(
        &self,
        message_id: &str,
        next_try: i64,
    ) -> Result<bool, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::DeferOutbox(message_id.to_owned(), next_try, reply))?;
        receive(receiver).await
    }

    pub async fn remove_outbox(&self, message_id: &str) -> Result<bool, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::RemoveOutbox(message_id.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn expire_outbox(&self, now: i64) -> Result<Vec<String>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::ExpireOutbox(now, reply))?;
        receive(receiver).await
    }

    pub async fn integrity_check(&self) -> Result<String, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::IntegrityCheck(reply))?;
        receive(receiver).await
    }

    fn send(&self, command: Command) -> Result<(), StorageError> {
        self.sender
            .send(command)
            .map_err(|_| StorageError::WorkerStopped)
    }
}

async fn receive<T>(
    receiver: oneshot::Receiver<Result<T, StorageError>>,
) -> Result<T, StorageError> {
    receiver.await.map_err(|_| StorageError::WorkerStopped)?
}

fn run_worker(
    path: PathBuf,
    mut receiver: tokio_mpsc::UnboundedReceiver<Command>,
    ready: oneshot::Sender<Result<(), StorageError>>,
) {
    let connection = match open(&path) {
        Ok(connection) => connection,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };

    if ready.send(Ok(())).is_err() {
        return;
    }

    while let Some(command) = receiver.blocking_recv() {
        dispatch(&connection, command);
    }

    let _ = connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
}

fn dispatch(connection: &Connection, command: Command) {
    match command {
        Command::CountPeers(reply) => {
            let _ = reply.send(peers::count(connection));
        }
        Command::OutboxDepth(reply) => {
            let _ = reply.send(peers::outbox_depth(connection));
        }
        Command::ListPeers(reply) => {
            let _ = reply.send(peers::list(connection));
        }
        Command::GetPeer(id, reply) => {
            let _ = reply.send(peers::get_by_id(connection, &id));
        }
        Command::GetPeerByFingerprint(fingerprint, reply) => {
            let _ = reply.send(peers::get_by_fingerprint(connection, &fingerprint));
        }
        Command::InsertPeer(peer, reply) => {
            let _ = reply.send(peers::insert(connection, &peer));
        }
        Command::UpdatePeerPolicy(id, policy, reply) => {
            let _ = reply.send(peers::update_policy(connection, &id, policy));
        }
        Command::UpdatePeerLastSeen(id, last_seen, reply) => {
            let _ = reply.send(peers::update_last_seen(connection, &id, last_seen));
        }
        Command::DeletePeer(id, reply) => {
            let _ = reply.send(peers::delete(connection, &id));
        }
        Command::UpdatePeerPublicKey(id, public_key, reply) => {
            let _ = reply.send(peers::update_public_key(connection, &id, &public_key));
        }
        Command::EnsureConversation {
            id,
            peer_id,
            created_at,
            reply,
        } => {
            let _ = reply.send(conversations::ensure(connection, &id, &peer_id, created_at));
        }
        Command::ConversationByPeer(peer_id, reply) => {
            let _ = reply.send(conversations::by_peer(connection, &peer_id));
        }
        Command::AllocateSeq(peer_id, reply) => {
            let _ = reply.send(peer_state::allocate_seq(connection, &peer_id));
        }
        Command::InsertMessage(record, received_at, signature, state, reply) => {
            let _ = reply.send(messages::insert(
                connection,
                &record,
                received_at,
                &signature,
                state,
            ));
        }
        Command::InsertInbound(record, received_at, signature, peer_id, reply) => {
            let _ = reply.send(messages::insert_inbound(
                connection,
                &record,
                received_at,
                &signature,
                &peer_id,
            ));
        }
        Command::GetMessage(id, reply) => {
            let _ = reply.send(messages::get(connection, &id));
        }
        Command::ListHistory {
            conversation_id,
            limit,
            offset,
            reply,
        } => {
            let _ = reply.send(messages::history(
                connection,
                &conversation_id,
                limit,
                offset,
            ));
        }
        Command::CountHistory(conversation_id, reply) => {
            let _ = reply.send(messages::count(connection, &conversation_id));
        }
        Command::RecordsBetween {
            conversation_id,
            sender_id,
            from_seq,
            to_seq,
            limit,
            reply,
        } => {
            let _ = reply.send(messages::records_between(
                connection,
                &conversation_id,
                &sender_id,
                from_seq,
                to_seq,
                limit,
            ));
        }
        Command::SetMessageState(id, state, reply) => {
            let _ = reply.send(messages::set_state(connection, &id, state));
        }
        Command::MessageState(id, reply) => {
            let _ = reply.send(messages::state(connection, &id));
        }
        Command::GetPeerState(peer_id, reply) => {
            let _ = reply.send(peer_state::get(connection, &peer_id));
        }
        Command::AdvanceRecvSeq(peer_id, seq, reply) => {
            let _ = reply.send(peer_state::advance_recv(connection, &peer_id, seq));
        }
        Command::EnqueueOutbox {
            message_id,
            next_try,
            expires_at,
            reply,
        } => {
            let _ = reply.send(outbox::enqueue(
                connection,
                &message_id,
                next_try,
                expires_at,
            ));
        }
        Command::DueOutbox(peer_id, now, reply) => {
            let _ = reply.send(outbox::due_for_peer(connection, &peer_id, now));
        }
        Command::DeferOutbox(message_id, next_try, reply) => {
            let _ = reply.send(outbox::defer(connection, &message_id, next_try));
        }
        Command::RemoveOutbox(message_id, reply) => {
            let _ = reply.send(outbox::remove(connection, &message_id));
        }
        Command::ExpireOutbox(now, reply) => {
            let _ = reply.send(outbox::expire(connection, now));
        }
        Command::IntegrityCheck(reply) => {
            let _ = reply.send(integrity_check(connection));
        }
    }
}

fn open(path: &Path) -> Result<Connection, StorageError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        create_private_dir(parent)?;
    }

    let mut connection = Connection::open(path)?;
    set_file_permissions(path)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    let _: String = connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    migrations::run(&mut connection)?;

    let result = integrity_check(&connection)?;
    if result != "ok" {
        return Err(StorageError::Integrity(result));
    }

    Ok(connection)
}

fn integrity_check(connection: &Connection) -> Result<String, StorageError> {
    Ok(connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?)
}

fn create_private_dir(dir: &Path) -> Result<(), StorageError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }

    #[cfg(not(unix))]
    std::fs::create_dir_all(dir)?;

    Ok(())
}

fn set_file_permissions(path: &Path) -> Result<(), StorageError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}
