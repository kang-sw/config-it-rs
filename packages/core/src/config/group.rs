use bitfield::bitfield;
use std::any::{Any, TypeId};
use std::iter::zip;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use strseq::SharedStringSequence;

use crate::shared::GroupId;

use super::entity::{Entity, EntityData, EntityValue, PropertyInfo};
use super::noti;

///
/// Base trait that is automatically generated
///
pub trait Template: Clone + 'static {
    /// Relevant type for stack allocation of props
    type LocalPropContextArray: LocalPropContextArray;

    /// Returns table mapping to <offset_from_base:property_metadata>
    #[doc(hidden)]
    fn props__() -> &'static [PropertyInfo];

    /// Gets property at memory offset
    #[doc(hidden)]
    fn prop_at_offset__(offset: usize) -> Option<&'static PropertyInfo>;

    /// Get path of this config template (module path, struct name)
    fn template_name() -> (&'static str, &'static str);

    /// Create default configuration object
    fn default_config() -> Self;

    #[doc(hidden)]
    fn elem_at_mut__(&mut self, index: usize) -> &mut dyn Any;

    #[doc(hidden)]
    fn update_elem_at__(&mut self, index: usize, value: &dyn Any, meta: &PropertyInfo) {
        let data = self.elem_at_mut__(index);
        meta.vtable.clone_in_place(value, data);
    }
}

/* --------------------------------------- Local Property --------------------------------------- */

/// Allows local properties to be stored on stack.
#[doc(hidden)]
pub trait LocalPropContextArray: Clone + Default + std::fmt::Debug {
    const N: usize;

    fn as_slice(&self) -> &[PropLocalContext];
    fn as_slice_mut(&mut self) -> &mut [PropLocalContext];
}

#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct LocalPropContextArrayImpl<const N: usize>([PropLocalContext; N]);

impl<const N: usize> LocalPropContextArray for LocalPropContextArrayImpl<N> {
    const N: usize = N;

    fn as_slice(&self) -> &[PropLocalContext] {
        &self.0
    }

    fn as_slice_mut(&mut self) -> &mut [PropLocalContext] {
        &mut self.0
    }
}

impl<const N: usize> Default for LocalPropContextArrayImpl<N> {
    fn default() -> Self {
        Self([0; N].map(|_| PropLocalContext::default()))
    }
}

/// Represents the context associated with a configuration group in the storage system.
///
/// A `GroupContext` provides necessary information about a particular group's instantiation
/// and its connection to the underlying storage. Implementations of the storage system may
/// use this structure to manage and access configurations tied to a specific group.
#[derive(cs::Debug)]
pub struct GroupContext {
    /// A unique identifier for the configuration group instance.
    pub group_id: GroupId,

    /// The type ID of the base template from which this group was derived.
    /// Used to validate the legitimacy of groups created afresh.
    pub template_type_id: TypeId,

    /// Type name and module path of this group (cached),
    pub template_name: (&'static str, &'static str),

    /// An ordered list of data entities, each corresponding to an individual property
    /// within the configuration group.
    pub(crate) sources: Arc<[EntityData]>,

    /// A weak reference to a hook which, if set, can be triggered upon
    /// group unregistration. Useful for clean-up operations or notifications.
    pub(crate) w_unregister_hook: Weak<dyn Any + Send + Sync>,

    /// Represents the current version of the group. This may be incremented with
    /// updates, allowing for versioned access and change tracking.
    pub(crate) version: AtomicU64,

    /// The hierarchical path representing the location of this configuration set
    /// within a broader configuration system.
    pub path: SharedStringSequence,

    /// A channel for receiving update notifications from the
    /// backend, enabling the group to respond to external changes or synchronize its state.
    pub(crate) update_receiver_channel: noti::Receiver,
}

mod monitor {
    //! Exposed APIs to control over entities

    use crate::{config::noti, shared::ItemId};

    impl super::GroupContext {
        /// Finds an item with the given `item_id` in the group's sources.
        pub fn find_item(&self, item_id: ItemId) -> Option<&super::EntityData> {
            debug_assert!(
                self.sources.windows(2).all(|w| w[0].id < w[1].id),
                "Logic Error: Sources are not sorted!"
            );

            self.sources
                .binary_search_by(|x| x.id.cmp(&item_id))
                .map(|index| &self.sources[index])
                .ok()
        }

        /// Returns a channel for receiving update notifications for this group.
        pub fn watch_update(&self) -> noti::Receiver {
            self.update_receiver_channel.clone()
        }

        /// List of available entities
        pub fn entities(&self) -> &[super::EntityData] {
            &self.sources
        }
    }
}

///
/// Primary interface that end user may interact with
///
/// Wrap `ReflectData` derivative like `Group<MyData>`
///
pub struct Group<T: Template> {
    /// Cached local content
    __body: T,

    /// Cached update fence
    version_cached: u64,

    /// Property-wise contexts
    local: T::LocalPropContextArray,

    /// List of managed properties. This act as source container
    origin: Arc<GroupContext>,

