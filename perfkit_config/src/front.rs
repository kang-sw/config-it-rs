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
#[derive(Clone)]
pub struct Entity<T> {
    base: Arc<()>,
    fence: u64,
    cached: RefCell<Arc<T>>,
}

impl<T> Entity<T> {
    /// Commit config entity value changes for next category update.
    pub fn commit(&self, _value: T) { todo!(); }

    /// Commit config entity in-place.
    pub fn set(&self, _value: T) { todo!(); }

    /// Get reference to value. Usefull when value need to be copied safely.
    pub fn get(&self) -> Arc<T> { todo!(); }
}

impl<T: Any + Send + Sync + Clone> UploadCache for Entity<T> {
    fn upload_cache(&self, value: Arc<dyn EntityValue>) {
        unsafe {
            assert_eq!(value.deref().type_id(), TypeId::of::<T>());

            let raw = Arc::into_raw(value);
            let raw = raw as *const T;
            let ptr = Arc::from_raw(raw);
            self.cached.replace(ptr);
        }
    }
}

pub(super) trait UploadCache {
    fn upload_cache(&self, value: Arc<dyn EntityValue>);
}

#[test]
fn momo() {
    let s = Entity::<i32> {
        cached: RefCell::new(Arc::new(0)),
        fence: 0,
        base: Arc::new(()),
    };

    let f = Arc::new(0);
    s.upload_cache(f as Arc<dyn EntityValue>);
}



