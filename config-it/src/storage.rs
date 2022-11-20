use std::{
    future::Future,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
};

use crate::{
    config::{self, SetCoreContext},
    entity::{self, EntityEventHook},
    storage_core::ControlDirective,
};
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
    /// Creates and register new config set.
    ///
    /// If path is duplicated for existing config set, the program will panic.
    ///
    pub fn create_set<T: config::CollectPropMeta>(
        &self,
        path: Vec<CompactString>,
    ) -> config::Set<T> {
        assert!(!path.is_empty(), "First argument that will be used as category, must exist!");
        assert!(path.iter().all(|x| !x.is_empty()), "Empty path argument is not allowed!");

        static ID_GEN: AtomicU64 = AtomicU64::new(0);
        let register_id = 1 + ID_GEN.fetch_add(1, Ordering::Relaxed);
        let path = Arc::new(path);

        // Collect metadata
        let mut table: Vec<_> = T::prop_desc_table__().values().collect();
        table.sort_by(|a, b| a.index.cmp(&b.index));

        let entity_hook = Arc::new(EntityHookImpl {
            register_id,
            tx: self.tx.clone(),
        });

        let sources: Vec<_> = table
            .into_iter()
            .map(|prop| entity::EntityData::new(prop.meta.clone(), entity_hook.clone()))
            .collect();

        // Create core config set context with reflected target metadata set
        let (broad_tx, broad_rx) = async_broadcast::broadcast::<()>(1);
        let core = Arc::new(SetCoreContext {
            register_id,
            sources: Arc::new(sources),
            source_update_fence: AtomicUsize::new(0),
            update_receiver_channel: async_mutex::Mutex::new(broad_rx),
            path: path.clone(),
        });

        // TODO: Send 'OnRegisterConfigSet' to worker.

        crate::Set::<T>::create_with__(
            core,
            Arc::new(SetUnregisterHook {
                register_id,
                tx: self.tx.clone(),
            }),
        )
    }

    // TODO: Dump all contents I/O from/to Serializer/Deserializer
    // NOTE:
}

struct SetUnregisterHook {
    register_id: u64,
    tx: async_channel::Sender<ControlDirective>,
}

impl Drop for SetUnregisterHook {
    fn drop(&mut self) {
        todo!("Send 'SetUnregister' event to tx")
    }
}

struct EntityHookImpl {
    register_id: u64,
    tx: async_channel::Sender<ControlDirective>,
}

impl EntityEventHook for EntityHookImpl {
    fn on_committed(&self, data: &entity::EntityData) {
        todo!("Send 'EntityNotifyCommit' event to tx")
    }

    fn on_value_changed(&self, data: &entity::EntityData) {
        todo!("Send 'EntityValueUpdate' event to tx")
    }
}

///
/// Drives storage internal events.
///
/// - Receives update request
///
struct StorageDriveContext {}

impl StorageDriveContext {
    fn new() -> Self {
        todo!()
    }

    async fn handle_once(&mut self, msg: ControlDirective) {
        match msg {
            _ => todo!(),
        }
    }
}
