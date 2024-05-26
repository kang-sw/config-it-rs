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
    fn group_added(&mut self, group_id: GroupID, group: &Arc<GroupContext>) {
        let _ = (group_id, group);
    }

    /// Can be call with falsy group_id if monitor is being replaced.
    fn group_removed(&mut self, group_id: GroupID) {
        let _ = group_id;
    }

    /// Called when any entity value is updated. Can be called with falsy group_id if monitor is
    /// being replaced.
    ///
    /// Since this is called frequently compared to group modification commands, receives immutable
    /// self reference. Therefore, all state modification should be handled with interior
    /// mutability!
    fn entity_value_updated(&self, group_id: GroupID, item_id: ItemID) {
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
    /// Gets ID of this storage instance. ID is unique per single program instance.
    pub fn storage_id(&self) -> crate::shared::StorageID {
        self.0.id
    }

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
    pub fn find<T: group::Template>(
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
        if self.0.find_group(&path_hash).is_some() {
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
            template_name: T::template_name(),
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
    pub fn replace_monitor(&self, handler: Box<impl Monitor>) -> Box<dyn Monitor> {
        self.0.replace_monitor(handler)
    }

    /// Unset monitor instance.
    pub fn unset_monitor(&self) {
        *self.0.monitor.write() = Box::new(inner::EmptyMonitor);
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
    #[cfg(feature = "crypt")]
    pub fn set_crypt_key(&self, key: impl AsRef<[u8]>) {
        use sha2::{Digest, Sha256};
        let arr = &*Sha256::new().chain_update(key).finalize();
        self.0.crypt_key.write().replace(std::array::from_fn(|index| arr[index]));
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
    use std::{collections::HashMap, mem::ManuallyDrop};

    use derive_setters::Setters;
    use parking_lot::RwLock;

    use crate::{
        config::entity::Entity,
        shared::{archive::Archive, meta::MetaFlag, StorageID},
    };

    use super::*;

    /// Manages and drives internal storage events.
    ///
    /// Primarily responsible for handling update requests and orchestrating
    /// the underlying storage mechanisms.
    #[derive(cs::Debug)]
    pub(super) struct Inner {
        /// Unique(during runtime) identifier for this storage.
        pub id: StorageID,

        /// Maintains a registry of all configuration sets within this storage.
        ///
        /// The key is the group's unique identifier, `GroupID`.
        all_groups: RwLock<HashMap<GroupID, GroupRegistration>>,

        /// Maintains a list of all monitors registered to this storage.
        ///
        /// Upon each monitoring event, the storage driver iterates over each session channel to
        /// attempt replication. This ensures that all components are kept in sync with storage
        /// changes.
        #[debug(with = "fmt_monitor")]
        pub monitor: RwLock<Box<dyn Monitor>>,

        /// Keeps track of registered path hashes to quickly identify potential path name
        /// duplications.
        ///
        /// Uses the path's hash representation as the key and its corresponding `GroupID` as the
        /// value.
        path_hashes: RwLock<HashMap<PathHash, GroupID>>,

        /// Holds a cached version of the archive. This may include content for groups that are
        /// currently non-existent.
        pub archive: RwLock<archive::Archive>,

        /// AES-256 encryption key for securing data.
        ///
        /// This key is used when the encryption feature is enabled. It ensures that stored data is
        /// encrypted, adding an additional layer of security.
        #[cfg(feature = "crypt")]
        #[debug(with = "fmt_encryption_key")]
        pub crypt_key: RwLock<Option<[u8; 32]>>,
    }

    fn fmt_monitor(
        monitor: &RwLock<Box<dyn Monitor>>,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        let ptr = &**monitor.read() as *const _;

        // Extract data pointers of `ptr`, and `&EmptyMonitor`
        let exists = ptr as *const () != &EmptyMonitor as *const _ as *const ();

        write!(f, "{:?}", exists.then_some(ptr))
    }

    #[cfg(feature = "crypt")]
    fn fmt_encryption_key(
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
            Self::new(Box::new(EmptyMonitor))
        }
    }

    #[derive(Debug)]
    struct GroupRegistration {
        context: Arc<GroupContext>,
        evt_on_update: noti::Sender,
    }

    impl Inner {
        pub fn new(monitor: Box<dyn Monitor>) -> Self {
            Self {
                id: StorageID::new_unique_incremental(),
                monitor: RwLock::new(monitor),
                archive: Default::default(),
                #[cfg(feature = "crypt")]
                crypt_key: Default::default(),

                // NOTE: Uses 4 shards for both maps. The default implementation's shared amount,
                all_groups: Default::default(),
                path_hashes: Default::default(),
            }
        }

        pub fn notify_edition(&self, group_id: GroupID) {
            if let Some(group) = self.all_groups.read().get(&group_id) {
                group.context.version.fetch_add(1, Ordering::Relaxed);
                group.evt_on_update.notify();
            }
        }

        pub fn find_group(&self, path_hash: &PathHash) -> Option<Arc<GroupContext>> {
            self.path_hashes
                .read()
                .get(path_hash)
                .and_then(|id| self.all_groups.read().get(id).map(|x| x.context.clone()))
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
                Self::load_node(
                    &rg.context,
                    node,
                    &EmptyMonitor,
                    #[cfg(feature = "crypt")]
                    Self::crypt_key_loader(&self.crypt_key),
                );
            }

            // Ensure that the Group ID is unique.
            assert!(
                self.all_groups.write().insert(group_id, rg).is_none(),
                "Group IDs must be unique." // Ensure we haven't exhausted all 2^64 possibilities.
            );

            // Check for path-hash collisions. In the rare case where a collision occurs due to
            // another thread registering the same path-hash, we remove the current group
            // registration and return an error.
            if self.path_hashes.write().entry(path_hash).or_insert(group_id) != &group_id {
                self.all_groups.write().remove(&group_id);
                return Err(GroupCreationError::PathCollisionRace(inserted.path.clone()));
            }

            // Notify the monitor that a new group has been added.
            self.monitor.write().group_added(group_id, &inserted);
            Ok(inserted)
        }

        pub fn unregister_group(&self, group_id: GroupID, path_hash: PathHash) {
            {
                let mut path_hashes = self.path_hashes.write();
                if !path_hashes.get(&path_hash).is_some_and(|v| *v == group_id) {
                    tr::debug!(?group_id, ?path_hash, "unregister_group() call to unexist group");
                    return;
                };

                path_hashes.remove(&path_hash);
            }

            if let Some(ctx) = self.all_groups.write().remove(&group_id) {
                let _s = tr::info_span!(
                    "unregister_group()",
                    template = ?ctx.context.template_name,
                    path = ?ctx.context.path
                );

                // Consider the removal valid only if the group was previously and validly created.
                // The method `all_groups.remove` might return `None` if the group was not
                // successfully registered during `register_group`. If this happens, this function
                // gets invoked during the disposal of `GroupUnregisterHook` within the
                // `create_impl` function.

                // For valid removals, add contents to the cached archive.
                Self::dump_node(
                    &ctx.context,
                    &mut self.archive.write(),
                    #[cfg(feature = "crypt")]
                    Self::crypt_key_loader(&self.crypt_key),
                );

                // Notify about the removal
                self.monitor.write().group_removed(group_id);
            }
        }

        pub fn on_value_update(&self, group_id: GroupID, data: &entity::EntityData, silent: bool) {
            // Monitor should always be notified on value update, regardless of silent flag
            self.monitor.read().entity_value_updated(group_id, data.id);

            // If silent flag is set, skip internal notify to other instances.
            if silent {
                return;
            }

            // This is trivially fallible operation.
            if let Some(group) = self.all_groups.read().get(&group_id) {
                group.context.version.fetch_add(1, Ordering::Relaxed);
                group.evt_on_update.notify();
            }
        }

        pub fn replace_monitor(&self, new_monitor: Box<dyn Monitor>) -> Box<dyn Monitor> {
            // At the start of the operation, we replace the monitor. This means that before we add all
            // valid groups to the new monitor, there may be notifications about group removals or updates.
            // Without redesigning the entire storage system into a more expensive locking mechanism,
            // there's no way to avoid this. We assume that monitor implementation providers will handle
            // any incorrect updates gracefully and thus, we are ignoring this case.

            let old_monitor = replace(&mut *self.monitor.write(), new_monitor);

            // NOTE: Ensuring thread-safe behavior during the initialization of a new monitor:
            // - Iteration partially locks `path_hashes` based on shards (see DashMap
            //   implementation).
            // - This means that new group insertions or removals can occur during iteration.
            // - However, it's guaranteed that while iterating over a shard, no other thread can
            //   modify the same shard of `path_hashes`.
            // - Since every group insertion or removal first modifies `path_hashes`, it's safe to
            //   assume we see a consistent state of `path_hashes` during shard iteration, given
            //   we're observing the read-locked state of that shard.
            for group_id in self.path_hashes.read().values() {
                let all_groups = self.all_groups.read();
                let Some(group) = all_groups.get(group_id) else {
                    unreachable!(
                        "As long as the group_id is found from path_hashes, \
                        the group must be found from `all_groups`."
                    )
                };

                // Since we call `group_added` on every group,
                self.monitor.write().group_added(*group_id, &group.context);
            }

            old_monitor
        }

        /// ⚠️ **CAUTION!** Do NOT alter this literal! Any modification will DESTROY all existing
        /// encrypted data irreparably! ⚠️
        ///
        /// PREFIX itself is valid base64, which is decorated word 'secret'.
        #[cfg(feature = "crypt")]
        const CRYPT_PREFIX: &'static str = "+/+sE/cRE+t//";

        #[cfg(feature = "crypt")]
        fn crypt_key_loader(
            key: &RwLock<Option<[u8; 32]>>,
        ) -> impl Fn() -> Option<[u8; 32]> + '_ + Copy {
            || key.read().or_else(Self::crypt_sys_key)
        }

        /// Uses hard coded NONCE, just to run the algorithm.
        #[cfg(feature = "crypt")]
        const CRYPT_NONCE: [u8; 12] = [15, 43, 5, 12, 6, 66, 126, 231, 141, 18, 33, 71];

        #[cfg(feature = "crypt")]
        fn crypt_sys_key() -> Option<[u8; 32]> {
            #[cfg(feature = "crypt-machine-id")]
            {
                use std::sync::OnceLock;
                static CACHED: OnceLock<Option<[u8; 32]>> = OnceLock::new();

                *CACHED.get_or_init(|| {
                    machine_uid::get().ok().map(|uid| {
                        use sha2::{Digest, Sha256};
                        let arr = &*Sha256::new().chain_update(uid).finalize();
                        std::array::from_fn(|index| arr[index])
                    })
                })
            }

            #[cfg(not(feature = "crypt-machine-id"))]
            {
                None
            }
        }

        fn dump_node(
            ctx: &GroupContext,
            archive: &mut archive::Archive,
            #[cfg(feature = "crypt")] crypt_key_loader: impl Fn() -> Option<[u8; 32]>,
        ) {
            let _s = tr::info_span!("dump_node()", template=?ctx.template_name, path=?ctx.path);

            let paths = ctx.path.iter();
            let node = archive.find_or_create_path_mut(paths);

            // Clear existing values before dumping.
            node.values.clear();

            #[cfg(feature = "crypt")]
            let mut crypt_key = None;

            '_outer: for (meta, val) in ctx
                .sources
                .iter()
                .map(|e| e.property_value())
                .filter(|(meta, _)| !meta.metadata.flags.contains(MetaFlag::NO_EXPORT))
            {
                let _s = tr::info_span!("node dump", varname=?meta.varname);
                let dst = node.values.entry(meta.name.into()).or_default();

                #[cfg(feature = "crypt")]
                'encryption: {
                    use aes_gcm::aead::{Aead, KeyInit};
                    use base64::prelude::*;

                    if !meta.metadata.flags.contains(MetaFlag::SECRET) {
                        break 'encryption;
                    }

                    if crypt_key.is_none() {
                        crypt_key = Some(crypt_key_loader().ok_or(()));
                    }

                    // Check if key was correctly loaded. If not, skip serialization itself to not
                    // export delicate data.
                    let Ok(key) = crypt_key.as_ref().unwrap() else {
                        tr::warn!("Crypt key missing. Skipping secret data serialization.");
                        continue '_outer;
                    };
                    let Ok(json) = serde_json::to_vec(val.as_serialize()) else {
                        tr::warn!("JSON dump failed");
                        continue '_outer;
                    };

                    let cipher = aes_gcm::Aes256Gcm::new(key.into());
                    let Ok(enc) = cipher.encrypt(&Self::CRYPT_NONCE.into(), &json[..]) else {
                        tr::warn!("Encryption failed");
                        continue '_outer;
                    };

                    *dst = serde_json::Value::String(format!(
                        "{}{}",
                        Self::CRYPT_PREFIX,
                        BASE64_STANDARD_NO_PAD.encode(&enc)
                    ));

                    continue '_outer;
                }

                #[cfg(not(feature = "crypt"))]
                if meta.metadata.flags.contains(MetaFlag::SECRET) {
                    tr::warn!("`crypt` Feature disabled: Skipping secret data serialization.");
                    continue;
                }

                match serde_json::to_value(val.as_serialize()) {
                    Ok(val) => *dst = val,
                    Err(error) => {
                        tr::warn!(%error, "JSON dump failed");
                    }
                }
            }
        }

        fn load_node(
            ctx: &GroupContext,
            node: &archive::Archive,
            monitor: &dyn Monitor,
            #[cfg(feature = "crypt")] crypt_key_loader: impl Fn() -> Option<[u8; 32]>,
        ) -> bool {
            let _s = tr::info_span!("load_node()", template=?ctx.template_name, path=?ctx.path);
            let mut has_update = false;

            #[cfg(feature = "crypt")]
            let mut crypt_key = None;

            '_outer: for (elem, de) in ctx
                .sources
                .iter()
                .filter(|e| !e.meta.flags.contains(MetaFlag::NO_IMPORT))
                .filter_map(|x| node.values.get(x.meta.name).map(|o| (x, o)))
            {
                let _s = tr::info_span!("node load", varname=?elem.meta.varname);

                #[allow(unused_mut)]
                let mut update_result = None;

                #[cfg(feature = "crypt")]
                'decryption: {
                    use aes_gcm::aead::{Aead, KeyInit};
                    use base64::prelude::*;

                    if !elem.meta.flags.contains(MetaFlag::SECRET) {
                        // Just try to deserialize from plain value.
                        break 'decryption;
                    }

                    // Non-string value is not an encrypted property. serve as-is.
                    let Some(str) = de.as_str() else { break 'decryption };

                    // Verify if it is encrpyted string repr.
                    if !str.starts_with(Self::CRYPT_PREFIX) {
                        tr::debug!("Non-encrypted string repr. serve as-is.");
                        break 'decryption;
                    }

                    let str = &str[Self::CRYPT_PREFIX.len()..];
                    let Ok(bin) = BASE64_STANDARD_NO_PAD.decode(str).map_err(|error| {
                        tr::debug!(
                            %error,
                            "Crypt-prefixed string is not valid base64. \
                             Trying to parse as plain string."
                        )
                    }) else {
                        break 'decryption;
                    };

                    if crypt_key.is_none() {
                        crypt_key = Some(crypt_key_loader().ok_or(()));
                    }

                    // Check if key was correctly loaded. If not, just try to parse string as plain
                    // string ... which woun't be very useful though.
                    let Ok(key) = crypt_key.as_ref().unwrap() else {
                        tr::warn!("Crypt key missing. Skipping secret data serialization.");
                        break 'decryption;
                    };

                    let cipher = aes_gcm::Aes256Gcm::new(key.into());
                    let Ok(json) =
                        cipher.decrypt(&Self::CRYPT_NONCE.into(), &bin[..]).map_err(|error| {
                            tr::warn!(%error, "Failed to decrypt secret data");
                        })
                    else {
                        break 'decryption;
                    };

                    update_result = Some(
                        elem.update_value_from(&mut serde_json::Deserializer::from_slice(&json)),
                    );
                }

                match update_result.unwrap_or_else(|| elem.update_value_from(de)) {
                    Ok(_) => {
                        has_update = true;
                        monitor.entity_value_updated(ctx.group_id, elem.id);
                    }
                    Err(error) => {
                        tr::warn!(%error, "Element value update error during node loading")
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

            #[cfg(feature = "crypt")]
            let key_loader = Inner::crypt_key_loader(&this.crypt_key);

            let import_archive = |archive: &Archive| {
                for group in this.all_groups.read().values() {
                    let path = &group.context.path;
                    let path = path.iter();
                    let Some(node) = archive.find_path(path) else { continue };

                    if Inner::load_node(
                        &group.context,
                        node,
                        &**this.monitor.read(),
                        #[cfg(feature = "crypt")]
                        key_loader,
                    ) {
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

            #[cfg(feature = "crypt")]
            let key_loader = Inner::crypt_key_loader(&this.crypt_key);

            for group in this.all_groups.read().values() {
                Inner::dump_node(
                    &group.context,
                    &mut archive,
                    #[cfg(feature = "crypt")]
                    key_loader,
                );
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

#[cfg(feature = "arc-swap")]
pub mod atomic {
    use arc_swap::ArcSwap;

    use super::Storage;

    pub struct AtomicStorageArc(ArcSwap<super::inner::Inner>);

    impl From<Storage> for AtomicStorageArc {
        fn from(value: Storage) -> Self {
            Self(ArcSwap::new(value.0))
        }
    }

    impl Default for AtomicStorageArc {
        fn default() -> Self {
            Self::from(Storage::default())
        }
    }

    impl AtomicStorageArc {
        pub fn load(&self) -> Storage {
            Storage(self.0.load_full())
        }

        pub fn store(&self, storage: Storage) {
            self.0.store(storage.0)
        }
    }
}
