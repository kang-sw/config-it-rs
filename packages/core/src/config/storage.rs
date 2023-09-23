use std::{
    any::{Any, TypeId},
    mem::replace,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Weak,
    },
};

use strseq::SharedStringSequence;

use crate::{
    config::{entity, noti},
    shared::{archive, GroupID, ItemID, PathHash},
};

use super::{
    entity::EntityEventHook,
    group::{self, GroupContext},
};

/* ---------------------------------------------------------------------------------------------- */
/*                                      STORAGE MONITOR TRAIT                                     */
/* ---------------------------------------------------------------------------------------------- */

/// Monitors every storage actions. If monitor tracks every event of single storage, it can
/// replicate internal state perfectly.
///
/// Monitor should properly handle property flags, such as `ADMIN`, `READONLY`, `SECRET`, etc ...
///
/// TODO: Handle flags; `HIDDEN`

pub trait Monitor: Send + Sync + 'static {
    fn group_add_batch(&self, iter: &mut dyn Iterator<Item = (GroupID, Arc<GroupContext>)>) {
        let _ = iter;
    }
    fn group_added(&self, group_id: &GroupID, group: &Arc<GroupContext>) {
        let _ = (group_id, group);
    }
    fn group_removed(&self, group_id: &GroupID) {
        let _ = group_id;
    }
    fn entity_value_updated(&self, group_id: &GroupID, item_id: &ItemID) {
        let _ = (group_id, item_id);
    }
}

/* ---------------------------------------------------------------------------------------------- */
/*                                           STORAGE API                                          */
/* ---------------------------------------------------------------------------------------------- */
///
/// Storage manages multiple sets registered by preset key.
///
#[derive(Debug, Default, Clone)]
pub struct Storage(Arc<inner::Inner>);

#[derive(thiserror::Error, Debug)]
pub enum GroupFindError {
    #[error("Given path was not found")]
    PathNotFound,
    #[error("Type ID mismatch from original registration")]
    MismatchedTypeID,
}

#[derive(thiserror::Error, Debug)]
pub enum GroupFindOrCreateError {
    #[error("Type ID mismatch from original registration")]
    MismatchedTypeID,
}

#[derive(thiserror::Error, Debug)]
pub enum GroupCreationError {
    #[error("Path name duplicated, found early! Path was: {0:?}")]
    PathCollisionEarly(SharedStringSequence),
    #[error("Path name duplication found during registeration. Path was: {0:?}")]
    PathCollisionRace(SharedStringSequence),
}

