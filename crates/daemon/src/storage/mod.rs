pub mod attachments;
pub mod conversations;
mod error;
pub mod messages;
pub mod migrations;
pub mod outbox;
pub mod profiles;
pub mod transfer_partials;
pub mod user_state;
pub mod users;

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use rusqlite::Connection;
use ticonnect_core::message::MessageRecord;
use tokio::sync::{mpsc as tokio_mpsc, oneshot};

pub use attachments::AttachmentRecord;
pub use conversations::Conversation;
pub use error::StorageError;
pub use messages::{InsertOutcome, MessageState, StoredMessage};
pub use outbox::OutboxEntry;
pub use profiles::UserProfile;
pub use transfer_partials::TransferPartial;
pub use user_state::UserState;
pub use users::{BlockedUser, NewUser, User, UserPolicy};

type Reply<T> = oneshot::Sender<Result<T, StorageError>>;

#[derive(Debug)]
enum Command {
    CountUsers(Reply<u32>),
    OutboxDepth(Reply<u32>),
    ListUsers(Reply<Vec<User>>),
    ListConversationSummaries(String, Reply<Vec<messages::ConversationSummary>>),
    GetUser(String, Reply<Option<User>>),
    GetUserByFingerprint(String, Reply<Option<User>>),
    InsertUser(NewUser, Reply<User>),
    UpdateUserPolicy(String, UserPolicy, Reply<bool>),
    UpdateUserLastSeen(String, i64, Reply<bool>),
    DeleteUser(String, Reply<bool>),
    UpdateUserPublicKey(String, [u8; 32], Reply<bool>),
    BlockUser(BlockedUser, Reply<()>),
    UnblockUser(String, Reply<bool>),
    ListBlockedUsers(Reply<Vec<BlockedUser>>),
    IsUserBlocked(String, Reply<bool>),
    IntegrityCheck(Reply<String>),
    EnsureConversation {
        id: String,
        user_id: String,
        created_at: i64,
        reply: Reply<Conversation>,
    },
    ConversationByUser(String, Reply<Option<Conversation>>),
    ConversationById(String, Reply<Option<Conversation>>),
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
    MarkMessagesRead {
        conversation_id: String,
        message_ids: Vec<String>,
        read_at: i64,
        reply: Reply<Vec<String>>,
    },
    MarkConversationRead {
        conversation_id: String,
        local_sender_id: String,
        read_at: i64,
        reply: Reply<Vec<String>>,
    },
    MessageState(String, Reply<Option<MessageState>>),
    GetUserState(String, Reply<UserState>),
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
    UpsertAttachment(AttachmentRecord, Reply<()>),
    GetAttachment(String, Reply<Option<AttachmentRecord>>),
    ListAttachments(Reply<Vec<AttachmentRecord>>),
    UpsertTransferPartial(TransferPartial, Reply<()>),
    GetTransferPartial(String, Reply<Option<TransferPartial>>),
    ListTransferPartials(Reply<Vec<TransferPartial>>),
    RemoveTransferPartial(String, Reply<bool>),
    UpsertUserProfile(UserProfile, Reply<()>),
    GetUserProfile(String, Reply<Option<UserProfile>>),
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

