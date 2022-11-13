//!
//!
//! # Perfkit configs frontend
//!
//! Any code interacts with perfkit configs library should utilize below functionalities.
//!
//! Firstly user define a config class which derives `ConfigBody` trait class.
//! > **Warning** `ConfigBody` intended to be used via attribute, not manually implementing
//! > the trait itself.
//!
//! ``` ignore
//! use perfkit_config::macro::ConfigBody
//! #[derive(ConfigBody)]
//! struct MyConfig {
//!   /// This docstring will be saved as entity's description string.
//!   ///
//!   /// Any field marked with `perfkit` attribute will be treated as config entity.
//!   #[perfkit(default=1.21, min=0, max=100)]
//!   my_double: f64,
//!
//!   /// Any non-perfkit decorated field is ignored by macro parser
//!   transient_field: f32,
//!
//!   /// There are number of useful traits ... See `ConfigBody` docs.
//!   #[perfkit(one_of(3, 4, 5, 6, 11), min=3, max=2)]
//!   my_int: i32,
//! }
//! ```
//!
//! Any data type inherits `Send + Sync + Clone + DeserializeOwned + Serialize` can be field of
//!  config body structure. However, since a configuration entity is updated at once always,
//!  frequent update of *too big* entity structure instance may incur performance issue.
//!
use std::any::{Any, TypeId};
use std::borrow::BorrowMut;
use std::cell::{BorrowMutError, RefCell};
use crate::__all::*;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::Ordering;


///
/// Config set reflection implementation for user defined config struct
/// will automatically be generated using macro.
///
pub trait ConfigSetReflection {
    fn __get_offset_idx_table(&self) -> Arc<OffsetPropTable>;
    fn __get_metadata_vec(&self) -> Vec<Arc<Metadata>>;
    fn __entity_update_value(&mut self, index: usize, value: &dyn EntityValue);
}

pub struct PropDesc {
    index: usize,
    type_id: TypeId,
}

impl From<(usize, TypeId)> for PropDesc {
    fn from(e: (usize, TypeId)) -> Self {
        Self {
            index: e.0,
            type_id: e.1,
        }
    }
}

pub type OffsetPropTable = HashMap<usize, PropDesc>;

///
/// Types of available errors
///
#[derive(Debug)]
pub enum ConfigSetError {
    /// Update may fail if
    AlreadyBorrowed(BorrowMutError),

    /// Prefix may not contain any
    InvalidPrefixFormat,

    /// Given prefix string is duplicated
    PrefixDuplicated,

    ///
    InvalidMemberPointer { offset: isize, type_id: TypeId },
}

///
///
/// User will use defined config class as template parameter of `EntityCollection`
///
#[derive(Clone)]
pub struct ConfigSet<T> {
    /// Context associated with storage
    context: Arc<ConfigSetContext>,

    /// Path to owning storage
    storage: Storage,

    /// Provides pointer offset -> index mapping
    offset_index_table: Arc<OffsetPropTable>,

    /// Used for filtering update targets. Only entities that has larger fence number
    /// than this can be updated. After update, this number will be set as largest value
    /// of all updated entities.
    update_fence: usize,

    /// Used for checking fence values of properties
    prop_check_fences: RefCell<Vec<usize>>,

    /// Anchor automatic unregister
    anchor: Arc<()>,

    /// Actual user data structure
    body: T,
}

impl<T: ConfigSetReflection + Default> ConfigSet<T> {
    ///
    /// TODO: Create collection with storage
    ///
    pub fn new(s: Storage, prefix: &[&str]) -> Result<Self, ConfigSetError> {
        // 0. Validate parameter
        if prefix.iter().any(|v| v.is_empty()) {
            return Err(ConfigSetError::InvalidPrefixFormat);
        }

        // 1. Create default T, retrieve offset-index mapping
        let body = T::default();
        let offset_index_table = body.__get_offset_idx_table();
        let metadata = body.__get_metadata_vec();
        let num_elems = metadata.len();

        // 2. Register entities to storage.
        let context = match s.__register(prefix, metadata.as_slice()) {
            Some(retval) => retval,
            None => return Err(ConfigSetError::PrefixDuplicated),
        };

        Ok(Self {
            anchor: Arc::new(()),
            storage: s,
            update_fence: 0,
            prop_check_fences: RefCell::new(vec![0; num_elems]),
            offset_index_table,
            context,
            body,
        })
    }

