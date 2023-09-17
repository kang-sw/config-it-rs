use std::{
    any::{Any, TypeId},
    hash::Hash,
    mem::replace,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Weak,
    },
};

use crate::{
    archive,
    config::{self, GroupContext},
    core::{self, GroupFindError, GroupID, Monitor, PathHash},
    entity::{self, EntityEventHook},
    noti,
};
use compact_str::{CompactString, ToCompactString};

///
/// Storage manages multiple sets registered by preset key.
///
#[derive(Default, Clone)]
pub struct Storage(Arc<inner::Inner>);

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
        Self { merge_onto_cache: true, apply_as_patch: true }
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
        Self { merge_onto_dumped: true, replace_import_cache: true }
    }
}

impl Storage {
    /// Tries to find existing group from path name, and if it doesn't exist, tries to create new
    pub fn find_or_create<'a, T>(
        &self,
        path: impl IntoIterator<Item = &'a (impl AsRef<str> + ?Sized + 'a)>,
    ) -> Result<config::Group<T>, core::Error>
    where
        T: config::Template,
    {
        let keys_iter = path.into_iter().map(|x| x.as_ref().to_compact_string());
        let keys: Vec<CompactString> = keys_iter.collect();
        let path_hash = PathHash::new(keys.iter().map(|x| x.as_str()));

        todo!()
    }

    /// A shortcut for find_group_ex
    pub fn find<'a, T: config::Template>(
        &self,
        path: impl IntoIterator<Item = &'a (impl AsRef<str> + ?Sized + 'a)>,
    ) -> Result<config::Group<T>, GroupFindError> {
        let path_hash = PathHash::new(path.into_iter().map(|x| x.as_ref()));

        if let Some(group) = self.0.find_group(&path_hash) {
            if group.template_type_id != std::any::TypeId::of::<T>() {
                Err(GroupFindError::MismatchedTypeID)
            } else if let Some(anchor) = group.w_unregister_hook.upgrade() {
                Ok(config::Group::create_with__(group, anchor))
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
    ) -> Result<config::Group<T>, core::Error>
    where
        T: config::Template,
    {
        let keys_iter = path.into_iter().map(|x| x.as_ref().to_compact_string());
        let keys: Vec<CompactString> = keys_iter.collect();

        self.create_impl(keys)
    }

    fn create_impl<T: config::Template>(
        &self,
        path: Vec<CompactString>,
    ) -> Result<config::Group<T>, core::Error> {
        assert!(path.is_empty());
        assert!(path.iter().all(|x| !x.is_empty()));

        let path: Arc<[CompactString]> = path.into();
        let path_hash = PathHash::new(path.iter().map(|x| x.as_str()));

        // Naively check if there's already existing group with same path.
        if let Some(_) = self.0.find_group(&path_hash) {
            return Err(core::Error::GroupPathDuplication);
        }

        // Collect metadata
        let mut table: Vec<_> = T::prop_desc_table__().values().collect();
        table.sort_by(|a, b| a.index.cmp(&b.index));

        // This ID may not be used if group creation failed ... it's generally okay since we have
        // 2^63 trials.
        let register_id = GroupID::new_unique();
        let entity_hook = Arc::new(EntityHookImpl { register_id, inner: Arc::downgrade(&self.0) });

        let sources: Vec<_> = table
            .into_iter()
            .map(|prop| entity::EntityData::new(prop.meta.clone(), entity_hook.clone()))
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
            sources: Arc::new(sources),
            source_update_fence: AtomicUsize::new(1), // NOTE: This will trigger initial check_update() always.
            update_receiver_channel: tx_noti.receiver(true),
            path: path.clone(),
        });

        self.0
            .register_group(path_hash, context, tx_noti)
            .map(|context| config::Group::create_with__(context, unregister_anchor))
    }

    /// Dump all  configs from storage.
    ///
    /// # Arguments
    ///
    /// * `no_merge` - If true, only active archive contents will be collected.
    ///   Otherwise, result will contain merge result of previously loaded archive.
    /// * `no_update` - If true, existing archive won't
    pub fn export(&self, option: ExportOptions) -> Result<archive::Archive, core::Error> {
        todo!()
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
    pub fn import(
        &self,
        archive: archive::Archive,
        option: ImportOptions,
    ) -> Result<(), core::Error> {
        todo!()
    }

    /// Replace existing monitor to given one.
    pub fn reset_monitor(&self, handler: Arc<impl Monitor>) -> Arc<dyn Monitor> {
        replace(&mut *self.0.monitor.write(), handler)
    }

    pub fn unset_monitor(&self) {
        *self.0.monitor.write() = Arc::new(inner::EmptyMonitor);
    }

    ///
    /// Send monitor event to storage driver.
    ///
    pub fn notify_edit_events(
        &self,
        items: impl IntoIterator<Item = GroupID>,
    ) -> Result<(), core::Error> {
        todo!()
    }
}

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

        // TODO: let group = inner.get_group(self.register_id).expect("logic error");

        // group.on_value_update(data, silent);
    }
}