    pub async fn count_users(&self) -> Result<u32, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::CountUsers(reply))?;
        receive(receiver).await
    }

    pub async fn outbox_depth(&self) -> Result<u32, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::OutboxDepth(reply))?;
        receive(receiver).await
    }

    pub async fn list_users(&self) -> Result<Vec<User>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::ListUsers(reply))?;
        receive(receiver).await
    }

    pub async fn get_user(&self, id: &str) -> Result<Option<User>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::GetUser(id.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn list_conversation_summaries(
        &self,
        local_node_id: &str,
    ) -> Result<Vec<messages::ConversationSummary>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::ListConversationSummaries(
            local_node_id.to_owned(),
            reply,
        ))?;
        receive(receiver).await
    }

    pub async fn upsert_user_profile(&self, profile: UserProfile) -> Result<(), StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::UpsertUserProfile(profile, reply))?;
        receive(receiver).await
    }

    pub async fn get_user_profile(
        &self,
        fingerprint: &str,
    ) -> Result<Option<UserProfile>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::GetUserProfile(fingerprint.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn get_user_by_fingerprint(
        &self,
        fingerprint: &str,
    ) -> Result<Option<User>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::GetUserByFingerprint(fingerprint.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn insert_user(&self, user: NewUser) -> Result<User, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::InsertUser(user, reply))?;
        receive(receiver).await
    }

    pub async fn update_user_policy(
        &self,
        id: &str,
        policy: UserPolicy,
    ) -> Result<bool, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::UpdateUserPolicy(id.to_owned(), policy, reply))?;
        receive(receiver).await
    }

    pub async fn update_user_last_seen(
        &self,
        id: &str,
        last_seen: i64,
    ) -> Result<bool, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::UpdateUserLastSeen(id.to_owned(), last_seen, reply))?;
        receive(receiver).await
    }

    pub async fn delete_user(&self, id: &str) -> Result<bool, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::DeleteUser(id.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn update_user_public_key(
        &self,
        id: &str,
        public_key: [u8; 32],
    ) -> Result<bool, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::UpdateUserPublicKey(
            id.to_owned(),
            public_key,
            reply,
        ))?;
        receive(receiver).await
    }

    pub async fn block_user(&self, user: BlockedUser) -> Result<(), StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::BlockUser(user, reply))?;
        receive(receiver).await
    }

    pub async fn unblock_user(&self, fingerprint: &str) -> Result<bool, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::UnblockUser(fingerprint.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn list_blocked_users(&self) -> Result<Vec<BlockedUser>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::ListBlockedUsers(reply))?;
        receive(receiver).await
    }

    pub async fn is_user_blocked(&self, fingerprint: &str) -> Result<bool, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::IsUserBlocked(fingerprint.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn ensure_conversation(
        &self,
        id: &str,
        user_id: &str,
        created_at: i64,
    ) -> Result<Conversation, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::EnsureConversation {
            id: id.to_owned(),
            user_id: user_id.to_owned(),
            created_at,
            reply,
        })?;
        receive(receiver).await
    }

    pub async fn conversation_by_user(
        &self,
        user_id: &str,
    ) -> Result<Option<Conversation>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::ConversationByUser(user_id.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn conversation_by_id(
        &self,
        conversation_id: &str,
    ) -> Result<Option<Conversation>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::ConversationById(conversation_id.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn allocate_seq(&self, user_id: &str) -> Result<u64, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::AllocateSeq(user_id.to_owned(), reply))?;
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
        user_id: &str,
    ) -> Result<InsertOutcome, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::InsertInbound(
            Box::new(record),
            received_at,
            signature,
            user_id.to_owned(),
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

    pub async fn mark_messages_read(
        &self,
        conversation_id: &str,
        message_ids: &[String],
        read_at: i64,
    ) -> Result<Vec<String>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::MarkMessagesRead {
            conversation_id: conversation_id.to_owned(),
            message_ids: message_ids.to_vec(),
            read_at,
            reply,
        })?;
        receive(receiver).await
    }

    pub async fn mark_conversation_read(
        &self,
        conversation_id: &str,
        local_sender_id: &str,
        read_at: i64,
    ) -> Result<Vec<String>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::MarkConversationRead {
            conversation_id: conversation_id.to_owned(),
            local_sender_id: local_sender_id.to_owned(),
            read_at,
            reply,
        })?;
        receive(receiver).await
    }

    pub async fn user_state(&self, user_id: &str) -> Result<UserState, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::GetUserState(user_id.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn advance_recv_seq(&self, user_id: &str, seq: u64) -> Result<(), StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::AdvanceRecvSeq(user_id.to_owned(), seq, reply))?;
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
        user_id: &str,
        now: i64,
    ) -> Result<Vec<OutboxEntry>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::DueOutbox(user_id.to_owned(), now, reply))?;
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

    pub async fn upsert_attachment(
        &self,
        attachment: AttachmentRecord,
    ) -> Result<(), StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::UpsertAttachment(attachment, reply))?;
        receive(receiver).await
    }

    pub async fn get_attachment(
        &self,
        hash: &str,
    ) -> Result<Option<AttachmentRecord>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::GetAttachment(hash.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn list_attachments(&self) -> Result<Vec<AttachmentRecord>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::ListAttachments(reply))?;
        receive(receiver).await
    }

    pub async fn upsert_transfer_partial(
        &self,
        partial: TransferPartial,
    ) -> Result<(), StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::UpsertTransferPartial(partial, reply))?;
        receive(receiver).await
    }

    pub async fn get_transfer_partial(
        &self,
        hash: &str,
    ) -> Result<Option<TransferPartial>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::GetTransferPartial(hash.to_owned(), reply))?;
        receive(receiver).await
    }

    pub async fn list_transfer_partials(&self) -> Result<Vec<TransferPartial>, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::ListTransferPartials(reply))?;
        receive(receiver).await
    }

    pub async fn remove_transfer_partial(&self, hash: &str) -> Result<bool, StorageError> {
        let (reply, receiver) = oneshot::channel();
        self.send(Command::RemoveTransferPartial(hash.to_owned(), reply))?;
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
        Command::CountUsers(reply) => {
            let _ = reply.send(users::count(connection));
        }
        Command::OutboxDepth(reply) => {
            let _ = reply.send(users::outbox_depth(connection));
        }
        Command::ListUsers(reply) => {
            let _ = reply.send(users::list(connection));
        }
        Command::ListConversationSummaries(local_node_id, reply) => {
            let _ = reply.send(messages::conversation_summaries(connection, &local_node_id));
        }
        Command::GetUser(id, reply) => {
            let _ = reply.send(users::get_by_id(connection, &id));
        }
        Command::GetUserByFingerprint(fingerprint, reply) => {
            let _ = reply.send(users::get_by_fingerprint(connection, &fingerprint));
        }
        Command::InsertUser(user, reply) => {
            let _ = reply.send(users::insert(connection, &user));
        }
        Command::UpdateUserPolicy(id, policy, reply) => {
            let _ = reply.send(users::update_policy(connection, &id, policy));
        }
        Command::UpdateUserLastSeen(id, last_seen, reply) => {
            let _ = reply.send(users::update_last_seen(connection, &id, last_seen));
        }
        Command::DeleteUser(id, reply) => {
            let _ = reply.send(users::delete(connection, &id));
        }
        Command::UpdateUserPublicKey(id, public_key, reply) => {
            let _ = reply.send(users::update_public_key(connection, &id, &public_key));
        }
        Command::BlockUser(user, reply) => {
            let _ = reply.send(users::block(connection, &user));
        }
        Command::UnblockUser(fingerprint, reply) => {
            let _ = reply.send(users::unblock(connection, &fingerprint));
        }
        Command::ListBlockedUsers(reply) => {
            let _ = reply.send(users::list_blocked(connection));
        }
        Command::IsUserBlocked(fingerprint, reply) => {
            let _ = reply.send(users::is_blocked(connection, &fingerprint));
        }
        Command::EnsureConversation {
            id,
            user_id,
            created_at,
            reply,
        } => {
            let _ = reply.send(conversations::ensure(connection, &id, &user_id, created_at));
        }
        Command::ConversationByUser(user_id, reply) => {
            let _ = reply.send(conversations::by_user(connection, &user_id));
        }
        Command::ConversationById(conversation_id, reply) => {
            let _ = reply.send(conversations::by_id(connection, &conversation_id));
        }
        Command::AllocateSeq(user_id, reply) => {
            let _ = reply.send(user_state::allocate_seq(connection, &user_id));
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
        Command::InsertInbound(record, received_at, signature, user_id, reply) => {
            let _ = reply.send(messages::insert_inbound(
                connection,
                &record,
                received_at,
                &signature,
                &user_id,
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
        Command::MarkMessagesRead {
            conversation_id,
            message_ids,
            read_at,
            reply,
        } => {
            let _ = reply.send(messages::mark_read(
                connection,
                &conversation_id,
                &message_ids,
                read_at,
            ));
        }
        Command::MarkConversationRead {
            conversation_id,
            local_sender_id,
            read_at,
            reply,
        } => {
            let _ = reply.send(messages::mark_conversation_read(
                connection,
                &conversation_id,
                &local_sender_id,
                read_at,
            ));
        }
        Command::MessageState(id, reply) => {
            let _ = reply.send(messages::state(connection, &id));
        }
        Command::GetUserState(user_id, reply) => {
            let _ = reply.send(user_state::get(connection, &user_id));
        }
        Command::AdvanceRecvSeq(user_id, seq, reply) => {
            let _ = reply.send(user_state::advance_recv(connection, &user_id, seq));
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
        Command::DueOutbox(user_id, now, reply) => {
            let _ = reply.send(outbox::due_for_user(connection, &user_id, now));
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
        Command::UpsertAttachment(attachment, reply) => {
            let _ = reply.send(attachments::upsert(connection, &attachment));
        }
        Command::GetAttachment(hash, reply) => {
            let _ = reply.send(attachments::get(connection, &hash));
        }
        Command::ListAttachments(reply) => {
            let _ = reply.send(attachments::list(connection));
        }
        Command::UpsertTransferPartial(partial, reply) => {
            let _ = reply.send(transfer_partials::upsert(connection, &partial));
        }
        Command::GetTransferPartial(hash, reply) => {
            let _ = reply.send(transfer_partials::get(connection, &hash));
        }
        Command::ListTransferPartials(reply) => {
            let _ = reply.send(transfer_partials::list(connection));
        }
        Command::RemoveTransferPartial(hash, reply) => {
            let _ = reply.send(transfer_partials::remove(connection, &hash));
        }
        Command::UpsertUserProfile(profile, reply) => {
            let _ = reply.send(profiles::upsert(connection, &profile));
        }
        Command::GetUserProfile(fingerprint, reply) => {
            let _ = reply.send(profiles::get(connection, &fingerprint));
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
