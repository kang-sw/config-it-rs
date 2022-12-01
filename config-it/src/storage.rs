use std::{
    future::Future,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use crate::{
    archive,
    config::{self, GroupContext},
    core::{self, ControlDirective, Error as ConfigError},
    entity::{self, EntityEventHook},
    monitor::StorageMonitor,
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
        assert!(!path.is_empty(), "First argument must exist!");
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
            group_id: register_id,
            sources: Arc::new(sources),
            source_update_fence: AtomicUsize::new(1), // NOTE: This will trigger initial check_update() always.
            update_receiver_channel: Mutex::new(broad_rx),
            path: path.clone(),
        });

        let (tx, rx) = oneshot::channel();
        match self
            .tx
            .send(ControlDirective::TryGroupRegister(
                core::GroupRegisterParam {
                    group_id: register_id,
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

    ///
    /// Dump all  configs from storage.
    ///
    /// # Arguments
    ///
    /// * `no_merge` - If true, only active archive contents will be collected.
    ///   Otherwise, result will contain merge result of previously loaded archive.
    /// * `no_update` - If true, existing archive won't
    ///
    pub async fn export(
        &self,
        no_merge: Option<bool>,
        no_update: Option<bool>,
    ) -> Result<archive::Archive, core::Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ControlDirective::Export {
                destination: tx,
                no_merge_exist: no_merge.unwrap_or(false),
                no_update: no_update.unwrap_or(false),
            })
            .await
            .map_err(|_| core::Error::ExpiredStorage)?;

        rx.await.map_err(|_| core::Error::ExpiredStorage)
    }

    ///
    /// Deserializes data
    ///
    /// # Arguments
    ///
    /// * `de` - Deserializer
    /// * `merge` - True if loaded archive should merge onto existing one. Otherwise, it'll replace
    ///             currently loaded archive data.
    ///
    /// # Data serialization rule:
    ///  - The first path component is root component, which is written as-is.
    ///  - Any path component after the first must be prefixed with `~(tilde)` character.
    ///    - Otherwise, they are treated as field element of enclosing path component.
    ///  - Any '~' prefixed key inside of existing field
    ///
    /// ```json
    /// {
    ///     "root_path": {
    ///         "~path_component": {
    ///             "field_name": "value",
    ///             "other_field": {
    ///                 "~this_is_not_treated_as_path"
    ///             }
    ///         },
    ///         "~another_path_component": {},
    ///         "field_name_of_root_path": "yay"
    ///     },
    ///     "another_root_path": {}
    /// }
    /// ```
    ///
    pub async fn import(
        &self,
        archive: archive::Archive,
        merge: Option<bool>,
    ) -> Result<(), core::Error> {
        self.tx
            .send(ControlDirective::Import {
                body: archive,
                merge_onto_exist: merge.unwrap_or(true),
            })
            .await
            .map_err(|_| core::Error::ExpiredStorage)
    }

    pub fn create_monitor(&self) -> StorageMonitor {
        StorageMonitor {
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
            .try_send(ControlDirective::GroupDisposal(self.register_id));
    }
}

struct EntityHookImpl {
    register_id: u64,
    tx: async_channel::Sender<ControlDirective>,
}

impl EntityEventHook for EntityHookImpl {
    fn on_value_changed(&self, data: &entity::EntityData, silent: bool) {
        // Update notification is transient, thus when storage driver is busy, it can
        //  just be dropped.
        let _ = self.tx.try_send(ControlDirective::EntityValueUpdate {
            group_id: self.register_id,
            item_id: data.get_id(),
            silent_mode: silent,
        });
    }
}

mod detail {
    use serde::de::IntoDeserializer;

    use crate::{
        archive::{self, Archive},
        core::{MonitorEvent, ReplicationEvent},
        entity::{EntityData, EntityTrait},
    };

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
        monitors: Vec<async_channel::Sender<ReplicationEvent>>,

        /// Registered path hashes. Used to quickly compare if there are any path name
        ///  duplication.
        path_hashes: HashSet<u64>,

        /// Cached archive. May contain contents for currently non-exist groups.
        archive: archive::Archive,
    }

    type MonitorList = Vec<async_channel::Sender<ReplicationEvent>>;

    struct GroupRegistration {
        /// Hash calculated from `context.path`.
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
                    self.handle_monitor_event_(msg)
                }

                // Registers config set to `all_sets` table, and publish replication event
                TryGroupRegister(msg) => {
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
                        context: msg.context.clone(),
                        path_hash,
                        evt_on_update: msg.event_broadcast,
                    };

                    // Apply initial update from loaded value.
                    if let Some(node) = self
                        .archive
                        .find_path(msg.context.path.iter().map(|x| x.as_str()))
                    {
                        // Apply node values to newly generated context.
                        Self::load_node_(&*rg.context, node, &mut self.monitors);
                    }

                    let prev = self.all_groups.insert(msg.group_id, rg);
                    assert!(prev.is_none(), "Key never duplicates");
                    let _ = msg.reply_success.send(Ok(()));

                    Self::send_repl_event_(
                        &mut self.monitors,
                        ReplicationEvent::GroupAdded(msg.group_id, msg.context),
                    );
                }

                GroupDisposal(id) => {
                    // Before dispose, dump existing content to archive.
                    let rg = self.all_groups.remove(&id).expect("Key must exist!");
                    Self::dump_node_(&rg.context, &mut self.archive);

                    Self::send_repl_event_(&mut self.monitors, ReplicationEvent::GroupRemoved(id));

                    // Erase from registry
                    assert!(self.path_hashes.remove(&rg.path_hash));
                }

                EntityValueUpdate {
                    group_id,
                    item_id,
                    silent_mode,
                } => {
                    let Some(group) = self.all_groups.get(&group_id) else { return };

                    // - Notify monitors value change
                    // - If it's silent mode, do not step group update fence forward.
                    //   Thus, this update will not trigger all group's update.
                    //   Otherwise, step group update fence, and propagate group update event
                    Self::send_repl_event_(
                        &mut self.monitors,
                        ReplicationEvent::EntityValueUpdated { group_id, item_id },
                    );

                    if !silent_mode {
                        group
                            .context
                            .source_update_fence
                            .fetch_add(1, Ordering::Release);

                        let _ = group.evt_on_update.broadcast(()).await;
                    }
                }

                MonitorRegister { reply_to: _ } => {
                    // TODO: Create new unbounded reflection channel, flush all current state into it.
                }

                Import {
                    body,
                    merge_onto_exist: merged,
                } => {
                    if merged {
                        self.archive.merge_with(body);
                    } else {
                        self.archive = body;
                    }

                    for (_id, group) in &self.all_groups {
                        let path = &group.context.path;
                        let path = path.iter().map(|x| x.as_str());
                        let Some(node) = self.archive.find_path(path) else { continue };

                        if Self::load_node_(&group.context, node, &mut self.monitors) {
                            let _ = group.evt_on_update.broadcast(()).await;
                        }
                    }
                }

                Export {
                    destination,
                    no_merge_exist,
                    no_update,
                } => {
                    let mut archive = Archive::default();
                    for (_, node) in &self.all_groups {
                        Self::dump_node_(&node.context, &mut archive);
                    }

                    let send_target = if no_merge_exist {
                        if no_update {
                            archive
                        } else {
                            self.archive = archive;
                            self.archive.clone()
                        }
                    } else {
                        if no_update {
                            self.archive.clone().merge(archive)
                        } else {
                            self.archive.merge_with(archive);
                            self.archive.clone()
                        }
                    };

                    let _ = destination.send(send_target);
                }
            }
        }

        fn handle_monitor_event_(&mut self, _msg: MonitorEvent) {
            todo!("handle_monitor_event_")
        }

        fn send_repl_event_(noti: &mut MonitorList, msg: ReplicationEvent) {
            noti.retain(|x| x.try_send(msg.clone()).is_ok());
        }

        fn dump_node_(ctx: &GroupContext, archive: &mut archive::Archive) {
            let paths = ctx.path.iter().map(|x| x.as_str());
            let node = archive.find_or_create_path_mut(paths);

            // Clear existing values before dumping.
            node.values.clear();
            let mut buf = Vec::<u8>::with_capacity(128);

            for elem in &*ctx.sources {
                let (meta, val) = elem.access_value();
                let dst = node.values.entry(meta.name.into()).or_default();

                // HACK: Find more efficient way to create json::Value from EntityValue ...
                // HACK: Current implementation naively dumps json -> load it back to serde_json::Value
                buf.clear();
                let mut ser = serde_json::Serializer::new(&mut buf);

                if val
                    .serialize(&mut <dyn erased_serde::Serializer>::erase(&mut ser))
                    .is_err()
                {
                    // Serialization has failed, do not use the result.
                    continue;
                }

                if let Ok(val) = serde_json::from_slice(&buf[0..]) {
                    *dst = val;
                }
            }

            dbg!(node);
        }

        fn load_node_(ctx: &GroupContext, node: &archive::Node, noti: &mut MonitorList) -> bool {
            let mut has_update = false;

            for elem in &*ctx.sources {
                let meta = elem.get_meta();
                let Some(value) = node.values.get(meta.name) else { continue };

                let de = value.clone().into_deserializer();

                if Self::update_elem_by_(elem, de) {
                    has_update = true;

                    Self::send_repl_event_(
                        noti,
                        ReplicationEvent::EntityValueUpdated {
                            group_id: ctx.group_id,
                            item_id: elem.get_id(),
                        },
                    )
                }
            }

            if has_update {
                // On successful load, set its fence value as 1, to make the first client
                //  side's call to `update()` call would be triggered.
                ctx.source_update_fence.fetch_add(1, Ordering::Release);
            }

            has_update
        }

        fn update_elem_by_<'a, T>(elem: &EntityData, de: T) -> bool
        where
            T: serde::Deserializer<'a>,
        {
            let meta = elem.get_meta();
            let mut erased = <dyn erased_serde::Deserializer>::erase(de);
            let mut built = (meta.fn_default)();

            match built.deserialize(&mut erased) {
                Ok(_) => {
                    match (meta.fn_validate)(&*meta, built.as_any_mut()) {
                        Some(_) => (),
                        None => return false,
                    };

                    let built: Arc<dyn EntityTrait> = built.into();
                    elem.update_value(built.clone());
                    true
                }
                Err(e) => {
                    log::error!(
                        "(Deserialization Failed) {}(var:{}) \n\nERROR: {e:#?}",
                        meta.name,
                        meta.props.varname,
                    );
                    false
                }
            }
        }
    }
}
