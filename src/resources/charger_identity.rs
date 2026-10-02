//! Durable allocation of external OCPP charge-point identities.

use std::collections::{HashMap, HashSet};

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
    /// Stable keys are used for authored chargers. Dynamically placed chargers
    /// receive unkeyed IDs so removing and rebuilding one can never recycle it.
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
        if let Some(error) = &self.load_error {
            return Err(IdentityRegistryError::StorageUnavailable(error.clone()));
        }
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

    pub fn allocated_count(&self) -> usize {
        self.data.allocated.len()
    }

    #[cfg(test)]
    pub(crate) fn in_memory() -> Self {
        Self::load_with_storage(Box::new(MemoryStorage::default()))
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
struct PlatformIdentityStorage(std::path::PathBuf);

#[cfg(not(target_arch = "wasm32"))]
impl IdentityStorage for PlatformIdentityStorage {
    fn load(&self) -> Result<Option<RegistryData>, String> {
        match std::fs::read(&self.0) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|_| "identity registry could not be decoded".to_string()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err("identity registry could not be read".to_string()),
        }
    }

    fn save(&self, data: &RegistryData) -> Result<(), String> {
        let parent = self
            .0
            .parent()
            .ok_or_else(|| "identity registry path is invalid".to_string())?;
        std::fs::create_dir_all(parent)
            .map_err(|_| "identity registry directory could not be created".to_string())?;
        let bytes = serde_json::to_vec(data)
            .map_err(|_| "identity registry could not be encoded".to_string())?;
        let mut file = std::fs::File::create(&self.0)
            .map_err(|_| "identity registry could not be written".to_string())?;
        use std::io::Write;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| "identity registry could not be persisted".to_string())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn platform_storage() -> Box<dyn IdentityStorage> {
    use directories::ProjectDirs;

    let Some(project_dirs) = ProjectDirs::from("com", "juherr", "Kilowatt Tycoon") else {
        return Box::new(UnavailableStorage("application data directory unavailable"));
    };
    Box::new(PlatformIdentityStorage(
        project_dirs
            .data_local_dir()
            .join("charge-point-identities.json"),
    ))
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

struct UnavailableStorage(&'static str);

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
struct MemoryStorage(std::sync::Arc<std::sync::Mutex<Option<RegistryData>>>);

#[cfg(test)]
impl IdentityStorage for MemoryStorage {
    fn load(&self) -> Result<Option<RegistryData>, String> {
        Ok(self.0.lock().unwrap().clone())
    }

    fn save(&self, data: &RegistryData) -> Result<(), String> {
        *self.0.lock().unwrap() = Some(data.clone());
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
    fn corrupted_registry_disables_new_allocations() {
        let storage = MemoryStorage::default();
        *storage.0.lock().unwrap() = Some(RegistryData {
            version: REGISTRY_VERSION,
            next_id: 0,
            assignments: HashMap::new(),
            allocated: HashSet::new(),
        });
        let mut registry = ChargerIdentityRegistry::load_with_storage(Box::new(storage));
        assert!(registry.assign(None).is_err());
    }
}
