//! Durable allocation of external OCPP charge-point identities.

use std::collections::{HashMap, HashSet};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicU64, Ordering};

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

const REGISTRY_VERSION: u32 = 1;
#[cfg(target_arch = "wasm32")]
const STORAGE_KEY: &str = "kilowatt-tycoon.charge-point-identities.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityRegistryError {
    StorageUnavailable(String),
    CorruptRegistry,
    Exhausted,
}

impl std::fmt::Display for IdentityRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StorageUnavailable(message) => {
                write!(f, "identity storage unavailable: {message}")
            }
            Self::CorruptRegistry => write!(f, "identity registry is invalid"),
            Self::Exhausted => write!(f, "charge-point identity space exhausted"),
        }
    }
}

impl std::error::Error for IdentityRegistryError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegistryData {
    version: u32,
    next_id: u64,
    /// Stable keys are used for authored chargers and persistent grid instance IDs.
    assignments: HashMap<String, String>,
    /// Kept for the lifetime of the registry; retired IDs remain reserved.
    allocated: HashSet<String>,
}

impl Default for RegistryData {
    fn default() -> Self {
        Self {
            version: REGISTRY_VERSION,
            next_id: 1,
            assignments: HashMap::new(),
            allocated: HashSet::new(),
        }
    }
}

trait IdentityStorage: Send + Sync {
    fn load(&self) -> Result<Option<RegistryData>, String>;
    fn save(&self, data: &RegistryData) -> Result<(), String>;
}

#[derive(Resource)]
pub struct ChargerIdentityRegistry {
    data: RegistryData,
    storage: Box<dyn IdentityStorage>,
    load_error: Option<String>,
}

impl ChargerIdentityRegistry {
    pub fn load_default() -> Self {
        let storage = platform_storage();
        Self::load_with_storage(storage)
    }

    fn load_with_storage(storage: Box<dyn IdentityStorage>) -> Self {
        match storage.load() {
            Ok(Some(data)) if data_is_valid(&data) => Self {
                data,
                storage,
                load_error: None,
            },
            Ok(None) => Self {
                data: RegistryData::default(),
                storage,
                load_error: None,
            },
            Ok(Some(_)) => Self {
                data: RegistryData::default(),
                storage,
                load_error: Some("identity registry is invalid".to_string()),
            },
            Err(error) => Self {
                data: RegistryData::default(),
                storage,
                load_error: Some(error),
            },
        }
    }

    /// Return the stable ID associated with `stable_key`, or durably allocate
    /// a fresh ID. Passing `None` always allocates a fresh, never-reused ID.
    pub fn assign(&mut self, stable_key: Option<&str>) -> Result<String, IdentityRegistryError> {
        self.retry_load_if_needed()?;
        if let Some(id) = stable_key.and_then(|key| self.data.assignments.get(key)) {
            return Ok(id.clone());
        }

        let mut next = self.data.clone();
        let id = loop {
            let candidate = format!("KT-{:08}", next.next_id);
            next.next_id = next
                .next_id
                .checked_add(1)
                .ok_or(IdentityRegistryError::Exhausted)?;
            if !next.allocated.contains(&candidate) {
                break candidate;
            }
        };
        next.allocated.insert(id.clone());
        if let Some(key) = stable_key {
            next.assignments.insert(key.to_string(), id.clone());
        }

        self.storage
            .save(&next)
            .map_err(IdentityRegistryError::StorageUnavailable)?;
        self.data = next;
        Ok(id)
    }

    fn retry_load_if_needed(&mut self) -> Result<(), IdentityRegistryError> {
        if self.load_error.is_none() {
            return Ok(());
        }

        match self.storage.load() {
            Ok(Some(data)) if data_is_valid(&data) => {
                self.data = data;
                self.load_error = None;
                Ok(())
            }
            Ok(None) => {
                self.data = RegistryData::default();
                self.load_error = None;
                Ok(())
            }
            Ok(Some(_)) => {
                self.load_error = Some("identity registry is invalid".to_string());
                Err(IdentityRegistryError::CorruptRegistry)
            }
            Err(error) => {
                self.load_error = Some(error.clone());
                Err(IdentityRegistryError::StorageUnavailable(error))
            }
        }
    }

    pub fn allocated_count(&self) -> usize {
        self.data.allocated.len()
    }

    #[cfg(test)]
    pub(crate) fn in_memory() -> Self {
        Self::load_with_storage(Box::new(MemoryStorage::default()))
    }