    /// Unregister hook anchor.
    ///
    /// It will unregister this config set from owner storage automatically, when all
    ///  instances of config set disposed.
    _unregister_hook: Arc<dyn Any + Send + Sync>,
}

impl<T: Clone + Template> Clone for Group<T> {
    fn clone(&self) -> Self {
        Self {
            __body: self.__body.clone(),
            version_cached: self.version_cached,
            local: self.local.clone(),
            origin: self.origin.clone(),
            _unregister_hook: self._unregister_hook.clone(),
        }
    }
}

impl<T: std::fmt::Debug + Template> std::fmt::Debug for Group<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Group")
            .field("__body", &self.__body)
            .field("fence", &self.version_cached)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct PropLocalContext {
    /// Locally cached update fence.
    bits: VersionBits,
}

bitfield! {
    #[derive(Clone, Copy, PartialEq, Eq)]
    struct VersionBits(u64);
    impl Debug;

    version, set_version: 62, 0;
    is_dirty, set_dirty: 63, 63;
}

impl Default for PropLocalContext {
    fn default() -> Self {
        Self {
            bits: {
                let mut bits = VersionBits(0);
                bits.set_dirty(1);
                bits
            },
        }
    }
}

/// Type alias for broadcast receiver
pub type WatchUpdate = noti::Receiver;

impl<T: Template> Group<T> {
    #[doc(hidden)]
    pub(crate) fn create_with__(
        core: Arc<GroupContext>,
        unregister_anchor: Arc<dyn Any + Send + Sync>,
    ) -> Self {
        Self {
            origin: core,
            __body: T::default_config(),
            version_cached: 0,
            local: T::LocalPropContextArray::default(),
            _unregister_hook: unregister_anchor,
        }
    }

    /// Conveniently updates the group instance during method chaining. Especially useful when
    /// initializing a group immediately after its creation, without the need to assign it to a
    /// separate mutable variable.
    ///
    /// ```ignore
    /// // Without `updated`:
    /// let group = {
    ///     let mut group = storage.create(["my","group"]).unwrap();
    ///     group.update();
    ///     group
    /// };
    ///
    /// // With `updated`:
    /// let group = storage.create(["my","group"]).map(|x| x.updated());
    /// ```
    pub fn updated(mut self) -> Self {
        self.update();
        self
    }

    /// Fetches and applies updates from the underlying object to the local cache.
    ///
    /// This function checks if there are updates available in the source and applies them to the
    /// local cache. If any updates are found, or if this is the initial fetch (determined by the
    /// `version_cached` being 0), the function will return `true`.
    ///
    /// # Returns
    ///
    /// - `true` if updates were found and applied or if this is the initial fetch.
    /// - `false` otherwise.
    pub fn update(&mut self) -> bool {
        let local = self.local.as_slice_mut();

        // Ensures that the initial update always returns true.
        let mut has_update = self.version_cached == 0;

        // Check if the update fence value has changed.
        match self.origin.version.load(Ordering::Relaxed) {
            new_ver if new_ver == self.version_cached => return false,
            new_ver => self.version_cached = new_ver,
        }

        // Ensure the local and origin sources have the same length.
        debug_assert_eq!(
            local.len(),
            self.origin.sources.len(),
            "Logic Error: The set was not correctly initialized!"
        );

        for ((index, local), source) in zip(zip(0..local.len(), &mut *local), &*self.origin.sources)
        {
            // Check if the given config entity has any updates.
            match source.version() {
                // NOTE: The locally updated version uses 63 bits out of 64. In rare scenarios, this
                // might cause it to deviate from the source version. However, this situation is
                // unlikely to be of practical concern unless a version gap of at least 2^63 arises.
                // Moreover, the tolerance resets to 2^63 with each update.
                v if v == local.bits.version() => continue,
                v => local.bits.set_version(v),
            }

            has_update = true;
            local.bits.set_dirty(1);

            let (meta, value) = source.property_value();
            self.__body.update_elem_at__(index, value.as_any(), meta);
        }

        has_update
    }

    /// Inspects the given element for updates using its address, then resets its dirty flag. For
    /// this check to have meaningful results, it's typically followed by a [`Group::update`]
    /// invocation.
    ///
    /// # Arguments
    ///
    /// * `prop` - A raw pointer to the property to be checked.
    ///
    /// # Returns
    ///
    /// * `true` if the property was marked dirty and has been reset, `false` otherwise.
    pub fn consume_update<U: 'static>(&mut self, prop: *const U) -> bool {
        let Some(index) = self.get_index_by_ptr(prop) else { return false };
        let bits = &mut self.local.as_slice_mut()[index].bits;

