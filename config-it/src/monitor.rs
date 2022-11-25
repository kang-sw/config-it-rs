use std::error::Error;

use crate::{
    core::{ControlDirective, MonitorEvent, MonitorReplication},
    storage,
};

///
/// monitor interface for storage
///
pub struct StorageMonitorChannel {
    pub(crate) tx: async_channel::Sender<ControlDirective>,
}

impl StorageMonitorChannel {
    ///
    /// Creates new monitor interface from storage
    ///
    pub fn new(storage: storage::Storage) -> Self {
        Self {
            tx: storage.tx.clone(),
        }
    }

    // Request new monitor event receiver
    pub fn open_channel(
        &self,
    ) -> Result<async_channel::Sender<MonitorReplication>, crate::core::Error> {
        todo!("Request new monitor event receive channel")
    }

    ///
    /// Send monitor event to storage driver.
    ///
    pub async fn send_event(&self, evt: MonitorEvent) -> Result<(), impl Error> {
        self.tx.send(ControlDirective::FromMonitor(evt)).await
    }
}