    ///
    /// Update enclosing contents iteratively.
    ///
    pub fn update(&mut self) -> bool {
        // 0. Storage update is performed in background continuously. (By replacing
        //     ConfigEntityBase's value pointer & update fence). From this operation,
        //     config storage's update fence value will be updated either.

        // 1. Compare update fence value in storage level, which performs early drop ...
        if self.context.check_update(&mut self.update_fence) {
            return false;
        }

        // 2. Borrow updates
        let mut fences = self.prop_check_fences.borrow_mut();
        let body = &mut self.body;

        debug_assert!(fences.len() == self.offset_index_table.len(), "Automation code was not correctly generated!");

        // 3. Iterate each config entities, collect all updates, and apply changes to local cache.
        let mut has_update = false;
        let iter = (0..fences.len()).zip(&self.context.entities).zip(fences.iter_mut());
        for ((index, config_base), fence_val) in iter {
            let target_fence = config_base.fence.load(Ordering::Relaxed);
            if target_fence == *fence_val {
                continue;
            }

            *fence_val = target_fence;
            let value = config_base.get_cached_data();

            body.__entity_update_value(index, &*value);
            has_update = true;
        }

        has_update
    }

    ///
    /// Check if given element has any update
    ///
    pub fn check_update<U: ?Sized + Any>(&self, elem: &U) -> bool {
        let index = self._index_of(elem).unwrap().index;
        let mut fences = self.prop_check_fences.borrow_mut();

        match (self.context.entities[index].fence.load(Ordering::Relaxed), &mut fences[index]) {
            (src, dst) if src != *dst => {
                *dst = src;
                true
            }
            _ => false
        }
    }

    ///
    /// Commits updated value to owning entity base.
    ///
    pub fn commit<U: ?Sized + Any>(&self, _elem: &U) -> bool { todo!() }

    ///
    /// Commits updated value to owning entity base.
    ///
    /// This will not touch host storage's update fence value,
    ///
    pub fn commit_silent<U: ?Sized + Any>(&self, _elem: &U) -> bool { todo!() }

    ///
    /// Get reference to owning storage
    ///
    pub fn storage(&self) -> &Storage { &self.storage }

    ///
    /// Get index range. Use this with StorageEvent::RemoteUpdate
    ///
    pub fn check_props_in_range(&self, reg_id: &[usize]) -> bool {
        let min = self.context.alloc_offset_id;
        let max = min + self.offset_index_table.len();

        match reg_id.binary_search(&min) {
            Ok(_) => true,
            Err(v) if v < reg_id.len() && reg_id[v] < max => true,
            _ => false
        }
    }

    ///
    /// Check if storage event affects to this config set.
    ///
    pub fn check_affect_this(&self, event: &StorageEvent) -> bool {
        use StorageEvent::*;
        match event {
            RemoteUpdate(_, arg) => self.check_props_in_range(arg),
            Import => true,
            Export => false
        }
    }

    ///
    /// Gets index of given element
    ///
    fn _index_of<U: ?Sized + Any>(&self, elem: &U) -> Result<&PropDesc, ConfigSetError> {
        let offset = unsafe {
            let elem = elem as *const U as *const u8;
            let base = &self.body as *const T as *const u8;

            elem.offset_from(base)
        };

        let type_id = TypeId::of::<U>();
        if offset < 0 {
            return Err(ConfigSetError::InvalidMemberPointer { offset, type_id });
        }

        match self.offset_index_table.get(&(offset as usize)) {
            Some(node) if node.type_id == type_id => Ok(node),
            _ => Err(ConfigSetError::InvalidMemberPointer { offset, type_id })
        }
    }
}

impl<T> Drop for ConfigSet<T> {
    ///
    /// Unregisters config set if this was the last active config set.
    ///
    fn drop(&mut self) {
        if 1 == Arc::strong_count(&self.anchor) {
            self.storage.__unregister(self.context.clone());
        }
    }
}