        if bits.is_dirty() == 1 {
            bits.set_dirty(0);
            true
        } else {
            false
        }
    }

    #[doc(hidden)]
    pub fn get_index_by_ptr<U: 'static>(&self, e: *const U) -> Option<usize> {
        debug_assert!({
            let e = e as usize;
            let base = &self.__body as *const _ as usize;
            e >= base && e < base + std::mem::size_of::<T>()
        });

        self.get_prop_by_ptr(e).map(|prop| prop.index)
    }

    #[doc(hidden)]
    pub fn get_prop_by_ptr<U: 'static>(&self, e: *const U) -> Option<&'static PropertyInfo> {
        let ptr = e as *const u8 as isize;
        let base = &self.__body as *const _ as *const u8 as isize;

        match ptr - base {
            v if v < 0 => None,
            v if v >= std::mem::size_of::<T>() as isize => None,
            v => {
                if let Some(prop) = T::prop_at_offset__(v as usize) {
                    debug_assert_eq!(prop.type_id, TypeId::of::<U>());
                    debug_assert!(prop.index < self.local.as_slice().len());
                    Some(prop)
                } else {
                    None
                }
            }
        }
    }

    /// Commits changes made to an element, ensuring that these changes are propagated to all other
    /// groups that share the same core context.
    ///
    /// # Arguments
    ///
    /// * `prop`: The element containing the changes to be committed.
    /// * `notify`: If set to `true`, it triggers other groups that share the same context to be
    ///   notified of this change.
    pub fn commit_elem<U: Clone + Entity>(&self, prop: &U, notify: bool) {
        // Replace source argument with created pointer
        let elem = &(*self.origin.sources)[self.get_index_by_ptr(prop).unwrap()];

        // Determine if the value type supports copy operations
        let impl_copy = elem.meta.vtable.implements_copy();

        // SAFETY: We rely on the `vtable.implements_copy()` check to ensure safe data handling.
        // Proper management of this check is essential to guarantee the safety of this operation.
        let new_value = unsafe { EntityValue::from_value(prop.clone(), impl_copy) };

        // Apply the new value to the element
        elem.__apply_value(new_value);
        // Update and potentially notify other contexts of the change
        elem.touch(notify);
    }

    /// Notify changes to core context, without actual content change. This will trigger the entire
    /// notification mechanism as if the value has been changed.
    pub fn touch_elem<U: 'static>(&self, prop: *const U) {
        let elem = &(*self.origin.sources)[self.get_index_by_ptr(prop).unwrap()];
        elem.touch(true)
    }

    /// Creates a new update receiver channel. The provided channel is notified whenever an
    /// `update()` method call detects changes. However, note that the event can be manually
    /// triggered even if there are no actual updates. Therefore, relying on this signal for
    /// critical logic is not recommended.
    ///
    /// The channel will always be notified on its first `wait()` call.
    pub fn watch_update(&self) -> WatchUpdate {
        self.origin.watch_update()
    }

    /// Mark all elements dirty. Next call to [`Group::update()`] may not return true if there
    /// wasn't any actual update, however, every call to [`Group::clear_flag()`] for
    /// each elements will return true.
    pub fn mark_all_elem_dirty(&mut self) {
        // Raising dirty flag does not incur remote reload.
        self.local.as_slice_mut().iter_mut().for_each(|e| e.bits.set_dirty(1));
    }

    /// Marks the entire group as dirty. The subsequent call to the `update()` method will return
    /// `true`, irrespective of any actual underlying updates. This operation doesn't affect
    /// individual property-wise dirty flags within the group.
    pub fn mark_group_dirty(&mut self) {
        self.version_cached = 0;
    }

    /// Mark given element dirty.
    pub fn mark_dirty<U: 'static>(&mut self, elem: *const U) {
        let index = self.get_index_by_ptr(elem).unwrap();
        self.local.as_slice_mut()[index].bits.set_dirty(1);
    }

    /// Get generated metadata of given element
    pub fn meta<U: 'static>(&self, elem: *const U) -> &'static PropertyInfo {
        self.get_prop_by_ptr(elem).unwrap()
    }

    /// Retrieves the instance path of `self`. This value corresponds to the list of tokens
    /// provided during the group's creation with the [`crate::Storage::create_group`] method.
    pub fn path(&self) -> &SharedStringSequence {
        &self.origin.path
    }
}

impl<T: Template> std::ops::Deref for Group<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.__body
    }
}

impl<T: Template> std::ops::DerefMut for Group<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.__body
    }
}

#[test]
fn _verify_send_impl() {
    #[derive(Clone, Default)]
    struct Example {}
    impl Template for Example {
        type LocalPropContextArray = LocalPropContextArrayImpl<0>;

        fn prop_at_offset__(_offset: usize) -> Option<&'static PropertyInfo> {
            unimplemented!()
        }

        fn props__() -> &'static [PropertyInfo] {
            unimplemented!()
        }

        fn elem_at_mut__(&mut self, _: usize) -> &mut dyn Any {
            unimplemented!()
        }

        fn template_name() -> (&'static str, &'static str) {
            unimplemented!()
        }

        fn default_config() -> Self {
            Self::default()
        }
    }

    fn _assert_send<T: Send + Sync>() {}
    _assert_send::<Group<Example>>();
}

impl<T: Template> Group<T> {
    #[doc(hidden)]
    pub fn __macro_as_mut(&mut self) -> &mut Self {
        //! Use coercion to get mutable reference to self regardless of its expression.
        self
    }
}
