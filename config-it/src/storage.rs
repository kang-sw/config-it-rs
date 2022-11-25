use std::{
    future::Future,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use crate::{
    config::{self, GroupContext},
    core::{self, ControlDirective, Error as ConfigError},
    entity::{self, EntityEventHook},
    monitor::StorageMonitorChannel,
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
    pub async fn create_group_ex<T: config::ConfigGroupData>(
        &self,
        path: Vec<CompactString>,
    ) -> Result<config::Group<T>, ConfigError> {
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
        let core = Arc::new(GroupContext {
            register_id,
            sources: Arc::new(sources),
            source_update_fence: AtomicUsize::new(0),
            update_receiver_channel: Mutex::new(broad_rx),
            path: path.clone(),
        });

        let (tx, rx) = oneshot::channel();
        match self
            .tx
            .send(ControlDirective::OnRegisterConfigGroup(
                core::ConfigGroupRegisterDesc {
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

        let group = crate::Group::<T>::create_with__(
            core,
            Arc::new(GroupUnregisterHook {
                register_id,
                tx: self.tx.clone(),
            }),
        );

        Ok(group)
    }

    pub async fn create_group<T>(
        &self,
        path: impl IntoIterator<Item = &str>,
    ) -> Result<config::Group<T>, ConfigError>
    where
        T: config::ConfigGroupData,
    {
        self.create_group_ex::<T>(
            path.into_iter()
                .map(|x| -> CompactString { x.into() })
                .collect::<Vec<_>>(),
        )
        .await
    }

    // TODO: Dump all contents I/O from/to Serializer/Deserializer

    ///
    /// # Arguments
    ///
    ///  * `data` -
    ///
    /// # Data serialization rule:
    ///  - The first path component is root component, which is written as-is.
    ///  - Any path component after the first must be prefixed with `#:` characters.
    ///    - Otherwise, they are treated as field element of enclosing path component.
    ///  - Any '#:' prefixed key
    ///
    /// ```json
    /// {
    ///     "root_path": {
    ///         "#:path_component": {
    ///             "field_name": "value",
    ///             "other_field": {
    ///                 "#:this_does_not_treated"
    ///             }
    ///         },
    ///         "#:another_path_component": {},
    ///         "field_name_of_root_path": "yay"
    ///     },
    ///     "another_root_path": {}
    /// }
    /// ```
    ///
    pub async fn load_data<'a>(&self, data: &dyn erased_serde::Deserializer<'a>) {
        todo!() // Create 'LoadedStorage' from deserializer.
    }

    pub fn create_monitor(&self) -> StorageMonitorChannel {
        StorageMonitorChannel {
            tx: self.tx.clone(),
        }
    }
}

struct GroupUnregisterHook {
    register_id: u64,
    tx: async_channel::Sender<ControlDirective>,
}

impl Drop for GroupUnregisterHook {
    fn drop(&mut self) {
        // Just ignore result. If channel was closed before the set is unregistered,
        //  it's ok to ignore this operation silently.
        let _ = self
            .tx
            .try_send(ControlDirective::OnUnregisterConfigGroup(self.register_id));
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
    use crate::core::{MonitorEvent, MonitorReplication};

    use super::*;
    use std::{
        collections::{hash_map::DefaultHasher, HashMap, HashSet},
        hash::{Hash, Hasher},
    };

    ///
    /// Drives storage internal events.
    ///
    /// - Receives update request
    ///
    #[derive(Default)]
    pub(super) struct StorageDriveContext {
        /// List of all config sets registered in this storage.
        all_groups: HashMap<u64, GroupRegistration>,

        /// List of all registered monitors within this storage.
        ///
        /// On every monitor event, storage driver will iterate each session channels
        ///  and will try replication.
        monitor_sessions: Vec<async_channel::Sender<MonitorReplication>>,

        /// Registered path hashes
        path_hashes: HashSet<u64>,
    }

    struct GroupRegistration {
        /// Category string array. Never empty.
        path_hash: u64,

        context: Arc<GroupContext>,
        evt_on_update: async_broadcast::Sender<()>,
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
                FromMonitor(msg) => {
                    // handles monitor event in separate routine.
                    self.on_monitor_event_(msg)
                }

                // Registers config set to `all_sets` table, and publish replication event
                OnRegisterConfigGroup(msg) => {
                    // Check if same named group exists inside of this storage
                    let mut hasher = DefaultHasher::new();
                    msg.context.path.hash(&mut hasher);

                    let path_hash = hasher.finish();
                    if !self.path_hashes.insert(path_hash) {
                        let _ = msg
                            .reply_success
                            .send(Err(ConfigError::GroupCreationFailed(msg.context.path.clone())));
                        return;
                    }

                    let rg = GroupRegistration {
                        context: msg.context,
                        path_hash,
                        evt_on_update: msg.event_broadcast,
                    };

                    // TODO: Apply initial update
                    //  - If any data is loaded previously, and could be found with

                    let prev = self.all_groups.insert(msg.register_id, rg);
                    assert!(prev.is_none(), "Key never duplicates");
                    let _ = msg.reply_success.send(Ok(()));

                    // TODO: Send 'new group' message to active monitors
                }

                OnUnregisterConfigGroup(id) => {
                    // TODO: Send 'deleted group' message to active monitors

                    // Erase from regist
                    let rg = self.all_groups.remove(&id).expect("Key must exist");
                    assert!(self.path_hashes.remove(&rg.path_hash));
                }

                EntityNotifyCommit {
                    register_id,
                    item_id,
                } => {
                    todo!("propagate-event-to-users")
                }

                EntityValueUpdate {
                    register_id,
                    item_id,
                } => {
                    todo!("propagate-event-to-subs")
                }

                NewSessionOpen {} => {
                    // TODO:
                }

                _ => unimplemented!(),
            }
        }

        fn on_monitor_event_(&mut self, msg: MonitorEvent) {}
    }
}
