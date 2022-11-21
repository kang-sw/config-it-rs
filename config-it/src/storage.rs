use std::{
    future::Future,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
};

use crate::{
    backend::StorageBackendChannel,
    config::{self, SetContext},
    core::{self, ControlDirective, Error as ConfigError},
    entity::{self, EntityEventHook},
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
                let mut context = detail::StorageDriveContext::new();
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
    pub async fn create_set<T: config::CollectPropMeta>(
        &self,
        path: Vec<CompactString>,
    ) -> Result<config::Set<T>, ConfigError> {
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
        let core = Arc::new(SetContext {
            register_id,
            sources: Arc::new(sources),
            source_update_fence: AtomicUsize::new(0),
            update_receiver_channel: async_mutex::Mutex::new(broad_rx),
            path: path.clone(),
        });

        let (tx, rx) = oneshot::channel();
        match self
            .tx
            .send(ControlDirective::OnRegisterConfigSet(
                core::ConfigSetRegisterDesc {
                    register_id,
                    context: core.clone(),
                    reply_success: tx,
                    event_broadcast: broad_tx,
                }
                .into(),
            ))
            .await
        {
            Ok(_) => {}
            Err(_) => return Err(ConfigError::ExpiredStorage),
        };

        match rx.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(ConfigError::ExpiredStorage),
        };

        let set = crate::Set::<T>::create_with__(
            core,
            Arc::new(SetUnregisterHook {
                register_id,
                tx: self.tx.clone(),
            }),
        );

        Ok(set)
    }

    // TODO: Dump all contents I/O from/to Serializer/Deserializer

    pub fn create_backend(&self) -> StorageBackendChannel {
        StorageBackendChannel {
            tx: self.tx.clone(),
        }
    }
}

struct SetUnregisterHook {
    register_id: u64,
    tx: async_channel::Sender<ControlDirective>,
}

impl Drop for SetUnregisterHook {
    fn drop(&mut self) {
        // Just ignore result. If channel was closed before the set is unregistered,
        //  it's ok to ignore this operation silently.
        let _ = self
            .tx
            .try_send(ControlDirective::OnUnregisterConfigSet(self.register_id));
    }
}

struct EntityHookImpl {
    register_id: u64,
    tx: async_channel::Sender<ControlDirective>,
}

impl EntityEventHook for EntityHookImpl {
    fn on_committed(&self, data: &entity::EntityData) {
        // Update notification is transient, thus when storage driver is busy, it can
        //  just be dropped.
        let _ = self.tx.try_send(ControlDirective::EntityNotifyCommit {
            register_id: self.register_id,
            item_id: data.get_id(),
        });
    }

    fn on_value_changed(&self, data: &entity::EntityData) {
        // Update notification is transient, thus when storage driver is busy, it can
        //  just be dropped.
        let _ = self.tx.try_send(ControlDirective::EntityValueUpdate {
            register_id: self.register_id,
            item_id: data.get_id(),
        });
    }
}

mod detail {
    use crate::core::{BackendEvent, BackendReplicateEvent, ConfigSetRegisterDesc};

    use super::*;
    use std::collections::HashMap;

    ///
    /// Drives storage internal events.
    ///
    /// - Receives update request
    ///
    #[derive(Default)]
    pub(super) struct StorageDriveContext {
        /// List of all config sets registered in this storage.
        all_sets: HashMap<u64, Arc<SetContext>>,

        /// List of all registered backends within this storage.
        ///
        /// On every backend event, storage driver will iterate each session channels
        ///  and will try replication.
        backend_sessions: Vec<async_channel::Sender<BackendReplicateEvent>>,
    }

    impl StorageDriveContext {
        pub fn new() -> Self {
            Self {
                ..Default::default()
            }
        }

        pub async fn handle_once(&mut self, msg: ControlDirective) {
            use ControlDirective::*;

            match msg {
                Backend(msg) => {
                    // handles backend event in separate routine.
                    self.on_backend_event_(msg)
                }

                OnRegisterConfigSet(msg) => {
                    todo!("Register config set to `all_sets` table, and publish replication event")
                }

                OnUnregisterConfigSet(id) => {
                    todo!("")
                }

                EntityNotifyCommit {
                    register_id,
                    item_id,
                } => {
                    todo!()
                }

                EntityValueUpdate {
                    register_id,
                    item_id,
                } => {
                    todo!()
                }

                _ => unimplemented!(),
            }
        }

        fn on_backend_event_(&mut self, msg: BackendEvent) {}
    }
}
