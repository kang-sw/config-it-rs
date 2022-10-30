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
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc};
use crate::__all::*;


pub trait EntityInstance {
    fn upload_cache(&self, value: ValuePtr);
    fn get_config_id(&self) -> u64;
}


///
///
///
///
pub trait ReflectCollection {
    fn __entity_get_meta(&self, at: usize) -> Arc<Metadata> {
        todo!()
    }
}


///
///
/// User will use defined config class as template parameter of `EntityCollection`
///
pub struct EntityCollection<T> {
    ctx: CollectionContext,
    body: T,
}

struct CollectionContext {
    /// Provides pointer offset -> index mapping
    offset_index_table: Arc<HashMap<usize, usize>>,
}

impl<T: ReflectCollection> EntityCollection<T> {
    // TODO: Create collection with storage
    fn create(s: Storage, prefix: impl Iterator<Item=&str>) {
        // 1. Create default T, retrieve offset-index mapping

        // 2. Retrieve metadata for each entities

        // 3.
    }

    /// Update enclosing contents iteratively.
    ///
    /// 1. Check if
    fn update(&self) {}
}


impl<T> Deref for EntityCollection<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.body
    }
}

#[cfg(test)]
mod test_concepts {
    use super::*;
}