    #[cfg(test)]
    pub(crate) fn in_memory_with_one_failed_save() -> Self {
        let storage = MemoryStorage::default();
        storage
            .fail_next_save
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Self::load_with_storage(Box::new(storage))
    }

    #[cfg(test)]
    pub(crate) fn in_memory_with_persistent_save_failure(
        saves: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) -> Self {
        struct AlwaysFailingStorage(std::sync::Arc<std::sync::atomic::AtomicUsize>);
        impl IdentityStorage for AlwaysFailingStorage {
            fn load(&self) -> Result<Option<RegistryData>, String> {
                Ok(None)
            }

            fn save(&self, _data: &RegistryData) -> Result<(), String> {
                self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Err("persistent simulated write failure".to_string())
            }
        }
        Self::load_with_storage(Box::new(AlwaysFailingStorage(saves)))
    }
}

fn data_is_valid(data: &RegistryData) -> bool {
    if data.version != REGISTRY_VERSION || data.next_id == 0 {
        return false;
    }
    data.assignments
        .values()
        .all(|id| data.allocated.contains(id))
}

#[cfg(not(target_arch = "wasm32"))]
struct PlatformIdentityStorage {
    path: std::path::PathBuf,
    // Held for the lifetime of the registry to enforce a single writer for this
    // local allocation history, including across game processes.
    _writer_lock: std::fs::File,
}

#[cfg(not(target_arch = "wasm32"))]
impl IdentityStorage for PlatformIdentityStorage {
    fn load(&self) -> Result<Option<RegistryData>, String> {
        match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|_| "identity registry could not be decoded".to_string()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err("identity registry could not be read".to_string()),
        }
    }

    fn save(&self, data: &RegistryData) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "identity registry path is invalid".to_string())?;
        std::fs::create_dir_all(parent)
            .map_err(|_| "identity registry directory could not be created".to_string())?;
        let bytes = serde_json::to_vec(data)
            .map_err(|_| "identity registry could not be encoded".to_string())?;
        atomic_replace(&self.path, &bytes)
    }
}

#[cfg(not(target_arch = "wasm32"))]
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(not(target_arch = "wasm32"))]
fn atomic_replace(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    atomic_replace_with(path, bytes, |from, to| std::fs::rename(from, to))
}

#[cfg(not(target_arch = "wasm32"))]
fn atomic_replace_with(
    path: &std::path::Path,
    bytes: &[u8],
    replace: impl FnOnce(&std::path::Path, &std::path::Path) -> std::io::Result<()>,
) -> Result<(), String> {
    use std::io::Write;

    let parent = path
        .parent()
        .ok_or_else(|| "identity registry path is invalid".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|_| "identity registry directory could not be created".to_string())?;
    let filename = path
        .file_name()
        .ok_or_else(|| "identity registry path is invalid".to_string())?
        .to_string_lossy();
    let temp_path = loop {
        let suffix = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(".{filename}.{}.{}.tmp", std::process::id(), suffix));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                let result = file.write_all(bytes).and_then(|_| file.sync_all());
                if result.is_err() {
                    let _ = std::fs::remove_file(&candidate);
                    return Err("identity registry could not be persisted".to_string());
                }
                break candidate;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err("identity registry temporary file could not be created".into()),
        }
    };

    let result = replace(&temp_path, path).and_then(|_| sync_parent_directory(parent));
    if result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
        return Err("identity registry could not be persisted".to_string());
    }
    Ok(())
}

#[cfg(unix)]
fn sync_parent_directory(parent: &std::path::Path) -> std::io::Result<()> {
    std::fs::File::open(parent)?.sync_all()
}

