use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use thiserror::Error;
use ticonnect_core::attachment::{AttachmentHash, AttachmentRef, mime_from_name, validate_name};

const PARTIAL_DIR: &str = ".partials";

#[derive(Debug, Clone)]
pub struct AttachmentStore {
    root: PathBuf,
}

impl AttachmentStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let root = root.into();
        create_private_dir(&root)?;
        create_private_dir(&root.join(PARTIAL_DIR))?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path_for(&self, hash: AttachmentHash) -> PathBuf {
        let hex = hash.to_hex();
        self.root.join(&hex[..2]).join(hex)
    }

    pub fn partial_path(&self, hash: AttachmentHash) -> PathBuf {
        self.root.join(PARTIAL_DIR).join(format!("{hash}.part"))
    }

    pub fn contains(&self, hash: AttachmentHash) -> bool {
        self.path_for(hash).is_file()
    }

    pub fn put_bytes(&self, hash: AttachmentHash, bytes: &[u8]) -> Result<PathBuf, StoreError> {
        if AttachmentHash::of(bytes) != hash {
            return Err(StoreError::HashMismatch);
        }
        let destination = self.path_for(hash);
        if destination.is_file() {
            return Ok(destination);
        }
        let parent = destination.parent().ok_or(StoreError::InvalidPath)?;
        create_private_dir(parent)?;
        let temp = parent.join(format!(".{hash}.tmp"));
        remove_if_exists(&temp)?;
        {
            let mut file = private_file(&temp)?;
            file.write_all(bytes)?;
            file.sync_all()?;
        }
        fs::rename(&temp, &destination)?;
        set_file_private(&destination)?;
        Ok(destination)
    }

    pub fn put_file(&self, path: &Path) -> Result<AttachmentRef, StoreError> {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_file() {
            return Err(StoreError::NotAFile);
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(StoreError::InvalidName)?
            .to_owned();
        validate_name(&name).map_err(|_| StoreError::InvalidName)?;
        let mime = mime_from_name(&name).to_owned();

        let mut source = File::open(path)?;
        let mut hasher = blake3::Hasher::new();
        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            let read = source.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        let hash = AttachmentHash::from_bytes(*hasher.finalize().as_bytes());
        let destination = self.path_for(hash);
        if !destination.is_file() {
            source.seek(SeekFrom::Start(0))?;
            let parent = destination.parent().ok_or(StoreError::InvalidPath)?;
            create_private_dir(parent)?;
            let temp = parent.join(format!(".{hash}.tmp"));
            remove_if_exists(&temp)?;
            let mut target = private_file(&temp)?;
            let mut hasher = blake3::Hasher::new();
            loop {
                let read = source.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
                target.write_all(&buffer[..read])?;
            }
            target.sync_all()?;
            if AttachmentHash::from_bytes(*hasher.finalize().as_bytes()) != hash {
                let _ = remove_if_exists(&temp);
                return Err(StoreError::HashMismatch);
            }
            fs::rename(&temp, &destination)?;
            set_file_private(&destination)?;
        }
        let size = metadata.len();
        AttachmentRef::new(hash, name, size, mime)
            .map_err(|error| StoreError::Invalid(error.to_string()))
    }

    pub fn verify(&self, hash: AttachmentHash) -> Result<(), StoreError> {
        let path = self.path_for(hash);
        let mut file = File::open(path)?;
        let mut hasher = blake3::Hasher::new();
        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        if AttachmentHash::from_bytes(*hasher.finalize().as_bytes()) != hash {
            return Err(StoreError::HashMismatch);
        }
        Ok(())
    }

    pub fn read_chunk(
        &self,
        hash: AttachmentHash,
        offset: u64,
        length: usize,
    ) -> Result<Vec<u8>, StoreError> {
        let mut file = File::open(self.path_for(hash))?;
        file.seek(SeekFrom::Start(offset))?;
        let mut data = vec![0_u8; length];
        let mut read = 0;
        while read < data.len() {
            let count = file.read(&mut data[read..])?;
            if count == 0 {
                break;
            }
            read += count;
        }
        data.truncate(read);
        Ok(data)
    }

    pub fn size(&self, hash: AttachmentHash) -> Result<u64, StoreError> {
        Ok(fs::metadata(self.path_for(hash))?.len())
    }

    pub fn export(&self, hash: AttachmentHash, destination: &Path) -> Result<(), StoreError> {
        if destination.exists() {
            let metadata = fs::symlink_metadata(destination)?;
            if metadata.file_type().is_symlink() {
                return Err(StoreError::Symlink);
            }
            return Err(StoreError::DestinationExists);
        }
        let source = self.path_for(hash);
        let mut input = File::open(source)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        set_file_private(destination)?;
        std::io::copy(&mut input, &mut output)?;
        output.sync_all()?;
        Ok(())
    }

    pub fn publish_partial(&self, hash: AttachmentHash) -> Result<PathBuf, StoreError> {
        let partial = self.partial_path(hash);
        let mut file = File::open(&partial)?;
        let mut hasher = blake3::Hasher::new();
        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        if AttachmentHash::from_bytes(*hasher.finalize().as_bytes()) != hash {
            let _ = remove_if_exists(&partial);
            return Err(StoreError::HashMismatch);
        }
        let destination = self.path_for(hash);
        if let Some(parent) = destination.parent() {
            create_private_dir(parent)?;
        }
        fs::rename(&partial, &destination)?;
        set_file_private(&destination)?;
        Ok(destination)
    }

    pub fn cleanup_partial(&self, hash: AttachmentHash) -> Result<(), StoreError> {
        remove_if_exists(&self.partial_path(hash))
    }

    pub fn cleanup_orphans(&self) -> Result<usize, StoreError> {
        let partial = self.root.join(PARTIAL_DIR);
        let mut removed = 0;
        for entry in fs::read_dir(partial)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                fs::remove_file(entry.path())?;
                removed += 1;
            }
        }
        Ok(removed)
    }
}

