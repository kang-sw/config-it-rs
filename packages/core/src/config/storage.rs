//! The `storage` module provides a high-level interface to interact with configuration data,
//! ensuring safety, flexibility, and efficiency.
//!
//! This module primarily revolves around the `Storage` struct, which serves as the main access
//! point for users to interact with the underlying storage system. By abstracting intricate
//! operations into straightforward methods, it simplifies user interaction with stored
//! configuration data.
//!
//! Key features include:
//! - **Data Retrieval and Creation**: Safely find or create items with `find_or_create`.
//! - **Data Import/Export**: Handle complex serialization and deserialization logic seamlessly with
//!   `import` and `exporter`.
//! - **Monitoring**: Integrate external monitoring systems and receive updates about storage
//!   modifications using the `replace_monitor`, `unset_monitor`, and `notify_editions` methods.
//! - **Encryption Support**: Securely encrypt data (when the encryption feature is enabled) using
//!   `set_encryption_key`.

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
/// Blocking behavior may cause deadlock, thus monitor should be implemented as non-blocking
/// manner. (e.g. forwarding events to channel)
pub trait Monitor: Send + Sync + 'static {
    /// Called when new group is added.
    fn group_added(&self, group_id: &GroupID, group: &Arc<GroupContext>) {
        let _ = (group_id, group);
    }

    /// Can be call with falsy group_id if monitor is being replaced.
    fn group_removed(&self, group_id: &GroupID) {
        let _ = group_id;
    }

    /// Called when any entity value is updated. Can be called with falsy group_id if monitor is
    /// being replaced.
    fn entity_value_updated(&self, group_id: &GroupID, item_id: &ItemID) {
        let _ = (group_id, item_id);
    }
}