// Windows does not support opening a directory as a regular File for fsync.
// The replacement file itself was synced before the rename.
#[cfg(not(unix))]
fn sync_parent_directory(_parent: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn platform_storage() -> Box<dyn IdentityStorage> {
    use directories::ProjectDirs;

    let Some(project_dirs) = ProjectDirs::from("com", "juherr", "Kilowatt Tycoon") else {
        return Box::new(UnavailableStorage("application data directory unavailable"));
    };
    let path = project_dirs
        .data_local_dir()
        .join("charge-point-identities.json");
    match PlatformIdentityStorage::open(path) {
        Ok(storage) => Box::new(storage),
        Err(_) => Box::new(UnavailableStorage(
            "identity registry is already in use by another game process",
        )),
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl PlatformIdentityStorage {
    fn open(path: std::path::PathBuf) -> Result<Self, String> {
        use fs2::FileExt;

        let parent = path
            .parent()
            .ok_or_else(|| "identity registry path is invalid".to_string())?;
        std::fs::create_dir_all(parent)
            .map_err(|_| "identity registry directory could not be created".to_string())?;
        let lock_path = path.with_extension("json.lock");
        let lock_file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
            .map_err(|_| "identity registry writer lock could not be opened".to_string())?;
        lock_file.try_lock_exclusive().map_err(|_| {
            "identity registry is already in use by another game process".to_string()
        })?;
        Ok(Self {
            path,
            _writer_lock: lock_file,
        })
    }
}

#[cfg(target_arch = "wasm32")]
struct PlatformIdentityStorage;

#[cfg(target_arch = "wasm32")]
impl IdentityStorage for PlatformIdentityStorage {
    fn load(&self) -> Result<Option<RegistryData>, String> {
        let storage = web_sys::window()
            .ok_or_else(|| "browser window unavailable".to_string())?
            .local_storage()
            .map_err(|_| "browser identity storage unavailable".to_string())?
            .ok_or_else(|| "browser identity storage unavailable".to_string())?;
        storage
            .get_item(STORAGE_KEY)
            .map_err(|_| "browser identity registry could not be read".to_string())?
            .map(|json| {
                serde_json::from_str(&json)
                    .map_err(|_| "identity registry could not be decoded".to_string())
            })
            .transpose()
    }

    fn save(&self, data: &RegistryData) -> Result<(), String> {
        let storage = web_sys::window()
            .ok_or_else(|| "browser window unavailable".to_string())?
            .local_storage()
            .map_err(|_| "browser identity storage unavailable".to_string())?
            .ok_or_else(|| "browser identity storage unavailable".to_string())?;
        let json = serde_json::to_string(data)
            .map_err(|_| "identity registry could not be encoded".to_string())?;
        storage
            .set_item(STORAGE_KEY, &json)
            .map_err(|_| "browser identity registry could not be persisted".to_string())
    }
}

#[cfg(target_arch = "wasm32")]
fn platform_storage() -> Box<dyn IdentityStorage> {
    Box::new(PlatformIdentityStorage)
}

#[cfg(not(target_arch = "wasm32"))]
struct UnavailableStorage(&'static str);

#[cfg(not(target_arch = "wasm32"))]
impl IdentityStorage for UnavailableStorage {
    fn load(&self) -> Result<Option<RegistryData>, String> {
        Err(self.0.to_string())
    }

    fn save(&self, _data: &RegistryData) -> Result<(), String> {
        Err(self.0.to_string())
    }
}

#[cfg(test)]
#[derive(Clone, Default)]
struct MemoryStorage {
    data: std::sync::Arc<std::sync::Mutex<Option<RegistryData>>>,
    fail_next_save: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(test)]
impl IdentityStorage for MemoryStorage {
    fn load(&self) -> Result<Option<RegistryData>, String> {
        Ok(self.data.lock().unwrap().clone())
    }

    fn save(&self, data: &RegistryData) -> Result<(), String> {
        if self
            .fail_next_save
            .swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            return Err("simulated temporary write failure".to_string());
        }
        *self.data.lock().unwrap() = Some(data.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingStorage;

    impl IdentityStorage for FailingStorage {
        fn load(&self) -> Result<Option<RegistryData>, String> {
            Ok(None)
        }

        fn save(&self, _data: &RegistryData) -> Result<(), String> {
            Err("write failed".to_string())
        }
    }

    #[test]
    fn restores_authored_identity_and_never_reuses_retired_allocations() {
        let storage = MemoryStorage::default();
        let mut registry = ChargerIdentityRegistry::load_with_storage(Box::new(storage.clone()));
        let first = registry.assign(Some("authored:site-1:chg_01")).unwrap();
        let retired = registry.assign(None).unwrap();

        let mut restored = ChargerIdentityRegistry::load_with_storage(Box::new(storage));
        assert_eq!(
            restored.assign(Some("authored:site-1:chg_01")).unwrap(),
            first
        );
        assert_eq!(restored.assign(None).unwrap(), "KT-00000003");
        assert_ne!(retired, "KT-00000003");
    }

    #[test]
    fn failed_persistence_does_not_allocate_an_identity() {
        let mut registry = ChargerIdentityRegistry::load_with_storage(Box::new(FailingStorage));
        assert!(matches!(
            registry.assign(None),
            Err(IdentityRegistryError::StorageUnavailable(_))
        ));
        assert_eq!(registry.allocated_count(), 0);
    }

    #[test]
    fn retries_a_cached_load_failure_after_storage_recovers() {
        use std::sync::atomic::{AtomicBool, Ordering};

        struct RecoverableStorage {
            available: std::sync::Arc<AtomicBool>,
            data: MemoryStorage,
        }

        impl IdentityStorage for RecoverableStorage {
            fn load(&self) -> Result<Option<RegistryData>, String> {
                if self.available.load(Ordering::Relaxed) {
                    self.data.load()
                } else {
                    Err("temporarily unavailable".to_string())
                }
            }

            fn save(&self, data: &RegistryData) -> Result<(), String> {
                self.data.save(data)
            }
        }

        let available = std::sync::Arc::new(AtomicBool::new(false));
        let storage = RecoverableStorage {
            available: available.clone(),
            data: MemoryStorage::default(),
        };
        let mut registry = ChargerIdentityRegistry::load_with_storage(Box::new(storage));
        assert!(registry.assign(None).is_err());

        available.store(true, Ordering::Relaxed);
        assert_eq!(registry.assign(None).unwrap(), "KT-00000001");
    }

    #[test]
    fn corrupted_registry_disables_new_allocations() {
        let storage = MemoryStorage::default();
        *storage.data.lock().unwrap() = Some(RegistryData {
            version: REGISTRY_VERSION,
            next_id: 0,
            assignments: HashMap::new(),
            allocated: HashSet::new(),
        });
        let mut registry = ChargerIdentityRegistry::load_with_storage(Box::new(storage));
        assert!(registry.assign(None).is_err());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn failed_atomic_replacement_preserves_the_last_valid_registry() {
        let path = std::env::temp_dir().join(format!(
            "kilowatt-tycoon-identity-test-{}-{}.json",
            std::process::id(),
            TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let previous = RegistryData {
            version: REGISTRY_VERSION,
            next_id: 2,
            assignments: HashMap::from([(
                "authored:site:charger".to_string(),
                "KT-00000001".to_string(),
            )]),
            allocated: HashSet::from(["KT-00000001".to_string()]),
        };
        std::fs::write(&path, serde_json::to_vec(&previous).unwrap()).unwrap();

        let result = atomic_replace_with(&path, b"new registry", |_, _| {
            Err(std::io::Error::other("simulated replacement failure"))
        });

        assert!(result.is_err());
        let restored: RegistryData =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(restored.assignments, previous.assignments);
        assert_eq!(restored.allocated, previous.allocated);
        std::fs::remove_file(path).unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn only_one_registry_writer_can_hold_the_same_installation_registry() {
        let path = std::env::temp_dir().join(format!(
            "kilowatt-tycoon-identity-lock-test-{}-{}.json",
            std::process::id(),
            TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let first = PlatformIdentityStorage::open(path.clone()).unwrap();
        assert!(PlatformIdentityStorage::open(path.clone()).is_err());
        drop(first);
        let second = PlatformIdentityStorage::open(path.clone()).unwrap();
        drop(second);
        std::fs::remove_file(path.with_extension("json.lock")).unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn platform_registry_replaces_existing_file_and_restores_allocations() {
        let path = std::env::temp_dir().join(format!(
            "kilowatt-tycoon-identity-persistence-test-{}-{}.json",
            std::process::id(),
            TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let first_id;
        let second_id;
        {
            let storage = PlatformIdentityStorage::open(path.clone()).unwrap();
            let mut registry = ChargerIdentityRegistry::load_with_storage(Box::new(storage));
            first_id = registry.assign(Some("charger:first")).unwrap();
            second_id = registry.assign(Some("charger:second")).unwrap();
            assert_ne!(first_id, second_id);
        }

        {
            let storage = PlatformIdentityStorage::open(path.clone()).unwrap();
            let mut registry = ChargerIdentityRegistry::load_with_storage(Box::new(storage));
            assert_eq!(registry.assign(Some("charger:first")).unwrap(), first_id);
            assert_eq!(registry.assign(Some("charger:second")).unwrap(), second_id);
            assert_eq!(registry.allocated_count(), 2);
        }

        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(path.with_extension("json.lock")).unwrap();
    }
}