impl Storage {
    /// Tries to find existing group from path name, and if it doesn't exist, tries to create new
    pub fn find_or_create<'a, T>(
        &self,
        path: impl IntoIterator<Item = impl AsRef<str> + 'a>,
    ) -> Result<group::Group<T>, GroupFindOrCreateError>
    where
        T: group::Template,
    {
        let keys = SharedStringSequence::from_iter(path);
        let path_hash = PathHash::new(keys.iter());

        use GroupCreationError as GCE;
        use GroupFindError as GFE;
        use GroupFindOrCreateError as GFOE;

        loop {
            match self.find(path_hash) {
                Ok(found) => break Ok(found),
                Err(GFE::MismatchedTypeID) => break Err(GFOE::MismatchedTypeID),
                Err(GFE::PathNotFound) => {}
            }

            match self.create_impl::<T>(keys.clone()) {
                Ok(created) => break Ok(created),

                // Simply retry on path collision
                Err(GCE::PathCollisionEarly(_) | GCE::PathCollisionRace(_)) => continue,
            }
        }
    }

    /// A shortcut for find_group_ex
    pub fn find<'a, T: group::Template>(
        &self,
        path: impl Into<PathHash>,
    ) -> Result<group::Group<T>, GroupFindError> {
        let path_hash = path.into();

        if let Some(group) = self.0.find_group(&path_hash) {
            if group.template_type_id != std::any::TypeId::of::<T>() {
                Err(GroupFindError::MismatchedTypeID)
            } else if let Some(anchor) = group.w_unregister_hook.upgrade() {
                Ok(group::Group::create_with__(group, anchor))
            } else {
                // This is corner case where group was disposed during `find_group` is invoked.
                Err(GroupFindError::PathNotFound)
            }
        } else {
            Err(GroupFindError::PathNotFound)
        }
    }

    pub fn create<'a, T>(
        &self,
        path: impl IntoIterator<Item = &'a (impl AsRef<str> + ?Sized + 'a)>,
    ) -> Result<group::Group<T>, GroupCreationError>
    where
        T: group::Template,
    {
        self.create_impl(path.into_iter().collect())
    }

    fn create_impl<T: group::Template>(
        &self,
        path: SharedStringSequence,
    ) -> Result<group::Group<T>, GroupCreationError> {
        assert!(!path.is_empty());
        assert!(path.iter().all(|x| !x.is_empty()));

        let path_hash = PathHash::new(path.iter());

        // Naively check if there's already existing group with same path.
        if let Some(_) = self.0.find_group(&path_hash) {
            return Err(GroupCreationError::PathCollisionEarly(path));
        }

        // This ID may not be used if group creation failed ... it's generally okay since we have
        // 2^63 trials.
        let register_id = GroupID::new_unique();
        let entity_hook = Arc::new(EntityHookImpl { register_id, inner: Arc::downgrade(&self.0) });

        debug_assert!(
            T::props__().windows(2).all(|x| x[0].index + 1 == x[1].index),
            "Something wrong with property generation"
        );

        let sources: Vec<_> = T::props__()
            .iter()
            .map(|prop| entity::EntityData::new(prop, entity_hook.clone()))
            .collect();

        // Drops the group when the final group instance is dropped.
        let unregister_anchor = Arc::new(GroupUnregisterHook {
            register_id,
            path_hash,
            inner: Arc::downgrade(&self.0),
        });

        // Create core config set context with reflected target metadata set
        let tx_noti = noti::Sender::new();
        let context = Arc::new(GroupContext {
            group_id: register_id,
            template_type_id: TypeId::of::<T>(),
            w_unregister_hook: Arc::downgrade(
                &(unregister_anchor.clone() as Arc<dyn Any + Send + Sync>),
            ),
            sources: sources.into(),
            version: AtomicU64::new(1), // NOTE: This will trigger initial check_update() always.
            update_receiver_channel: tx_noti.receiver(true),
            path: path.clone(),
        });

        self.0
            .register_group(path_hash, context, tx_noti)
            .map(|context| group::Group::create_with__(context, unregister_anchor))
    }

    /// Dump all  configs from storage.
    ///
    /// # Arguments
    ///
    /// * `no_merge` - If true, only active archive contents will be collected.
    ///   Otherwise, result will contain merge result of previously loaded archive.
    /// * `no_update` - If true, existing archive won't
    pub fn exporter(&self) -> inner::ExportTask {
        inner::ExportTask::new(&self.0)
    }

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
    ///                 "~this_is_not_treated_as_path": 123
    ///             }
    ///         },
    ///         "~another_path_component": {},
    ///         "field_name_of_root_path": "yay"
    ///     },
    ///     "another_root_path": {}
    /// }
    /// ```
    pub fn import(&self, archive: archive::Archive) -> inner::ImportOnDrop {
        inner::ImportOnDrop::new(&self.0, archive)
    }

    /// Replace existing monitor to given one.
    pub fn replace_monitor(&self, handler: Arc<impl Monitor>) -> Arc<dyn Monitor> {
        self.0.replace_monitor(handler)
    }

    /// Unset monitor instance.
    pub fn unset_monitor(&self) {
        *self.0.monitor.write() = Arc::new(inner::EmptyMonitor);
    }

    /// Send monitor event to storage driver.
    pub fn notify_editions(&self, items: impl IntoIterator<Item = GroupID>) {
        for group in items {
            self.0.notify_edition(group);
        }
    }

    /// Supply cypher key. If not specified until the first de/encryption, it will be generated from
    /// machine, or if the machine-uid generation is not available, it'll simply use some hard-coded
    /// sequence.
    pub fn set_encryption_key(&self, key: &[u8]) {
        todo!("Digest input key sequence")
    }
}

/* ---------------------------------------------------------------------------------------------- */
/*                                            INTERNALS                                           */
/* ---------------------------------------------------------------------------------------------- */

struct GroupUnregisterHook {
    register_id: GroupID,
    path_hash: PathHash,
    inner: Weak<inner::Inner>,
}

