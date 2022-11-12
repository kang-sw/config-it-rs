use std::sync::Arc;
use crate::entity::EntityBase;
use crate::registry::Registry;

///
///
/// Stores multiple storage instance. A proxy to storage body class.
///
#[derive(Clone)]
pub struct Storage {
    body: Arc<StorageBody>,
}

struct StorageBody {}

impl Storage {
    ///
    /// Creates new storage instance.
    ///
    /// Returns existing storage instance
    ///
    pub fn new(rg: Registry, category: String) -> Self {
        // TODO
        Self {
            body: Arc::new(StorageBody {})
        }
    }

    ///
    /// Gets current update fence value
    ///
    pub(crate) fn update_fence(&self) -> usize { todo!() }

    ///
    /// Registers set of entities
    ///
    /// Returns offset id, that each entity will be registered
    ///  as id `[retval ... retval + entities.size()]`
    ///
    pub(crate) fn register_entities(&self, entities: &[Arc<EntityBase>]) -> usize {
        todo!()
    }

    ///
    /// Creates event receiver
    ///
    #[cfg(feature = "tokio")]
    pub fn subscribe_events(&self) -> broadcast::Receiver<StorageEvent> {
        todo!()
    }

    // TODO: Dump to serializer
    // TODO: Load from deserializer
}

#[cfg(feature = "tokio")]
use tokio::sync::broadcast;

pub enum StorageEvent {
    ///
    /// Remote backend send update to this storage.
    ///
    /// - 0: Remote backend identifier
    /// - 1: Updated target's registration IDs (sorted)
    ///
    RemoteUpdate(Arc<str>, Arc<[usize]>),

    ///
    /// Imported from any deserializer
    ///
    Import,

    ///
    /// Exported to any serializer
    ///
    Export,
}
