use std::error::Error;

use crate::{
    core::{ControlDirective, MonitorEvent, ReplicationEvent},
    storage,
};

type ReplicationChannel = async_channel::Receiver<ReplicationEvent>;

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
    pub async fn open_replication_channel(&self) -> Result<ReplicationChannel, crate::core::Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ControlDirective::MonitorRegister { reply_to: tx })
            .await
            .map_err(|_| crate::core::Error::ExpiredStorage)?;

        rx.await.map_err(|_| crate::core::Error::ExpiredStorage)
    }

    ///
    /// Send monitor event to storage driver.
    ///
    pub async fn send_event(&self, evt: MonitorEvent) -> Result<(), impl Error> {
        self.tx.send(ControlDirective::FromMonitor(evt)).await
    }
}
