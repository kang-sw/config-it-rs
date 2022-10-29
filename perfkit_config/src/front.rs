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
//! ...
//!
//! let cfg = MySet::create(&storage, "name"
//! ```
//!
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync;
use std::sync::{Arc};
use crate::__all::*;


///
///
/// User may interact with this config entity.
///
#[derive(Clone)]
pub struct Entity<T> {
    fence: u64,
    local_copy: Arc<T>,
    base: sync::Arc<EntityBase>,
}

impl<T> Entity<T> {
    pub fn __test_create(val: T) -> Entity<T> {
        let s = Entity::<T> {
            base: unimplemented!(),
            fence: 0,
            local_copy: Arc::new(val),
        };

        return s;
    }

    /// Commit config entity value changes for next category update.
    pub fn commit(&self, _value: T) { unimplemented!(); }

    /// Commit config entity in-place.
    pub fn set(&self, _value: T) { unimplemented!(); }

    /// Get reference to original data.
    /// > **warning** It may not
    pub fn refer(&self) -> &T { self.local_copy.deref() }

    /// Check if there's any active update. Dirty flag is consumed after invocation.
    /// - Fetches local copy from base
    pub fn fetch(&mut self) -> bool { unimplemented!(); }

    /// Mark this config entity as dirty state.
    pub fn mark_dirty(&mut self) { unimplemented!(); }
}

mod __test {
    use std::any::Any;
    use std::ops::Deref;
    use std::sync::Arc;
    use crate::front::Entity;

    #[test]
    fn test_compilation() {
        let _s;
        {
            let _r = Entity::<i32>::__test_create(3);
            let _v = 3;
            let _ = _r.clone();

            _s = _r.refer();

            let _s = Arc::<i32>::new(3);
            let _g: Arc::<dyn Any> = _s.clone();
            let _f = || 3;

            assert_eq!(_g.deref().type_id(), _s.deref().type_id());
            assert_eq!(_f(), 3);
        }
    }
}