impl<T> Deref for ConfigSet<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.body
    }
}

impl<T> DerefMut for ConfigSet<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.body
    }
}

#[cfg(test)]
mod test_concepts {
    use once_cell::sync::{OnceCell};

    use super::*;

    struct MyTestConfigSet {
        my_ivar: i32,
        my_fvar: f32,
        my_dvar: f64,
    }

    impl ConfigSetReflection for MyTestConfigSet {
        fn __get_offset_idx_table(&self) -> Arc<OffsetPropTable> {
            static LAZY_TABLE: OnceCell<Arc<OffsetPropTable>> = OnceCell::new();
            return LAZY_TABLE
                .get_or_init(|| {
                    let mut table = <OffsetPropTable>::with_capacity(3);

                    // Defines conversion from pointer to
                    unsafe {
                        let v = 0 as *const MyTestConfigSet;
                        table.insert(&(*v).my_ivar as *const i32 as usize, (0, TypeId::of::<i32>()).into());
                        table.insert(&(*v).my_fvar as *const f32 as usize, (0, TypeId::of::<f32>()).into());
                        table.insert(&(*v).my_dvar as *const f64 as usize, (0, TypeId::of::<f64>()).into());
                    }

                    Arc::new(table)
                })
                .clone();
        }

        fn __get_metadata_vec(&self) -> Vec<Arc<Metadata>> {
            static LAZY_TABLE: OnceCell<Vec<Arc<Metadata>>> = OnceCell::new();
            return LAZY_TABLE
                .get_or_init(|| {
                    let mut table = Vec::<Arc<Metadata>>::with_capacity(3);
                    let default = MyTestConfigSet::default();

                    let mut meta = Metadata::create_base::<i32>("my_ivar".into(), default.my_ivar);
                    meta.description = "hello".into();
                    meta.hidden = false;
                    table.push(Arc::new(meta));

                    table.push(Arc::new(Metadata::create_base::<f32>("my_fvar".into(), default.my_fvar)));
                    // ... Manipulate with these ...

                    table.push(Arc::new(Metadata::create_base::<f64>("my_dvar".into(), default.my_dvar)));
                    table
                })
                .clone();
        }

        fn __entity_update_value(&mut self, index: usize, value: &dyn EntityValue) {
            match index {
                0 => value.clone_to(&mut self.my_ivar),
                1 => value.clone_to(&mut self.my_fvar),
                2 => value.clone_to(&mut self.my_dvar),
                _ => unimplemented!(),
            }
        }
    }

    impl Default for MyTestConfigSet {
        fn default() -> Self {
            //
            // Default::default() will be replaced if any other default
            //  value was specified via macro attribute
            //
            Self {
                my_ivar: Default::default(),
                my_fvar: Default::default(),
                my_dvar: Default::default(),
            }
        }
    }

    #[test]
    fn test_if_compiled() {
        unsafe {
            let v = 0 as *const MyTestConfigSet;
            let addr_1 = &(*v).my_dvar as *const f64 as *const u8;
            let addr_1 = addr_1 as usize;

            println!("addr_1: {addr_1}")
        }

        let storage = Storage::new(Registry::new(), "MyRg".into());

        // Create configs with different prefix
        let mut cfg1 = ConfigSet::<MyTestConfigSet>::new(storage.clone(), &[]).unwrap();
        let mut cfg2 = ConfigSet::<MyTestConfigSet>::new(storage, &["My", "Little", "Pony"]).unwrap();

        assert_eq!(true, cfg1.check_update(&cfg1.my_ivar));
        assert_eq!(false, cfg1.check_update(&cfg1.my_ivar));
        cfg1.my_ivar = 2;
    }

    #[derive(Default)]
    struct Inner;

    #[derive(Default)]
    struct Outer {
        inner: Inner,
    }

    impl Drop for Inner {
        fn drop(&mut self) {
            println!("Dropping Inner!");
        }
    }

    impl Drop for Outer {
        fn drop(&mut self) {
            println!("Dropping Outer!");
        }
    }

    #[test]
    #[ignore]
    fn test_pseudo_automation() {
        let s = Outer::default();
    }
}
