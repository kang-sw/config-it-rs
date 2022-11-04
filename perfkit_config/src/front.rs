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
use crate::__all::*;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

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
pub struct ConfigSet<T> {
    /// Description for this collection instance, which will be shared across
    /// collection instances.
    desc: Arc<CollectionDescriptor>,

    /// Used for filtering update targets. Only entities that has larger fence number
    /// than this can be updated. After update, this number will be set as largest value
    /// of all updated entities.
    update_fence: usize,
    
    /// 
    body: T,
}

struct CollectionDescriptor {
    /// Path to owning storage
    storage: Storage,

    /// Provides pointer offset -> index mapping
    offset_index_table: Arc<HashMap<usize, usize>>,

    /// Each field of config set's target struct will be mapped into this.
    config_base_set: Vec<Arc<EntityBase>>,
}

impl<T: ReflectCollection> ConfigSet<T> {
    // TODO: Create collection with storage
    fn create(s: Storage, prefix: impl Iterator<Item = &'static str>) {
        // 1. Create default T, retrieve offset-index mapping
    }

    /// Update enclosing contents iteratively.
    fn update(&self) -> bool {
        todo!()

        // 1. Submit update_fence value to storage, then storage returns
    }

    /// Check if given element
    fn check_update<U>(&self, elem: &U) -> bool {
        todo!()
    }
}

impl<T> Deref for ConfigSet<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.body
    }
}

#[cfg(test)]
mod test_concepts {
    use super::*;

    #[test]
    fn test_if_compiled() {}

    #[test]
    fn test_pseudo_automation() {}
}
