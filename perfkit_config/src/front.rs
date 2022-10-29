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
use std::cell::{Cell, RefCell};
use std::ops::Deref;
use std::sync::{Arc};
use crate::__all::*;


///
///
/// User may interact with this config entity.
///
#[derive(Clone, Default)]
pub struct Entity<T> {
    config_id: u64,
    fence: u64,
    cached: RefCell<Arc<T>>,
}

impl<T> Entity<T> {
    /// Commit config entity value changes for next category update.
    pub fn commit(&self, _value: T) { todo!(); }

    /// Commit config entity in-place.
    pub fn set(&self, _value: T) { todo!(); }

    /// Get pointer to value. Usefull when value need to be copied safely.
    pub fn get(&self) -> Arc<T> { todo!(); }
}

impl<T: Any + Send + Sync + Clone> EntityInstance for Entity<T> {
    fn upload_cache(&self, value: ValuePtr) {
        unsafe {
            assert_eq!(value.deref().type_id(), TypeId::of::<T>());

            let raw = Arc::into_raw(value);
            let raw = raw as *const T;
            let ptr = Arc::from_raw(raw);
            self.cached.replace(ptr);
        }
    }

    fn get_config_id(&self) -> u64 {
        self.config_id
    }
}

pub trait EntityInstance {
    fn upload_cache(&self, value: ValuePtr);
    fn get_config_id(&self) -> u64;
}

///
///
/// Basic behavior of configuration set.
///
///

pub trait EntityCollection {
    fn collect_init_info(&self, visitor: impl Fn(&dyn EntityInstance, Arc<Metadata>));
    fn component_at(&self, at: usize) -> &dyn EntityInstance;
}

#[cfg(test)]
mod example {
    use once_cell::sync::{OnceCell};
    use super::*;

    struct MyConfig {
        s0: Entity<i32>,
        s1: Entity<f32>,
        s2: Entity<f64>,
    }

    #[cfg(none)]
    impl EntityCollection for MyConfig {
        fn collect_init_info(&self, visitor: impl Fn(&dyn EntityInstance, Arc<Metadata>)) {
            static META_SET: [OnceCell<Arc<Metadata>>; 3] = [
                OnceCell::new(),
                OnceCell::new(),
                OnceCell::new(),
            ];

            let meta_arr = [
                META_SET[0].get_or_init(|| Arc::new(Metadata { name: "".into(), description: "".into() })),
                META_SET[1].get_or_init(|| Arc::new(Metadata { name: "".into(), description: "".into() })),
                META_SET[2].get_or_init(|| Arc::new(Metadata { name: "".into(), description: "".into() })),
            ];

            let inst_arr = [
                &self.s0 as &dyn EntityInstance,
                &self.s1 as &dyn EntityInstance,
                &self.s2 as &dyn EntityInstance,
            ];

            static META_VERIFY: OnceCell<()> = OnceCell::new();
            META_VERIFY.get_or_init(|| todo!("Check if there's name duplication"));

            inst_arr.iter().zip(meta_arr)
                .for_each(|(x1, x2)| visitor(x1.deref(), (*x2).clone()));
        }

        fn component_at(&self, at: usize) -> &dyn EntityInstance {
            match at {
                0 => &self.s0 as &dyn EntityInstance,
                1 => &self.s1 as &dyn EntityInstance,
                2 => &self.s2 as &dyn EntityInstance,
                _ => unimplemented!(),
            }
        }
    }
}

#[test]
fn momo() {
    let s = Entity::<i32> {
        cached: RefCell::new(Arc::new(0)),
        fence: 0,
        config_id: 0,
    };

    let f = Arc::new(0);
    s.upload_cache(f as ValuePtr);
}
