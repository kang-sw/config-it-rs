use std::borrow::Borrow;
use std::collections::HashMap;
use super::__all::*;
use std::sync::{Arc, Mutex, Weak};
use serde_json::map::Keys;

///
///
/// User should create registry to control over storages
///
/// # Usage
/// Config registry is somewhat similar with runtime.
///
/// - Uses mutex for safe internal state access
/// - Storage management is up to each owning classes
///
#[derive(Clone, Default)]
pub struct RegistryHandle {
    body: Arc<Mutex<RegistryBody>>,
}


#[derive(Default)]
struct RegistryBody {
    observers: Vec<Weak<dyn ObserveRegistry>>,
    storages: HashMap<String, Arc<Storage>>,
    cached_dump: Arc<JsonObject>,
}

impl RegistryHandle {
    /// Create new storage
    pub fn fork_storage(&self, key: String) -> Arc<Storage> {
        self.body.lock().unwrap().storages.entry(key)
            .or_insert_with_key(
                |key| {
                    Storage::create(
                        Box::from(self.clone()),
                        key,
                    )
                }
            ).clone()
    }

    /// Install new observer
    pub fn install_observer(&self, obsrv: Arc<dyn ObserveRegistry>) {
        self.body.lock().unwrap().observers.push(Arc::downgrade(&obsrv));
    }

    /// Visit storage entities within locked scope
    pub fn collect_storage(&self) -> Vec<Arc<Storage>> {
        let body = self.body.lock().unwrap();
        body.storages.iter().map(|(_, x)| x.clone()).collect()
    }

    /// Get cached json content
    pub fn get_cache(&self) -> Arc<JsonObject> { todo!() }

    /// Dump all contents.
    pub fn dump(&self, mode: DumpPolicy) -> Arc<JsonObject> { todo!() }

    // TODO: Load from json value
    pub fn load(&self, data: LoadPolicy) { todo!() }
}

#[derive(Default)]
pub enum DumpPolicy {
    /// Performs read-only behavior.
    #[default]
    DumpOnly,

    /// Merge dump result onto current cache.
    MergeCache,

    ///
    ResetCache,
}

pub enum LoadPolicy<'a> {
    LoadOnly(&'a JsonObject),
    ResetCache(Arc<JsonObject>),
    MergeCache(&'a JsonObject),
}

impl StorageOwner for RegistryHandle {}

pub trait ObserveRegistry {
    fn on_new_storage(&self);
    fn on_dropped_storage(&self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_registry() {
        let _rg = RegistryHandle::default();
    }
}