fn create_private_dir(path: &Path) -> Result<(), StoreError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(not(unix))]
    fs::create_dir_all(path)?;
    Ok(())
}

fn private_file(path: &Path) -> Result<File, StoreError> {
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    set_file_private(path)?;
    Ok(file)
}

fn set_file_private(path: &Path) -> Result<(), StoreError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn remove_if_exists(path: &Path) -> Result<(), StoreError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("attachment I/O failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("attachment hash does not match its content")]
    HashMismatch,

    #[error("attachment path is invalid")]
    InvalidPath,

    #[error("attachment name is invalid")]
    InvalidName,

    #[error("attachment source is not a regular file")]
    NotAFile,

    #[error("attachment destination already exists")]
    DestinationExists,

    #[error("refusing to write through a symlink")]
    Symlink,

    #[error("attachment is invalid: {0}")]
    Invalid(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ticonnect-store-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        dir
    }

    #[test]
    fn stores_and_dedupes_content() {
        let dir = temp_dir();
        let store = AttachmentStore::new(dir.join("attachments")).expect("store");
        let hash = AttachmentHash::of(b"hello");
        let first = store.put_bytes(hash, b"hello").expect("put");
        let second = store.put_bytes(hash, b"hello").expect("put again");
        assert_eq!(first, second);
        assert!(store.contains(hash));
        assert_eq!(store.read_chunk(hash, 1, 3).expect("read"), b"ell");
        store.verify(hash).expect("verify");
    }

    #[test]
    fn rejects_hash_mismatch() {
        let dir = temp_dir();
        let store = AttachmentStore::new(dir.join("attachments")).expect("store");
        assert!(matches!(
            store.put_bytes(AttachmentHash::of(b"a"), b"b"),
            Err(StoreError::HashMismatch)
        ));
    }

    #[test]
    fn ingests_files_and_exports_without_overwrite() {
        let dir = temp_dir();
        let store = AttachmentStore::new(dir.join("attachments")).expect("store");
        let source = dir.join("photo.png");
        std::fs::write(&source, b"image").expect("write");
        let reference = store.put_file(&source).expect("put file");
        assert_eq!(reference.name, "photo.png");
        assert_eq!(reference.mime, "image/png");

        let output = dir.join("saved.png");
        store.export(reference.hash, &output).expect("export");
        assert_eq!(std::fs::read(&output).expect("read"), b"image");
        assert!(matches!(
            store.export(reference.hash, &output),
            Err(StoreError::DestinationExists)
        ));
    }
}