impl Drop for GroupUnregisterHook {
    fn drop(&mut self) {
        // Just ignore result. If channel was closed before the set is unregistered,
        //  it's ok to ignore this operation silently.
        let Some(inner) = self.inner.upgrade() else { return };
        inner.unregister_group(self.register_id, self.path_hash);
    }
}

struct EntityHookImpl {
    register_id: GroupID,
    inner: Weak<inner::Inner>,
}

impl EntityEventHook for EntityHookImpl {
    fn on_value_changed(&self, data: &entity::EntityData, silent: bool) {
        // Update notification is transient, thus when storage driver is busy, it can
        //  just be dropped.
        let Some(inner) = self.inner.upgrade() else { return };
        inner.on_value_update(self.register_id, data, silent);
    }
}

mod inner {
    use std::mem::ManuallyDrop;

    use dashmap::DashMap;
    use derive_setters::Setters;
    use parking_lot::RwLock;
    use serde::de::IntoDeserializer;

    use crate::{
        config::entity::EntityTrait,
        shared::{archive::Archive, meta::MetaFlag},
    };

    use super::*;

    /// Drives storage internal events.
    ///
    /// - Receives update request
    #[derive(cs::Debug)]
    pub(super) struct Inner {
        /// List of all config sets registered in this storage.
        ///
        /// Uses group id as key.
        all_groups: DashMap<GroupID, GroupRegistration>,

        /// List of all registered monitors within this storage.
        ///
        /// On every monitor event, storage driver will iterate each session channels
        ///  and will try replication.
        #[debug(with = "fmt_monitor")]
        pub monitor: RwLock<Arc<dyn Monitor>>,

        /// Registered path hashes. Used to quickly compare if there are any path name
        ///  duplication.
        ///
        /// Uses path hash as key, group id as value.
        path_hashes: DashMap<PathHash, GroupID>,

        /// Cached archive. May contain contents for currently non-exist groups.
        pub archive: RwLock<archive::Archive>,

        /// AES-256 key for encryption
        #[debug(with = "fmt_encrpytion_key")]
        encryption_key: RwLock<Option<[u8; 32]>>,
    }

    fn fmt_monitor(
        monitor: &RwLock<Arc<dyn Monitor>>,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        let ptr = &**monitor.read() as *const _;
        let exists = ptr != &EmptyMonitor as *const _;

        write!(f, "{:?}", exists.then_some(ptr))
    }

    fn fmt_encrpytion_key(
        key: &RwLock<Option<[u8; 32]>>,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        let exists = key.read().is_some();
        write!(f, "{:?}", exists)
    }

    /// A dummy monitor class, which represnet empty monitor.
    pub(super) struct EmptyMonitor;
    impl Monitor for EmptyMonitor {}

    impl Default for Inner {
        fn default() -> Self {
            Self::new(Arc::new(EmptyMonitor))
        }
    }

    #[derive(Debug)]
    struct GroupRegistration {
        context: Arc<GroupContext>,
        evt_on_update: noti::Sender,
    }

    impl Inner {
        pub fn new(monitor: Arc<dyn Monitor>) -> Self {
            Self {
                all_groups: Default::default(),
                monitor: RwLock::new(monitor),
                path_hashes: Default::default(),
                archive: Default::default(),
                encryption_key: Default::default(),
            }
        }

        pub fn notify_edition(&self, group_id: GroupID) {
            if let Some(group) = self.all_groups.get(&group_id) {
                group.context.version.fetch_add(1, Ordering::Relaxed);
                group.evt_on_update.notify();
            }
        }

        pub fn find_group(&self, path_hash: &PathHash) -> Option<Arc<GroupContext>> {
            self.path_hashes
                .get(path_hash)
                .and_then(|id| self.all_groups.get(&*id))
                .map(|ctx| ctx.context.clone())
        }

