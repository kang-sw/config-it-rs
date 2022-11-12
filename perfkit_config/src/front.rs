//!
//! 1. Config Entity
//! 2. Config Set -> NOT a physical concept ... just user-defined struct of various entities!
//! 3. Config Storage -> Collection of entities. Classes are set of entities with prefix.
//! 4. Config Registry -> Collection of categories. File save/load root
//!
//! Config Entity    --> ConfigBase:1, Data:1:mut
//! Config Set       --> ConfigStorage:1, ConfigEntity:N
//! Config Storage   --> w:ConfigBase:N
//! Config Base      --> Data:1:mut
//!
//! Targets following behavior:
//!
//! ``` ignore
//! #[perkfit_config_set]
//! struct MySet {
//!   /// Doc comment will be embedded into metadata as entity description
//!   #[min(3), max(5)]
//!   element: i32, // Turns into perfkit_config::Entity<i32>
//!
//!   /// Serde serializable/deserializable user defined structs are allowed
//!   ///  to be used as config set component
//!   my_data: UserType,
//!
//!   /// one_of attribute
//! }
//!
//! #[derive(Serialize,Deserialize)]
//! struct UserType {
//!   ...
//! }
//!
//! ... // Frontend
//! let reg = Registry::new();
//! reg.load(load_json_from_file("file_name.json"), LoadPolicy::);
//!
//! let back = perfkit_backend_web::Backend::create(
//!   perfkit_backend_web::BackendDesc {
//!     config_registry: reg.clone(),
//!     trace_registry: ...,
//!     command_registry: ...,
//!     binding: (None, parse_env_or<u16>("PF_BK_PORT", 15572)),
//!     ...
//!   }
//! );
//!
//! // Create storage
//! let storage : Arc<Storage> = Storage::find_or_create(reg, "storage_name");
//!
//! // Create config set(user defined), by registering given storage.
//! let cfg = MySet::create(&storage, ["prefix", "path", "goes", "here"]).expect("Config set with same prefix is not allowed!");
//!
//! //
//! cfg.update(); // Searches storage, fetch all updates
//! if cfg.element.take_update_flag() {
//!   // Do some heavy work with this ...
//! }
//! ... // Backend
//!
//! ```
use std::any::{Any, TypeId};
use std::cell::{BorrowMutError, Ref, RefCell, RefMut};
use crate::__all::*;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use crate::__all::ConfigSetError::AlreadyBorrowed;

///
/// Config set reflection implementation for user defined config struct
/// will automatically be generated using macro.
///
pub trait ConfigSetReflection {
    fn __get_offset_idx_table(&self) -> Arc<OffsetPropTable>;
    fn __entity_get_meta(&self) -> Arc<Vec<Metadata>>;
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

    ///
    InvalidMemberPointer { offset: isize, type_id: TypeId },
}

///
/// Alias to result around config set operation
///

///
///
/// User will use defined config class as template parameter of `EntityCollection`
///
pub struct ConfigSet<T> {
    /// Description for this collection instance, which will be shared across
    /// collection instances.
    desc: Arc<CollectionDescriptor>,

    /// Used for filtering update targets. Only entities that has larger fence number
    /// than this can be updated. After update, this number will be set as largest value
    /// of all updated entities.
    update_fence: usize,

    /// Used for checking fence values of properties
    prop_check_fences: RefCell<Vec<usize>>,

    ///
    body: RefCell<T>,
}


struct CollectionDescriptor {
    /// Path to owning storage
    storage: Storage,

    /// Provides pointer offset -> index mapping
    offset_index_table: Arc<OffsetPropTable>,

    /// Each field of config set's target struct will be mapped into this.
    config_base_set: Vec<Arc<EntityBase>>,
}


impl<T: ConfigSetReflection + Default> ConfigSet<T> {
    ///
    /// TODO: Create collection with storage
    ///
    pub fn new(s: Storage, prefix: impl Iterator<Item=&'static str>) -> Result<Self, ConfigSetError> {
        // 1. Create default T, retrieve offset-index mapping
        let body = T::default();
        let offset_index_table = body.__get_offset_idx_table();

        // 2. Create basic entities and collect metadata for them.
        todo!();

        // 3. Register entities to storage.
        todo!();


        todo!()
    }