mod inner {
    use dashmap::DashMap;
    use parking_lot::{Mutex, RwLock};
    use serde::de::IntoDeserializer;

    use crate::{
        archive::{self},
        noti,
    };

    use super::*;

    ///
    /// Drives storage internal events.
    ///
    /// - Receives update request
    ///
    pub(super) struct Inner {
        /// List of all config sets registered in this storage.
        ///
        /// Uses group id as key.
        all_groups: DashMap<GroupID, GroupRegistration>,

        /// List of all registered monitors within this storage.
        ///
        /// On every monitor event, storage driver will iterate each session channels
        ///  and will try replication.
        pub monitor: RwLock<Arc<dyn Monitor>>,

        /// Registered path hashes. Used to quickly compare if there are any path name
        ///  duplication.
        ///
        /// Uses path hash as key, group id as value.
        path_hashes: DashMap<PathHash, GroupID>,

        /// Cached archive. May contain contents for currently non-exist groups.
        pub archive: RwLock<archive::Archive>,
    }

    pub(super) struct EmptyMonitor;
    impl Monitor for EmptyMonitor {}

    impl Default for Inner {
        fn default() -> Self {
            Self::new(Arc::new(EmptyMonitor))
        }
    }

    struct GroupRegistration {
        /// Hash calculated from `context.path`.
        path_hash: PathHash,
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
        ) -> Result<Arc<GroupContext>, core::Error> {
            // Path-hash to GroupID mappings can be collided if there's simultaneous access.
            // Therefore treat group insertion as success only when path_hash was successfully
            // registered to corresponding group ID.
            let group_id = context.group_id;
            let inserted = context.clone();

            let rg = GroupRegistration { path_hash, context, evt_on_update };

            if let Some(node) =
                self.archive.read().find_path(rg.context.path.iter().map(|x| x.as_str()))
            {
                Self::load_node(&rg.context, node, None);
            }

            assert!(
                self.all_groups.insert(group_id, rg).is_none(),
                "Group ID never be duplicated." // as long as we didn't consume all 2^64 candidates ...
            );

            let path_hash = self.path_hashes.entry(path_hash).or_insert(group_id);
            if path_hash.value() != &group_id {
                // This is corner case where path hash was registered by other thread. In this case,
                // we should remove group registration and return error.
                self.all_groups.remove(&group_id);
                return Err(core::Error::GroupPathDuplication);
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
                Self::dump_node(&ctx.context, &mut *self.archive.write());

                // Notify removal
                self.monitor.read().group_removed(&group_id);
            }
        }

        fn dump_node(ctx: &GroupContext, archive: &mut archive::Archive) {
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

        fn load_node(
            ctx: &GroupContext,
            node: &archive::Archive,
            monitor: Option<&dyn Monitor>,
        ) -> bool {
            let mut has_update = false;

            for (elem, de) in ctx
                .sources
                .iter()
                .map(|e| (e, e.get_meta()))
                .filter(|(_, m)| !m.props.disable_import)
                .filter_map(|(e, m)| {
                    node.values.get(m.name).map(|o| (e, o.clone().into_deserializer()))
                })
            {
                match elem.update_value_from(de) {
                    Ok(_) => {
                        has_update = true;

                        if let Some(monitor) = monitor {
                            monitor.entity_value_updated(&ctx.group_id, &elem.get_id());
                        }
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