        pub fn register_group(
            &self,
            path_hash: PathHash,
            context: Arc<GroupContext>,
            evt_on_update: noti::Sender,
        ) -> Result<Arc<GroupContext>, GroupCreationError> {
            // Path-hash to GroupID mappings can be collided if there's simultaneous access.
            // Therefore treat group insertion as success only when path_hash was successfully
            // registered to corresponding group ID.
            let group_id = context.group_id;
            let inserted = context.clone();

            let rg = GroupRegistration { context, evt_on_update };

            if let Some(node) = self.archive.read().find_path(rg.context.path.iter()) {
                Self::load_node(&rg.context, node, &EmptyMonitor);
            }

            assert!(
                self.all_groups.insert(group_id, rg).is_none(),
                "Group ID never be duplicated." // as long as we didn't consume all 2^64 candidates ...
            );

            if self.path_hashes.entry(path_hash).or_insert(group_id).value() != &group_id {
                // This is corner case where path hash was registered by other thread. In this case,
                // we should remove group registration and return error.
                self.all_groups.remove(&group_id);
                return Err(GroupCreationError::PathCollisionRace(inserted.path.clone()));
            }

            self.monitor.read().group_added(&group_id, &inserted);
            Ok(inserted)
        }

        pub fn unregister_group(&self, group_id: GroupID, path_hash: PathHash) {
            self.path_hashes.remove_if(&path_hash, |_, v| v == &group_id);
            if let Some((_, ctx)) = self.all_groups.remove(&group_id) {
                // Treat this remove as valid only when the group has ever been validly created.
                // `all_groups.remove` can return `None` if the group wasn't successfully registered
                // during `register_group`. In this case, this function will be called on disposal
                // of `GroupUnregisterHook` inside of the function `create_impl`.

                // On valid removal, accumulate contents to cached archive.
                Self::dump_node(&ctx.context, &mut self.archive.write());

                // Notify removal
                self.monitor.read().group_removed(&group_id);
            }
        }

        pub fn on_value_update(&self, group_id: GroupID, data: &entity::EntityData, silent: bool) {
            // Monitor should always be notified on value update, regardless of silent flag
            self.monitor.read().entity_value_updated(&group_id, &data.get_id());

            // If silent flag is set, skip internal notify to other instances.
            if silent {
                return;
            }

            // This is trivially fallible operation.
            let Some(group) = self.all_groups.get(&group_id) else { return };
            group.context.version.fetch_add(1, Ordering::Relaxed);
            group.evt_on_update.notify();
        }

        pub fn replace_monitor(&self, new_monitor: Arc<dyn Monitor>) -> Arc<dyn Monitor> {
            let old_monitor = replace(&mut *self.monitor.write(), new_monitor.clone());
            new_monitor.group_add_batch(&mut self.path_hashes.iter().filter_map(|elem| {
                self.all_groups.get(&elem.value()).map(|e| (*e.key(), e.context.clone()))
            }));
            old_monitor
        }

        const CRYPTO_PREFIX: &'static str = "%%CONFIG-IT-SECRET%%";

        fn dump_node(ctx: &GroupContext, archive: &mut archive::Archive) {
            let paths = ctx.path.iter();
            let node = archive.find_or_create_path_mut(paths);

            // Clear existing values before dumping.
            node.values.clear();

            for (meta, mut val) in ctx
                .sources
                .iter()
                .map(|e| e.get_value())
                .filter(|(meta, _)| !meta.metadata.flags.contains(MetaFlag::NO_EXPORT))
            {
                let dst = node.values.entry(meta.name.into()).or_default();

                if meta.metadata.flags.contains(MetaFlag::SECRET) {
                    todo!("JsonString => AES-256 Enc => Base64String => Prefix");
                }

                match serde_json::to_value(val.as_serialize()) {
                    Ok(val) => *dst = val,
                    Err(e) => log::warn!("failed to dump {}: {}", meta.name, e),
                }
            }
        }

        fn load_node(ctx: &GroupContext, node: &archive::Archive, monitor: &dyn Monitor) -> bool {
            let mut has_update = false;

            'outer: for (elem, de) in ctx
                .sources
                .iter()
                .map(|e| (e, e.get_meta()))
                .filter(|(_, m)| !m.metadata.flags.contains(MetaFlag::NO_IMPORT))
                .filter_map(|(e, m)| {
                    node.values.get(m.name).map(|o| (e, o.clone().into_deserializer()))
                })
            {
                'decryption: {
                    if !elem.get_meta().metadata.flags.contains(MetaFlag::SECRET) {
                        break 'decryption;
                    }

                    let Some(str) = de.as_str() else { break 'decryption };

                    if !str.starts_with(Self::CRYPTO_PREFIX) {
                        break 'decryption;
                    }

                    todo!("'Prefixed' Base64String => AES-256 Dec => JsonValue");

                    continue 'outer;
                }

                match elem.update_value_from(de) {
                    Ok(_) => {
                        has_update = true;
                        monitor.entity_value_updated(&ctx.group_id, &elem.get_id());
                    }
                    Err(e) => {
                        log::warn!("Element value update error during node loading: {e:?}")
                    }
                }
            }

            if has_update {
                // On successful load, set its fence value as 1, to make the first client
                //  side's call to `update()` call would be triggered.
                ctx.version.fetch_add(1, Ordering::Release);
            }

            has_update
        }
    }

