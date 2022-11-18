use std::future::Future;

use crate::config;
use log::debug;
use smol_str::SmolStr;

///
/// Storage manages multiple sets registered by preset key.
///
#[derive(Clone)]
pub struct Storage {
    /// Internally, any request/notify is transferred through this channel.
    ///
    /// Thus, any reply-required operation becomes async inherently.
    tx: async_channel::Sender<StorageControl>,
}

impl Storage {
    ///
    /// Creates new storage and its driver.
    ///
    /// The second tuple parameter is asynchronous loop which handles all storage events,
    ///  which must be spawned or blocked by runtime to make storage work. All storage
    ///
    pub fn create() -> (Self, impl Future<Output = ()>) {
        let (tx, rx) = async_channel::unbounded();
        let driver = {
            async move {
                let mut context = StorageDriveContext::new();
                loop {
                    match rx.recv().await {
                        Ok(msg) => {
                            context.handle_once(msg);
                        }
                        Err(e) => {
                            let ptr = &rx as *const _;
                            debug!("[{ptr:p}] ({e:?}) All sender channel has been closed. Closing storage ...")
                        }
                    }
                }
            }
        };

        (Self { tx }, driver)
    }

    ///
    /// Creates new config set from storage.
    ///
    /// # Parameters
    ///
    /// - `category`:
    ///
    pub fn create_set<T: config::CollectPropMeta>(
        &self,
        category: Vec<SmolStr>,
    ) -> config::Set<T> {
        // TODO: Create core config set context with reflected target metadata set

        // TODO: Create unregister hook

        todo!()
    }

    // TODO: Dump all contents I/O from/to Serializer/Deserializer

    // TODO: Install change notification hook
}

/// Message type to drive storage
enum StorageControl {}

///
///
///
struct StorageDriveContext {}

impl StorageDriveContext {
    fn new() -> Self {
        todo!()
    }

    fn handle_once(
        &mut self,
        msg: StorageControl,
    ) {
    }
}
