use std::error::Error;

use crate::{
    storage,
    storage_core::{BackendEvent, ControlDirective},
};

///
/// Backend interface for storage
///
pub struct StorageBackendIface {
    tx: async_channel::Sender<ControlDirective>,
}

impl StorageBackendIface {
    ///
    /// Creates new backend interface from storage
    ///
    pub fn new(storage: storage::Storage) -> Self {
        Self {
            tx: storage.tx.clone(),
        }
    }

    // TODO: Request new backend event receive channel

    ///
    /// Send backend event to storage driver.
    ///
    pub async fn send_event(
        &self,
        evt: BackendEvent,
    ) -> Result<(), impl Error> {
        self.tx.send(ControlDirective::Backend(evt)).await
    }
}
