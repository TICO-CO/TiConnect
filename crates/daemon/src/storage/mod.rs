mod error;
pub mod migrations;
pub mod peers;

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use rusqlite::Connection;
use tokio::sync::{mpsc as tokio_mpsc, oneshot};

pub use error::StorageError;
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
    IntegrityCheck(Reply<String>),
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
