use std::collections::hash_map::DefaultHasher;
use std::fs::{self, File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use super::io;
use crate::error::Result;

const PROCESS_LOCK_SHARDS: usize = 64;

static PROCESS_LOCKS: OnceLock<Box<[Mutex<()>]>> = OnceLock::new();

pub struct WriteLock {
    _file: File,
    _process: MutexGuard<'static, ()>,
}

pub fn acquire_write_lock(path: &Path) -> Result<WriteLock> {
    let process = process_lock(path)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let mut options = OpenOptions::new();
    options.create(true).truncate(false).read(true).write(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        options.mode(0o600);
    }

    let file = options.open(io::sidecar(path, ".lock"))?;

    file.lock()?;

    Ok(WriteLock {
        _file: file,
        _process: process,
    })
}

fn shards() -> &'static [Mutex<()>] {
    PROCESS_LOCKS.get_or_init(|| {
        (0..PROCESS_LOCK_SHARDS)
            .map(|_| Mutex::new(()))
            .collect::<Box<[_]>>()
    })
}

fn lock_key(path: &Path) -> PathBuf {
    let name = path.file_name().unwrap_or_default();
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    fs::canonicalize(parent).map_or_else(|_| path.to_path_buf(), |dir| dir.join(name))
}

fn process_lock(path: &Path) -> &'static Mutex<()> {
    let mut hasher = DefaultHasher::new();
    lock_key(path).hash(&mut hasher);
    &shards()[hasher.finish() as usize % PROCESS_LOCK_SHARDS]
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("ticonnect-lock-{}-{id}", std::process::id()))
    }

    #[test]
    fn lock_key_canonicalizes_parent() {
        let dir = temp_dir();
        fs::create_dir_all(dir.join("sub")).expect("create test dirs");
        let direct = dir.join("config.toml");
        let indirect = dir.join("sub").join("..").join("config.toml");

        assert_eq!(lock_key(&direct), lock_key(&indirect));
        assert!(std::ptr::eq(process_lock(&direct), process_lock(&indirect)));

        fs::remove_dir_all(dir).expect("remove test dirs");
    }

    #[test]
    fn poisoned_process_lock_is_recovered() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).expect("create test dir");
        let path = dir.join("config.toml");
        let poisoned = path.clone();

        let handle = std::thread::spawn(move || {
            let _guard = process_lock(&poisoned)
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            panic!("poison the process lock");
        });
        assert!(handle.join().is_err());

        let lock = acquire_write_lock(&path).expect("recover from poisoned lock");
        drop(lock);

        fs::remove_dir_all(dir).expect("remove test dir");
    }
}
