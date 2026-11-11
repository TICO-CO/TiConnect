use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use thiserror::Error;
use ticonnect_core::fingerprint::Fingerprint;
use ticonnect_core::id::{IdError, NodeId};
use ticonnect_core::identity::IdentityKeyPair;

const KEY_FILE: &str = "identity.key";
const METADATA_FILE: &str = "identity.toml";

#[derive(Debug, Clone)]
pub struct PublicIdentity {
    node_id: NodeId,
    fingerprint: Fingerprint,
}

impl PublicIdentity {
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

pub struct Identity {
    key_pair: IdentityKeyPair,
    public: PublicIdentity,
}

impl Identity {
    pub fn load_or_create(config_dir: &Path) -> Result<Self, IdentityError> {
        create_private_dir(config_dir)?;
        let key_path = config_dir.join(KEY_FILE);
        let metadata_path = config_dir.join(METADATA_FILE);

        let (key_pair, created) = match fs::read(&key_path) {
            Ok(bytes) => (load_key(&key_path, &bytes)?, false),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let key_pair = IdentityKeyPair::generate();
                write_private_new(&key_path, &key_pair.secret_bytes())?;
                (key_pair, true)
            }
            Err(error) => return Err(IdentityError::Io(error)),
        };

        let fingerprint = Fingerprint::of(&key_pair.verifying_key());
        let node_id = if created {
            let node_id = NodeId::generate();
            write_metadata(&metadata_path, node_id)?;
            node_id
        } else {
            read_node_id(&metadata_path)?
        };

        tracing::info!(
            node_id = %node_id,
            fingerprint = %fingerprint,
            "identity loaded"
        );

        Ok(Self {
            key_pair,
            public: PublicIdentity {
                node_id,
                fingerprint,
            },
        })
    }

    pub fn public(&self) -> PublicIdentity {
        self.public.clone()
    }

    pub fn key_pair(&self) -> &IdentityKeyPair {
        &self.key_pair
    }
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Identity")
            .field("node_id", &self.public.node_id)
            .field("fingerprint", &self.public.fingerprint)
            .finish_non_exhaustive()
    }
}

fn load_key(path: &Path, bytes: &[u8]) -> Result<IdentityKeyPair, IdentityError> {
    check_key_permissions(path)?;
    let secret: [u8; 32] = bytes
        .try_into()
        .map_err(|_| IdentityError::InvalidKeyLength {
            path: path.to_path_buf(),
            size: bytes.len(),
        })?;
    Ok(IdentityKeyPair::from_secret_bytes(secret))
}

fn read_node_id(path: &Path) -> Result<NodeId, IdentityError> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(IdentityError::MissingMetadata);
        }
        Err(error) => return Err(IdentityError::Io(error)),
    };

    let metadata: IdentityMetadata =
        toml::from_str(&contents).map_err(|source| IdentityError::InvalidMetadata {
            path: path.to_path_buf(),
            source,
        })?;
    let node_id = metadata
        .node_id
        .parse()
        .map_err(IdentityError::InvalidNodeId)?;
    Ok(node_id)
}

fn write_metadata(path: &Path, node_id: NodeId) -> Result<(), IdentityError> {
    let metadata = IdentityMetadata {
        node_id: node_id.to_string(),
    };
    let contents = toml::to_string(&metadata)
        .map_err(|error| IdentityError::MetadataEncoding(error.to_string()))?;
    write_private_replace(path, contents.as_bytes())
}

fn write_private_new(path: &Path, contents: &[u8]) -> Result<(), IdentityError> {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

        options.mode(0o600);
        let mut file = options.open(path)?;
        file.write_all(contents)?;
        file.sync_all()?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }

    #[cfg(not(unix))]
    {
        let mut file = options.open(path)?;
        file.write_all(contents)?;
        file.sync_all()?;
    }

    Ok(())
}

fn write_private_replace(path: &Path, contents: &[u8]) -> Result<(), IdentityError> {
    let temporary = path.with_extension("toml.tmp");
    let _ = fs::remove_file(&temporary);
    write_private_new(&temporary, contents)?;

    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(IdentityError::Io(error))
        }
    }
}

fn create_private_dir(dir: &Path) -> Result<(), IdentityError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }

    #[cfg(not(unix))]
    fs::create_dir_all(dir)?;

    Ok(())
}

#[cfg(unix)]
fn check_key_permissions(path: &Path) -> Result<(), IdentityError> {
    use std::os::unix::fs::PermissionsExt;

    let mode = fs::metadata(path)?.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(IdentityError::InsecureKeyPermissions {
            path: path.to_path_buf(),
            mode,
        });
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_key_permissions(_path: &Path) -> Result<(), IdentityError> {
    Ok(())
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct IdentityMetadata {
    node_id: String,
}

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error("identity key {path} must be 32 bytes, found {size}")]
    InvalidKeyLength { path: PathBuf, size: usize },

    #[error("identity key {path} has insecure permissions {mode:o}; expected 0600")]
    InsecureKeyPermissions { path: PathBuf, mode: u32 },

    #[error("identity metadata is missing; delete identity.key to create a new node")]
    MissingMetadata,

    #[error("invalid identity metadata {path}: {source}")]
    InvalidMetadata {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("could not encode identity metadata: {0}")]
    MetadataEncoding(String),

    #[error("invalid node id: {0}")]
    InvalidNodeId(#[from] IdError),
}