    ///
    /// TODO: Update enclosing contents iteratively.
    ///
    pub fn update(&self) -> Result<bool, ConfigSetError> {
        // 0. Storage update is performed in background continuously. (By replacing
        //     ConfigEntityBase's value pointer & update fence). From this operation,
        //     config storage's update fence value will be updated either.

        // 1. Compare update fence value in storage level, which performs early drop ...
        if self.desc.storage.update_fence() == self.update_fence {
            return Ok(false);
        }

        // 2.
        let mut fences = self.prop_check_fences.borrow_mut();
        let mut body = self.body.try_borrow_mut();
        if body.is_err() {
            return Err(AlreadyBorrowed(body.err().unwrap()));
        }

        debug_assert!(fences.len() == self.desc.config_base_set.len(), "Automation code was not correctly generated!");
        let mut body = body.unwrap();

        // 3. Iterate each config entities, collect all updates, and apply changes to local cache.
        let iter = (0..fences.len()).zip(&self.desc.config_base_set).zip(fences.iter_mut());
        for ((index, config_base), fence_val) in iter {
            let target_fence = config_base.fence.load(Ordering::Relaxed);
            if target_fence == *fence_val {
                continue;
            }

            *fence_val = target_fence;
            let value = config_base.get_cached_data();

            body.__entity_update_value(index, &*value);
        }

        Ok(true)
    }

    ///
    /// Borrows body instance
    ///
    pub fn borrow(&self) -> Ref<T> {
        self.body.borrow()
    }

    ///
    /// Borrows body as mutable
    ///
    pub fn borrow_mut(&self) -> RefMut<T> { self.body.borrow_mut() }

    ///
    /// Check if given element has any update
    ///
    pub fn check_update<U: ?Sized + Any>(&self, elem: &U) -> bool { todo!() }

    ///
    /// Commits updated value to owning entity base.
    ///
    pub fn commit<U: ?Sized + Any>(&self, elem: &U) -> bool { todo!() }

    ///
    /// Commits updated value to owning entity base.
    ///
    /// This will not touch host storage's update fence value,
    ///
    pub fn commit_silent<U: ?Sized + Any>(&self, elem: &U) -> bool { todo!() }

    ///
    /// Get reference to owning storage
    ///
    pub fn storage(&self) -> &Storage { &self.desc.storage }

    ///
    /// Gets index of given element
    ///
    fn _index_of<U: ?Sized + Any>(&self, elem: &U) -> Result<&PropDesc, ConfigSetError> {
        let offset = unsafe {
            let elem = elem as *const U as *const u8;
            let base = self as *const Self as *const u8;

            elem.offset_from(base)
        };

        let type_id = TypeId::of::<U>();
        if offset < 0 {
            return Err(ConfigSetError::InvalidMemberPointer { offset, type_id });
        }

        if let Some(node) = self.desc.offset_index_table.get(&(offset as usize)) {
            Ok(node)
        } else {
            Err(ConfigSetError::InvalidMemberPointer { offset, type_id })
        }
    }
}

impl<T: Clone> Clone for ConfigSet<T> {
    fn clone(&self) -> Self {
        Self {
            update_fence: self.update_fence,
            prop_check_fences: self.prop_check_fences.clone(),
            desc: self.desc.clone(),
            body: self.body.clone(),
        }
    }
}

#[cfg(test)]
mod test_concepts {
    use once_cell::sync::{Lazy, OnceCell};

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

        fn __entity_get_meta(&self) -> Arc<Vec<Metadata>> {
            static LAZY_TABLE: OnceCell<Arc<Vec<Metadata>>> = OnceCell::new();
            return LAZY_TABLE
                .get_or_init(|| {
                    let mut table = Vec::<Metadata>::with_capacity(3);
                    table.push(Metadata::create_base::<i32>("my_ivar".into(), Default::default()));
                    table[0].description = "hello".into();
                    table[0].hidden = true;


                    table.push(Metadata::create_base::<f32>("my_fvar".into(), Default::default()));
                    // ... Manipulate with these ...

                    table.push(Metadata::create_base::<f64>("my_dvar".into(), Default::default()));
                    Arc::new(table)
                })
                .clone();
        }

        fn __entity_update_value(&mut self, index: usize, value: &dyn EntityValue) {
            match index {
                0 => value.clone_to(&mut self.my_ivar),
                1 => value.clone_to(&mut self.my_fvar),
                _ => ()
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
    }

    #[test]
    fn test_pseudo_automation() {}
}
