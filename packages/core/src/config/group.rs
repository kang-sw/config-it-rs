use bitfield::bitfield;
use std::any::{Any, TypeId};
use std::iter::zip;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use strseq::SharedStringSequence;

use crate::shared::GroupID;

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

///
/// May storage implement this
///
#[derive(cs::Debug)]
pub struct GroupContext {
    /// This group's instance ID
    pub group_id: GroupID,

    /// Type ID of base template. Used to verify newly creation group's validity.
    pub template_type_id: TypeId,

    /// List of sources; each element represents single property.
    pub(crate) sources: Arc<[EntityData]>,

    pub(crate) w_unregister_hook: Weak<dyn Any + Send + Sync>,
    pub(crate) version: AtomicU64,

    /// Path of instantiated config set.
    pub path: SharedStringSequence,

    /// Broadcast subscriber to receive updates from backend.
    pub(crate) update_receiver_channel: noti::Receiver,
}

pub mod monitor {
    //! Exposed APIs to control over entities
    impl super::GroupContext {}
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

    /// Fetch underlying object's updates and apply to local cache. Returns true if there was
    /// any update available.
    pub fn update(&mut self) -> bool {
        let local = self.local.as_slice_mut();

        // Forces initial update always return true.
        let mut has_update = self.version_cached == 0;

        // Perform quick check: Does update fence value changed?
        match self.origin.version.load(Ordering::Relaxed) {
            new_ver if new_ver == self.version_cached => return false,
            new_ver => self.version_cached = new_ver,
        }

        debug_assert_eq!(
            local.len(),
            self.origin.sources.len(),
            "Logic Error: set was not correctly initialized!"
        );

        for ((index, local), source) in zip(zip(0..local.len(), &mut *local), &*self.origin.sources)
        {
            // Perform quick check to see if given config entity has any update.
            match source.version() {
                // NOTE: The locally updated version uses only 63 bits of the bit variable, which
                // may occasionally cause it to differ from the source version. However, this
                // discrepancy is unlikely to be a practical issue because it would require a
                // version gap of at least 2^63 to occur, and the tolerance resets to 2^63 whenever
                // an update is made.
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

    /// Check element update from its address, and clears dirty flag on given element.
    /// This is only meaningful when followed by [`Group::update`] call.
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

    /// Get index of element based on element address.
    #[doc(hidden)]
    pub fn get_index_by_ptr<U: 'static>(&self, e: *const U) -> Option<usize> {
        debug_assert!({
            let e = e as usize;
            let base = &self.__body as *const _ as usize;
            e >= base && e < base + std::mem::size_of::<T>()
        });

        self.get_prop_by_ptr(e).map(|prop| prop.index)
    }

    /// Get property descriptor by element address. Provides primitive guarantee for type safety.
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

    /// Commit changes on element to core context, then it will be propagated to all other groups
    /// which shares same core context.
    pub fn commit_elem<U: Clone + Entity>(&self, prop: &U, notify: bool) {
        // Replace source argument with created ptr
        let elem = &(*self.origin.sources)[self.get_index_by_ptr(prop).unwrap()];

        // SAFETY: We know that `vtable.implements_copy()` is strictly managed.
        let impl_copy = elem.property_info().vtable.implements_copy();
        let new_value = unsafe { EntityValue::from_value(prop.clone(), impl_copy) };

        elem.__apply_value(new_value);
        elem.__notify_value_change(notify)
    }

    /// Notifie this element has been changed, without committing any changes to core context.
    pub fn touch_elem<U: 'static>(&self, prop: *const U) {
        let elem = &(*self.origin.sources)[self.get_index_by_ptr(prop).unwrap()];
        elem.__notify_value_change(true)
    }

    /// Clone new update receiver channel. Given channel will be notified whenever call to
    /// `update()` method meaningful. However, as the event can be generated manually even
    /// if there's no actual update, it's not recommended to make critical logics rely on
    /// this signal.
    pub fn watch_update(&self) -> WatchUpdate {
        self.origin.update_receiver_channel.clone()
    }

    /// Mark all elements dirty. Next call to [`Group::update()`] may not return true if there
    /// wasn't any actual update, however, every call to [`Group::clear_flag()`] for
    /// each elements will return true.
    pub fn mark_all_elem_dirty(&mut self) {
        // Raising dirty flag does not incur remote reload.
        self.local.as_slice_mut().iter_mut().for_each(|e| e.bits.set_dirty(1));
    }

    /// Mark this group dirty. Next call to `update()` method will return true, regardless of
    /// whether there's any actual update.
    pub fn mark_group_dirty(&mut self) {
        self.version_cached = 0;
    }

    /// Mark given element dirty.
    pub fn mark_dirty<U: 'static>(&mut self, elem: *const U) {
        let index = self.get_index_by_ptr(elem).unwrap();
        self.local.as_slice_mut()[index].bits.set_dirty(1);
    }

    /// Get generated metadata of given element
    pub fn property_info<U: 'static>(&self, elem: *const U) -> &'static PropertyInfo {
        self.get_prop_by_ptr(elem).unwrap()
    }

    /// Get instance path of `self`. This value is same as the list of tokens that you have
    /// provided to [`crate::Storage::create_group`] method.
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
