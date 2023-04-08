use std::{
    any::{Any, TypeId},
    future::Future,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
};

use crate::{
    archive,
    config::{self, GroupContext},
    core::{
        self, ControlDirective, Error as ConfigError, GroupFindError, MonitorEvent,
        ReplicationEvent,
    },
    entity::{self, EntityEventHook},
    storage::detail::PathHash,
};
use compact_str::{CompactString, ToCompactString};
use log::debug;

///
/// Storage manages multiple sets registered by preset key.
///
#[derive(Clone)]
pub struct Storage {
    /// Internally, any request/notify is transferred through this channel.
    ///
    /// Thus, any reply-required operation becomes async inherently.
    pub(crate) tx: flume::Sender<ControlDirective>,
}

pub struct ImportOptions {
    /// If set this to true, the imported config will be merged onto existing cache. Usually turning
    /// this on is useful to prevent unsaved archive entity from being overwritten.
    pub merge_onto_cache: bool,

    /// If this option is enabled, imported setting will be converted into 'patch' before applied.
    /// Otherwise, the imported setting will be applied directly, and will affect to all properties
    /// that are included in the archive even if there is no actual change on archive content.
    pub apply_as_patch: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            merge_onto_cache: true,
            apply_as_patch: true,
        }
    }
}

pub struct ExportOptions {
    /// On export, storage collects only active instances of config groups. If this is set to true,
    /// collected result will be merged onto existing dump cache, thus it will preserve
    /// uninitialized config groups' archive data.
    ///
    /// Otherwise, only active config groups will be exported.
    pub merge_onto_dumped: bool,

    /// If this option is set true, storage will replace import cache with dumped export data.
    /// This will affect the next config group creation.
    pub replace_import_cache: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            merge_onto_dumped: true,
            replace_import_cache: true,
        }
    }
}

///
/// Create config storage and its driver pair.
///
pub fn create() -> (Storage, impl Future<Output = ()>) {
    let (tx, rx) = flume::unbounded();
    let driver = {
        async move {
            debug!("Config storage worker launched");
            let mut context = detail::StorageDriveContext::new();
            loop {
                match rx.recv_async().await {
                    Ok(ControlDirective::Close) => {
                        debug!("closing storage driver ...");
                        break;
                    }

                    Ok(msg) => {
                        context.handle_once(msg).await;
                    }

                    Err(e) => {
                        let ptr = &rx as *const _;
                        debug!("[{ptr:p}] ({e:?}) All sender channel has been closed. Closing storage ...");
                        break;
                    }
                }
            }
        }
    };

    (Storage { tx }, driver)
}

impl Storage {
    ///
    /// Creates new storage and its driver.
    ///
    /// The second tuple parameter is asynchronous loop which handles all storage events,
    ///  which must be spawned or blocked by runtime to make storage work. All storage
    ///
    #[deprecated(note = "Use global function 'create' instead")]
    pub fn new() -> (Self, impl Future<Output = ()>) {
        create()
    }

    pub fn close(&self) -> Result<(), core::Error> {
        self.tx
            .send(ControlDirective::Close)
            .map_err(|_| core::Error::ExpiredStorage)
    }

    ///
    /// First tries to find existing config set from path hash, and if it doesn't exist,
    /// tries to create new group.
    ///
    pub async fn find_group_ex<T: config::Template>(
        &self,
        path_hash: PathHash,
    ) -> Result<config::Group<T>, GroupFindError> {
        let (tx, rx) = oneshot::channel();
        let msg = ControlDirective::TryFindGroup {
            path_hash: path_hash.0,
            type_id: TypeId::of::<T>(),
            reply: tx,
        };

        fn err_expire<T>(_: T) -> GroupFindError {
            GroupFindError::ExpiredStorage
        }

        self.tx.send_async(msg).await.map_err(err_expire)?;

        let core::FoundGroupInfo {
            context,
            unregister_hook,
        } = rx.await.map_err(err_expire)??;

        Ok(<crate::Group<T>>::create_with__(context, unregister_hook))
    }

    /// Tries to find existing group from path name, and if it doesn't exist, tries to create new
    pub async fn find_or_create<T>(
        &self,
        path: impl IntoIterator<Item = impl ToCompactString>,
    ) -> Result<config::Group<T>, ConfigError>
    where
        T: config::Template,
    {
        let paths = path
            .into_iter()
            .map(|x| x.to_compact_string())
            .collect::<Vec<_>>();

        let path_hash = PathHash::new(paths.iter().map(|x| x.as_str()));
        match self.find_group_ex::<T>(path_hash).await {
            Ok(group) => Ok(group),
            Err(GroupFindError::PathNotFound) => Ok(self.create_group_ex::<T>(paths).await?),
            Err(GroupFindError::ExpiredStorage) => Err(ConfigError::ExpiredStorage),
            Err(GroupFindError::MismatchedTypeID) => Err(ConfigError::MismatchedTypeID),
        }
    }

