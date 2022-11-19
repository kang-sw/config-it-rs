use std::future::Future;

use crate::{config, storage_core::ControlDirective};
use log::debug;
use smartstring::alias::CompactString;

///
/// Storage manages multiple sets registered by preset key.
///
#[derive(Clone)]
pub struct Storage {
    /// Internally, any request/notify is transferred through this channel.
    ///
    /// Thus, any reply-required operation becomes async inherently.
    pub(crate) tx: async_channel::Sender<ControlDirective>,
}

impl Storage {
    ///
    /// Creates new storage and its driver.
    ///
    /// The second tuple parameter is asynchronous loop which handles all storage events,
    ///  which must be spawned or blocked by runtime to make storage work. All storage
    ///
    pub fn new() -> (Self, impl Future<Output = ()>) {
        let (tx, rx) = async_channel::unbounded();
        let driver = {
            async move {
                let mut context = StorageDriveContext::new();
                loop {
                    match rx.recv().await {
                        Ok(msg) => {
                            context.handle_once(msg).await;
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
    /// - `category`
    ///
    pub fn create_set<T: config::CollectPropMeta>(
        &self,
        category: CompactString,
        path: Vec<CompactString>,
    ) -> config::Set<T> {
        // TODO: Create core config set context with reflected target metadata set

        // TODO: Create unregister hook
        todo!()
    }

    // TODO: Dump all contents I/O from/to Serializer/Deserializer

    // TODO: Install change notification hook
}

///
/// Drives storage internal events.
///
/// - Receives update request}
///
struct StorageDriveContext {}

impl StorageDriveContext {
    fn new() -> Self {
        todo!()
    }

    async fn handle_once(
        &mut self,
        msg: ControlDirective,
    ) {
        match msg {
            _ => todo!(),
        }
    }
}
