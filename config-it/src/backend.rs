use std::error::Error;

use crate::{
    core::{BackendEvent, BackendReplicateEvent, ControlDirective},
    storage,
};

///
/// Backend interface for storage
///
pub struct StorageBackendChannel {
    pub(crate) tx: async_channel::Sender<ControlDirective>,
}

impl StorageBackendChannel {
    ///
    /// Creates new backend interface from storage
    ///
    pub fn new(storage: storage::Storage) -> Self {
        Self {
            tx: storage.tx.clone(),
        }
    }

    // Request new backend event receiver
    pub fn open_channel(
        &self,
    ) -> Result<async_channel::Sender<BackendReplicateEvent>, crate::core::Error> {
        todo!("Request new backend event receive channel")
    }

    ///
    /// Send backend event to storage driver.
    ///
    pub async fn send_event(&self, evt: BackendEvent) -> Result<(), impl Error> {
        self.tx.send(ControlDirective::Backend(evt)).await
    }
}
