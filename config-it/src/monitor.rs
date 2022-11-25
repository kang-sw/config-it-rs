use std::error::Error;

use crate::{
    core::{ControlDirective, MonitorEvent, ReplicationEvent},
    storage,
};

type ReplicationChannel = async_channel::Sender<ReplicationEvent>;

///
/// monitor interface for storage
///
pub struct StorageMonitor {
    pub(crate) tx: async_channel::Sender<ControlDirective>,
}

impl StorageMonitor {
    ///
    /// Creates new monitor interface from storage
    ///
    pub fn new(storage: storage::Storage) -> Self {
        Self {
            tx: storage.tx.clone(),
        }
    }

    // Request new monitor event receiver
    pub fn open_channel(&self) -> Result<ReplicationChannel, crate::core::Error> {
        todo!("Request new monitor event receive channel")
    }

    ///
    /// Send monitor event to storage driver.
    ///
    pub async fn send_event(&self, evt: MonitorEvent) -> Result<(), impl Error> {
        self.tx.send(ControlDirective::FromMonitor(evt)).await
    }
}
