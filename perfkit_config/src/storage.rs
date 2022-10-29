use std::ops::DerefMut;
use std::sync::{Arc, Mutex, Weak};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use super::__all::*;

///
///
/// User has basic control over config category
///
pub struct Storage {
    w_self: Weak<Storage>,
    owner: Mutex<Option<Box<dyn StorageOwner>>>,
    name: Arc<str>,
    update_fence: AtomicUsize,

    update_context: Mutex<StorageUpdateContext>,
    pending_context: Mutex<StoragePendingContext>,
}

#[derive(Default)]
struct StorageUpdateContext {}

#[derive(Default)]
struct StoragePendingContext {}

impl Storage {
    pub(super) fn create(owner: Box<dyn StorageOwner>, name: &str) -> Arc<Storage> {
        Arc::new_cyclic(
            move |w| {
                Storage {
                    w_self: w.clone(),
                    owner: Mutex::new(Some(owner)),
                    name: Arc::from(name),
                    update_fence: AtomicUsize::new(0),
                    update_context: Mutex::default(),
                    pending_context: Mutex::default(),
                }
            }
        )
    }

    pub fn update(&self) -> bool {
        todo!()

        // Acquire mutex. Update operation must be protected.
    }

    pub fn merge_entities(&self, entities: impl ConfigSetBehavior, prefix: &[&str]) {
        todo!()
    }

    pub fn fence(&self) -> usize {
        self.update_fence.load(Ordering::Relaxed)
    }

    pub fn try_unregister(&self) -> bool {
        if let Some(monitor) = self.owner.lock().unwrap().deref_mut() {
            // Consumes monitor to prevent next access to this
            monitor.notify_dispose(&self.name);
            true
        } else {
            false
        }
    }

    pub fn observe(&self, inst: Weak<dyn ObserveStorage>) {
        todo!()
    }
}

impl Drop for Storage {
    fn drop(&mut self) {
        self.try_unregister();
    }
}

pub(super) trait StorageOwner {
    fn notify_dispose(&self, _name: &str) {}
}

pub trait ObserveStorage {
    /// Following methods only
    fn on_update_begin(&self, _: &Arc<Storage>) {}
    fn on_new_entity(&self, _: &Arc<EntityBase>) {}
    fn on_delete_entity(&self, _: u64) {}
    fn on_update_entity(&self, _: &Arc<EntityBase>, _new_value: &Arc<dyn EntityValue>) {}
    fn on_update_end(&self) {}
}

///
///
/// Represents required behavior of config set.
///
pub trait ConfigSetBehavior: Default {
    fn collect_entities(&self) -> Vec<Arc<EntityBase>>;
}