/* ---------------------------------------------------------------------------------------------- */
/*                                           STORAGE API                                          */
/* ---------------------------------------------------------------------------------------------- */
/// Provides a high-level, thread-safe interface to the configuration storage system.
///
/// `Storage` acts as the primary access point for users to interact with the underlying storage
/// system. It abstracts away the intricacies of direct storage interactions by wrapping around the
/// `inner::Inner` type, ensuring concurrent safety.
///
/// With `Storage`, users can seamlessly perform read and write operations on their configuration
/// data without worrying about potential concurrency issues. This design ensures that the storage
/// system remains robust and efficient, even in multi-threaded environments.
///
/// # Features:
/// - **Thread Safety**: Guarantees safe concurrent access to the configuration storage.
/// - **High-level Interface**: Abstracts the complexities of direct storage interactions, offering
///   a user-friendly API.
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
    /// Searches for an existing item of type `T` in the storage, or creates a new one if it doesn't
    /// exist.
    ///
    /// # Arguments
    ///
    /// * `T` - The type of item to search for or create.
    ///
    /// # Returns
    ///
    /// A reference to the existing or newly created item of type `T`. Value remains in template
    /// default until you call first `update()` on it.
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

    /// Find a group with the given path and template type.
    ///
    /// # Arguments
    ///
    /// * `path` - The path of the group to find.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type of the template to use for the group.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the found group or a `GroupFindError` if the group was not
    /// found or if the template type does not match the expected type. Value remains in template
    /// default until you call first `update()` on it.
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

    /// Creates a new instance of the `Storage` struct with the specified type parameter. Value
    /// remains in template default until you call first `update()` on it.
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
        let register_id = GroupID::new_unique_incremental();
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

    /// Create internal archive export task.
    ///
    /// You should explicitly call `confirm()` to retrieve the exported archive explcitly.
    pub fn exporter(&self) -> inner::ExportTask {
        inner::ExportTask::new(&self.0)
    }

    /// Deserializes the data.
    ///
    /// # Data Serialization Rules:
    /// - The root component is the first path component and is written as-is.
    /// - Subsequent path components must be prefixed with a `~` (tilde) character.
    ///   - If not prefixed, they are treated as a field element of the enclosing path component.
    /// - A key prefixed with '~' within an existing field is ...
    ///   (Note: The comment here seems to be incomplete; please provide further details.)
    ///
    /// # Example JSON structure:
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
    ///
    /// # Returns
    ///
    /// An instance of `ImportOnDrop` which will handle the import operation.
    pub fn import(&self, archive: archive::Archive) -> inner::ImportOnDrop {
        inner::ImportOnDrop::new(&self.0, archive)
    }

    /// Replaces the current monitor with the provided one.
    ///
    /// This function dumps the active list of groups to the new monitor sequentially. If the
    /// monitor is not efficiently implemented, this operation can significantly impact the
    /// performance of all storage consumers and replicators. Therefore, exercise caution when
    /// replacing the monitor on a storage instance that's under heavy use.
    ///
    /// # Arguments
    ///
    /// * `handler` - The new monitor to replace the current one.
    ///
    /// # Returns
    ///
    /// The previous monitor that has been replaced.
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

    /// Sets the encryption key.
    ///
    /// If the encryption key is not provided before the first encryption or decryption operation, it will be automatically generated based on the machine's unique identifier (UID). If machine-UID generation is not supported, a predefined, hard-coded sequence will be used as the key.
    ///
    /// # Arguments
    ///
    /// * `key` - Byte slice representing the encryption key.
    #[cfg(feature = "encryption")]
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
        config::entity::Entity,
        shared::{archive::Archive, meta::MetaFlag},
    };

    use super::*;

    /// Manages and drives internal storage events.
    ///
    /// Primarily responsible for handling update requests and orchestrating
    /// the underlying storage mechanisms.
    #[derive(cs::Debug)]
    pub(super) struct Inner {
        /// Maintains a registry of all configuration sets within this storage.
        ///
        /// The key is the group's unique identifier, `GroupID`.
        all_groups: DashMap<GroupID, GroupRegistration>,

        /// Maintains a list of all monitors registered to this storage.
        ///
        /// Upon each monitoring event, the storage driver iterates over each session channel to
        /// attempt replication. This ensures that all components are kept in sync with storage
        /// changes.
        #[debug(with = "fmt_monitor")]
        pub monitor: RwLock<Arc<dyn Monitor>>,

        /// Keeps track of registered path hashes to quickly identify potential path name
        /// duplications.
        ///
        /// Uses the path's hash representation as the key and its corresponding `GroupID` as the
        /// value.
        path_hashes: DashMap<PathHash, GroupID>,

        /// Holds a cached version of the archive. This may include content for groups that are
        /// currently non-existent.
        pub archive: RwLock<archive::Archive>,

        /// AES-256 encryption key for securing data.
        ///
        /// This key is used when the encryption feature is enabled. It ensures that stored data is
        /// encrypted, adding an additional layer of security.
        #[cfg(feature = "encryption")]
        #[debug(with = "fmt_encryption_key")]
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

    #[cfg(feature = "encryption")]
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
                #[cfg(feature = "encryption")]
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
            // Path-hash to GroupID mappings might experience collisions due to simultaneous access.
            // To ensure integrity, only consider the group insertion successful when its path_hash
            // is successfully registered to its corresponding group ID.
            let group_id = context.group_id;
            let inserted = context.clone();
            let rg = GroupRegistration { context, evt_on_update };

            // If the path already exists in the archive, load the corresponding node.
            if let Some(node) = self.archive.read().find_path(rg.context.path.iter()) {
                Self::load_node(&rg.context, node, &EmptyMonitor);
            }

            // Ensure that the Group ID is unique.
            assert!(
                self.all_groups.insert(group_id, rg).is_none(),
                "Group IDs must be unique." // Ensure we haven't exhausted all 2^64 possibilities.
            );

            // Check for path-hash collisions. In the rare case where a collision occurs due to
            // another thread registering the same path-hash, we remove the current group
            // registration and return an error.
            if self.path_hashes.entry(path_hash).or_insert(group_id).value() != &group_id {
                self.all_groups.remove(&group_id);
                return Err(GroupCreationError::PathCollisionRace(inserted.path.clone()));
            }

            // Notify the monitor that a new group has been added.
            self.monitor.read().group_added(&group_id, &inserted);
            Ok(inserted)
        }

        pub fn unregister_group(&self, group_id: GroupID, path_hash: PathHash) {
            self.path_hashes.remove_if(&path_hash, |_, v| v == &group_id);
            if let Some((_, ctx)) = self.all_groups.remove(&group_id) {
                // Consider the removal valid only if the group was previously and validly created.
                // The method `all_groups.remove` might return `None` if the group was not
                // successfully registered during `register_group`. If this happens, this function
                // gets invoked during the disposal of `GroupUnregisterHook` within the
                // `create_impl` function.

                // For valid removals, add contents to the cached archive.
                Self::dump_node(&ctx.context, &mut self.archive.write());

                // Notify about the removal
                self.monitor.read().group_removed(&group_id);
            }
        }

        pub fn on_value_update(&self, group_id: GroupID, data: &entity::EntityData, silent: bool) {
            // Monitor should always be notified on value update, regardless of silent flag
            self.monitor.read().entity_value_updated(&group_id, &data.id);

            // If silent flag is set, skip internal notify to other instances.
            if silent {
                return;
            }

            // This is trivially fallible operation.
            if let Some(group) = self.all_groups.get(&group_id) {
                group.context.version.fetch_add(1, Ordering::Relaxed);
                group.evt_on_update.notify();
            }
        }

        pub fn replace_monitor(&self, new_monitor: Arc<dyn Monitor>) -> Arc<dyn Monitor> {
            // At the start of the operation, we replace the monitor. This means that before we add all
            // valid groups to the new monitor, there may be notifications about group removals or updates.
            // Without redesigning the entire storage system into a more expensive locking mechanism,
            // there's no way to avoid this. We assume that monitor implementation providers will handle
            // any incorrect updates gracefully and thus, we are ignoring this case.

            let old_monitor = replace(&mut *self.monitor.write(), new_monitor.clone());

            // NOTE: Ensuring thread-safe behavior during the initialization of a new monitor:
            // - Iteration partially locks `path_hashes` based on shards (see DashMap
            //   implementation).
            // - This means that new group insertions or removals can occur during iteration.
            // - However, it's guaranteed that while iterating over a shard, no other thread can
            //   modify the same shard of `path_hashes`.
            // - Since every group insertion or removal first modifies `path_hashes`, it's safe to
            //   assume we see a consistent state of `path_hashes` during shard iteration, given
            //   we're observing the read-locked state of that shard.
            for path in self.path_hashes.iter() {
                let Some(group) = self.all_groups.get(&path.value()) else {
                    unreachable!(
                        "As long as the group_id is found from path_hashes, \
                        the group must be found from `all_groups`."
                    )
                };

                // Since we call `group_added` on every group,
                new_monitor.group_added(&path.value(), &group.context);
            }

            old_monitor
        }

        #[cfg(feature = "encryption")]
        const CRYPTO_PREFIX: &'static str = "%%CONFIG-IT-SECRET%%";

        fn dump_node(ctx: &GroupContext, archive: &mut archive::Archive) {
            let paths = ctx.path.iter();
            let node = archive.find_or_create_path_mut(paths);

            // Clear existing values before dumping.
            node.values.clear();

            for (meta, val) in ctx
                .sources
                .iter()
                .map(|e| e.property_value())
                .filter(|(meta, _)| !meta.metadata.flags.contains(MetaFlag::NO_EXPORT))
            {
                let dst = node.values.entry(meta.name.into()).or_default();

                #[cfg(feature = "encryption")]
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

            '_outer: for (elem, de) in ctx
                .sources
                .iter()
                .filter(|e| !e.meta.flags.contains(MetaFlag::NO_IMPORT))
                .filter_map(|x| {
                    node.values.get(x.meta.name).map(|o| (x, o.clone().into_deserializer()))
                })
            {
                #[cfg(feature = "encryption")]
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
                        monitor.entity_value_updated(&ctx.group_id, &elem.id);
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

        /// When set to true, the imported config will be merged with the existing cache. This is typically
        /// useful to prevent unsaved archive entities from being overwritten.
        ///
        /// Default is `true`.
        merge_onto_cache: bool,

        /// If this option is enabled, the imported settings will be treated as a 'patch' before being applied.
        /// If disabled, the imported settings will directly overwrite existing ones, affecting all properties
        /// in the archive even if the archive content hasn't actually changed.
        ///
        /// Default is `true`.
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

        /// On export, the storage gathers only active instances of config groups. If this is set to true,
        /// the collected results will be merged with the existing dump cache, preserving
        /// the archive data of uninitialized config groups.
        ///
        /// If set to false, only active config groups will be exported.
        ///
        /// Default is `true`
        merge_onto_dumped: bool,

        /// When this option is true, the storage will overwrite the import cache with the exported data.
        /// This will influence the creation of the next config group.
        ///
        /// Default is `true`
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