    /// A shortcut for find_group_ex
    pub async fn find<'a, T: config::Template>(
        &self,
        path: impl IntoIterator<Item = &'a (impl AsRef<str> + ?Sized + 'a)>,
    ) -> Result<config::Group<T>, GroupFindError> {
        self.find_group_ex::<T>(PathHash::new(path.into_iter().map(|x| x.as_ref())))
            .await
    }

    ///
    /// Creates and register new config set.
    ///
    /// If path is duplicated for existing config set, the program will panic.
    ///
    pub async fn create_group_ex<T: config::Template>(
        &self,
        path: Vec<CompactString>,
    ) -> Result<config::Group<T>, ConfigError> {
        assert!(!path.is_empty(), "First argument must exist!");
        assert!(path.iter().all(|x| !x.is_empty()), "Empty path argument is not allowed!");

        // NOTE: this increments ID_GEN whether the creation succeeds or not
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

        let unregister_anchor = Arc::new(GroupUnregisterHook {
            register_id,
            tx: self.tx.clone(),
        });

        // Create core config set context with reflected target metadata set
        let (broad_tx, broad_rx) = async_broadcast::broadcast::<()>(1);
        let core = Arc::new(GroupContext {
            group_id: register_id,
            template_type_id: TypeId::of::<T>(),
            w_unregister_hook: Arc::downgrade(
                &(unregister_anchor.clone() as Arc<dyn Any + Send + Sync>),
            ),
            sources: Arc::new(sources),
            source_update_fence: AtomicUsize::new(1), // NOTE: This will trigger initial check_update() always.
            update_receiver_channel: broad_rx.deactivate(),
            path: path.clone(),
        });

        let (tx, rx) = oneshot::channel();
        match self
            .tx
            .send_async(ControlDirective::TryGroupRegister(
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

        let group = crate::Group::<T>::create_with__(core, unregister_anchor);
        Ok(group)
    }

    /// A easy-to-use version of `create_group_ex`.
    #[deprecated(note = "Use 'create()' instead")]
    pub async fn create_group<T>(
        &self,
        path: impl IntoIterator<Item = impl ToCompactString>,
    ) -> Result<config::Group<T>, ConfigError>
    where
        T: config::Template,
    {
        self.create(path).await
    }

    pub async fn create<T>(
        &self,
        path: impl IntoIterator<Item = impl ToCompactString>,
    ) -> Result<config::Group<T>, ConfigError>
    where
        T: config::Template,
    {
        self.create_group_ex::<T>(
            path.into_iter()
                .map(|x| x.to_compact_string())
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
    pub async fn export(&self, option: ExportOptions) -> Result<archive::Archive, core::Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send_async(ControlDirective::Export {
                destination: tx,
                option,
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
        option: ImportOptions,
    ) -> Result<(), core::Error> {
        self.tx
            .send_async(ControlDirective::Import {
                body: archive,
                option,
            })
            .await
            .map_err(|_| core::Error::ExpiredStorage)
    }

    ///
    /// Wait synchronization after calling 'import'
    ///
    pub async fn fence(&self) {
        async {
            let (tx, rx) = oneshot::channel();
            self.tx
                .send_async(ControlDirective::Fence(tx))
                .await
                .map(|_| rx)
                .ok()?
                .await
                .ok()
        }
        .await;
    }

    ///
    /// Create replication channel
    ///
    pub async fn monitor_open_replication_channel(
        &self,
    ) -> Result<ReplicationChannel, crate::core::Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send_async(ControlDirective::MonitorRegister { reply_to: tx })
            .await
            .map_err(|_| crate::core::Error::ExpiredStorage)?;

        rx.await.map_err(|_| crate::core::Error::ExpiredStorage)
    }

    ///
    /// Send monitor event to storage driver.
    ///
    pub async fn monitor_send_event(&self, evt: MonitorEvent) -> Result<(), core::Error> {
        self.tx
            .send_async(ControlDirective::FromMonitor(evt))
            .await
            .map_err(|_| core::Error::ExpiredStorage)
    }
}

///
/// A unbounded receiver channel which receives replication stream from storage.
///
/// By tracking this, one can synchronize its state with storage.
///
pub type ReplicationChannel = flume::Receiver<ReplicationEvent>;

struct GroupUnregisterHook {
    register_id: u64,
    tx: flume::Sender<ControlDirective>,
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
    tx: flume::Sender<ControlDirective>,
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
        core::{FoundGroupInfo, MonitorEvent, ReplicationEvent},
    };

    use super::*;
    use std::{
        collections::{hash_map::DefaultHasher, HashMap},
        hash::{Hash, Hasher},
    };

    ///
    /// A hash type for hash name
    ///
    #[derive(Debug, Clone, Copy, Hash, derive_more::From)]
    pub struct PathHash(pub u64);

    impl PathHash {
        pub fn new<'a>(paths: impl IntoIterator<Item = &'a str>) -> Self {
            let mut hasher = DefaultHasher::new();
            paths.into_iter().for_each(|x| x.hash(&mut hasher));
            Self(hasher.finish())
        }
    }

    ///
    /// Drives storage internal events.
    ///
    /// - Receives update request
    ///
    #[derive(Default)]
    pub(super) struct StorageDriveContext {
        /// List of all config sets registered in this storage.
        ///
        /// Uses group id as key.
        all_groups: HashMap<u64, GroupRegistration>,

        /// List of all registered monitors within this storage.
        ///
        /// On every monitor event, storage driver will iterate each session channels
        ///  and will try replication.
        monitors: MonitorList,

        /// Registered path hashes. Used to quickly compare if there are any path name
        ///  duplication.
        ///
        /// Uses path hash as key, group id as value.
        path_hashes: HashMap<u64, u64>,

        /// Cached archive. May contain contents for currently non-exist groups.
        archive: archive::Archive,
    }

    type MonitorList = Vec<flume::Sender<ReplicationEvent>>;

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
                    let path_hash = PathHash::new(msg.context.path.iter().map(|x| x.as_str())).0;
                    let mut inserted = false;

                    self.path_hashes.entry(path_hash).or_insert_with(|| {
                        inserted = true;
                        msg.group_id
                    });

                    if inserted == false {
                        let item = Err(ConfigError::GroupCreationFailed(msg.context.path.clone()));
                        let _ = msg.reply_success.send(item);
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

                TryFindGroup {
                    path_hash,
                    type_id,
                    reply,
                } => {
                    let Some(group_id) = self.path_hashes.get(&path_hash) else {
                        let _ = reply.send(Err(core::GroupFindError::PathNotFound));
                        return
                    };

                    let group = self.all_groups.get(&group_id).expect("logic error!");
                    if group.context.template_type_id != type_id {
                        let _ = reply.send(Err(core::GroupFindError::MismatchedTypeID));
                        return;
                    }

                    let Some(hook) = group.context.w_unregister_hook.upgrade() else {
                        // this is pretty corner case ... the group was disposed before
                        // `GroupDisposal` event being processed. This usually due to timing issue,
                        // and technically the group is being unregistered, so we can just treat
                        // this event as `PathNotFound` and let the caller retry registration.
                        let _ = reply.send(Err(core::GroupFindError::PathNotFound));
                        return
                    };

                    let _ = reply.send(Ok(FoundGroupInfo {
                        context: group.context.clone(),
                        unregister_hook: hook,
                    }));
                }

                GroupDisposal(id) => {
                    // Before dispose, dump existing content to archive.
                    let Some(rg) = self.all_groups.remove(&id) else {
                        // We can safely ignore not-found-group, as the logic has been changed to
                        // create unregister hook first whether the group creation succeeds or not.
                        return;
                    };

                    Self::dump_node_(&rg.context, &mut self.archive);

                    Self::send_repl_event_(&mut self.monitors, ReplicationEvent::GroupRemoved(id));

                    // Erase from registry
                    assert!(self.path_hashes.remove(&rg.path_hash).is_some());
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

                        let _ = group.evt_on_update.try_broadcast(());
                    }
                }

                MonitorRegister { reply_to } => {
                    // Create new unbounded reflection channel, flush all current state into it.
                    let (tx, rx) = flume::unbounded();
                    if reply_to.send(rx).is_err() {
                        log::warn!("MonitorRegister() canceled.");
                        return;
                    }

                    let args: Vec<_> = self
                        .all_groups
                        .iter()
                        .map(|e| (*e.0, e.1.context.clone()))
                        .collect();

                    if tx
                        .send_async(ReplicationEvent::InitialGroups(args))
                        .await
                        .is_err()
                    {
                        log::warn!("Initial replication data transfer failed.");
                        return;
                    }

                    self.monitors.push(tx);
                }

                Import { body, option } => {
                    let mut _apply_archive = |archive: &Archive| {
                        for (_id, group) in &self.all_groups {
                            let path = &group.context.path;
                            let path = path.iter().map(|x| x.as_str());
                            let Some(node) = archive.find_path(path) else { continue };

                            if Self::load_node_(&group.context, node, &mut self.monitors) {
                                let _ = group.evt_on_update.try_broadcast(());
                            }
                        }
                    };

                    if option.apply_as_patch {
                        let mut body = body;
                        let patch = self.archive.create_patch(&mut body);

                        _apply_archive(&patch);

                        if option.merge_onto_cache {
                            self.archive.merge_from(patch);
                        } else {
                            body.merge_from(patch);
                            self.archive = body;
                        }
                    } else {
                        if option.merge_onto_cache {
                            self.archive.merge_from(body);
                        } else {
                            self.archive = body;
                        }

                        _apply_archive(&self.archive);
                    }
                }

                Export {
                    destination,
                    option,
                } => {
                    let mut archive = Archive::default();
                    for (_, node) in &self.all_groups {
                        Self::dump_node_(&node.context, &mut archive);
                    }

                    let send_target = if !option.merge_onto_dumped {
                        if option.replace_import_cache {
                            self.archive = archive;
                            self.archive.clone()
                        } else {
                            archive
                        }
                    } else {
                        if option.replace_import_cache {
                            self.archive.merge_from(archive);
                            self.archive.clone()
                        } else {
                            archive.merge(self.archive.clone())
                        }
                    };

                    let _ = destination.send(send_target);
                }

                Fence(reply) => {
                    reply.send(()).ok();
                }

                Close => unreachable!(),
            }
        }

        fn handle_monitor_event_(&mut self, msg: MonitorEvent) {
            match msg {
                MonitorEvent::GroupUpdateNotify { mut updates } => {
                    updates.sort();
                    updates.dedup();

                    for group_id in updates {
                        let group = if let Some(g) = self.all_groups.get(&group_id) {
                            g
                        } else {
                            log::warn!("ValueUpdateNotify request failed for group [{group_id}]");
                            continue;
                        };

                        let sources = &group.context.sources;
                        debug_assert!(
                            sources.windows(2).all(|w| w[0].get_id() < w[1].get_id()),
                            "If sources are not sorted by item id, it's logic error!"
                        );

                        group
                            .context
                            .source_update_fence
                            .fetch_add(1, Ordering::AcqRel);

                        let _ = group.evt_on_update.try_broadcast(());
                    }
                }
            }
        }

        fn send_repl_event_(noti: &mut MonitorList, msg: ReplicationEvent) {
            noti.retain(|x| x.try_send(msg.clone()).is_ok());
        }

        fn dump_node_(ctx: &GroupContext, archive: &mut archive::Archive) {
            let paths = ctx.path.iter().map(|x| x.as_str());
            let node = archive.find_or_create_path_mut(paths);

            // Clear existing values before dumping.
            node.values.clear();

            for (meta, val) in ctx
                .sources
                .iter()
                .map(|e| e.get_value())
                .filter(|(meta, _)| !meta.props.disable_export)
            {
                let dst = node.values.entry(meta.name.into()).or_default();
                match val.to_json_value() {
                    Ok(val) => *dst = val,
                    Err(e) => log::warn!("failed to dump {}: {}", meta.name, e),
                }
            }
        }

        fn load_node_(ctx: &GroupContext, node: &archive::Archive, noti: &mut MonitorList) -> bool {
            let mut has_update = false;

            for (elem, de) in ctx
                .sources
                .iter()
                .map(|e| (e, e.get_meta()))
                .filter(|(_, m)| !m.props.disable_import)
                .filter_map(|(e, m)| {
                    node.values
                        .get(m.name)
                        .map(|o| (e, o.clone().into_deserializer()))
                })
            {
                match elem.update_value_from(de) {
                    Ok(_) => {
                        has_update = true;

                        Self::send_repl_event_(
                            noti,
                            ReplicationEvent::EntityValueUpdated {
                                group_id: ctx.group_id,
                                item_id: elem.get_id(),
                            },
                        )
                    }
                    Err(e) => {
                        log::warn!("Element value update error during node loading: {e:?}")
                    }
                }
            }

            if has_update {
                // On successful load, set its fence value as 1, to make the first client
                //  side's call to `update()` call would be triggered.
                ctx.source_update_fence.fetch_add(1, Ordering::Release);
            }

            has_update
        }
    }
}