    /* ------------------------------------ Import Operation ------------------------------------ */
    #[derive(Setters)]
    #[setters(borrow_self)]
    pub struct ImportOnDrop<'a> {
        #[setters(skip)]
        inner: &'a Inner,

        #[setters(skip)]
        archive: ManuallyDrop<Archive>,

        /// If set this to true, the imported config will be merged onto existing cache. Usually turning
        /// this on is useful to prevent unsaved archive entity from being overwritten.
        merge_onto_cache: bool,

        /// If this option is enabled, imported setting will be converted into 'patch' before applied.
        /// Otherwise, the imported setting will be applied directly, and will affect to all properties
        /// that are included in the archive even if there is no actual change on archive content.
        apply_as_patch: bool,
    }

    impl<'a> ImportOnDrop<'a> {
        pub(super) fn new(inner: &'a Inner, archive: Archive) -> Self {
            Self {
                inner,
                archive: ManuallyDrop::new(archive),
                merge_onto_cache: true,
                apply_as_patch: true,
            }
        }
    }

    impl<'a> Drop for ImportOnDrop<'a> {
        fn drop(&mut self) {
            // SAFETY: Typical `ManuallyDrop` usage.
            let mut imported = unsafe { ManuallyDrop::take(&mut self.archive) };
            let this = self.inner;

            let import_archive = |archive: &Archive| {
                for elem in this.all_groups.iter() {
                    let group = elem.value();
                    let path = &group.context.path;
                    let path = path.iter();
                    let Some(node) = archive.find_path(path) else { continue };

                    if Inner::load_node(&group.context, node, &**this.monitor.read()) {
                        group.evt_on_update.notify();
                    }
                }
            };

            let mut self_archive = this.archive.write();
            if self.apply_as_patch {
                let patch = self_archive.create_patch(&mut imported);
                import_archive(&patch);

                if self.merge_onto_cache {
                    self_archive.merge_from(patch);
                } else {
                    imported.merge_from(patch);
                    *self_archive = imported;
                }
            } else {
                if self.merge_onto_cache {
                    self_archive.merge_from(imported);
                } else {
                    *self_archive = imported;
                }

                import_archive(&self_archive);
            }
        }
    }

    /* ------------------------------------ Export Operation ------------------------------------ */
    #[derive(Setters)]
    pub struct ExportTask<'a> {
        #[setters(skip)]
        inner: &'a Inner,

        /// On export, storage collects only active instances of config groups. If this is set to true,
        /// collected result will be merged onto existing dump cache, thus it will preserve
        /// uninitialized config groups' archive data.
        ///
        /// Otherwise, only active config groups will be exported.
        merge_onto_dumped: bool,

        /// If this option is set true, storage will replace import cache with dumped export data.
        /// This will affect the next config group creation.
        replace_import_cache: bool,
    }

    impl<'a> ExportTask<'a> {
        pub(super) fn new(inner: &'a Inner) -> Self {
            Self { inner, merge_onto_dumped: true, replace_import_cache: true }
        }

        /// Performs export operation with given settings
        pub fn collect(self) -> Archive {
            let mut archive = Archive::default();
            let this = self.inner;

            for elem in this.all_groups.iter() {
                let group = elem.value();
                Inner::dump_node(&group.context, &mut archive);
            }

            let mut self_archive = this.archive.write();
            if !self.merge_onto_dumped {
                if self.replace_import_cache {
                    *self_archive = archive;
                    self_archive.clone()
                } else {
                    archive
                }
            } else if self.replace_import_cache {
                self_archive.merge_from(archive);
                self_archive.clone()
            } else {
                archive.merge(self_archive.clone())
            }
        }
    }
}
