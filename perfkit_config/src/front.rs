/*
   1. Config Entity
   2. Config Set -> NOT a physical concept ... just user-defined struct of various entities!
   3. Config Storage -> Collection of entities. Classes are set of entities with prefix.
   4. Config Registry -> Collection of categories. File save/load root

   Config Entity    --> ConfigBase:1, Data:1:mut
   Config Set       --> ConfigStorage:1, ConfigEntity:N
   Config Storage   --> w:ConfigBase:N
   Config Base      --> Data:1:mut
 */
use std::any::Any;
use std::marker::PhantomData;
use std::ops::Deref;
use std::rc::Weak;
use std::sync::{Arc, Mutex, RwLock};
use crate::back::ConfigSetBackend;


///
///
/// Metadata for configuration entity. This can be used as template for multiple config entity
///  instances.
///
///
pub struct ConfigMetadata {
    name: String,
    description: String,
}

///
///
/// User may interact with this config entity.
///
#[derive(Clone)]
pub struct ConfigEntity<T> {
    __p0: PhantomData<T>,
    fence: u64,
    local_copy: Arc<T>,
    base: Arc<()>,
}

impl<T> ConfigEntity<T> {
    pub fn __test_create(val: T) -> ConfigEntity<T> {
        let s = ConfigEntity::<T> {
            __p0: PhantomData::default(),
            base: Arc::new(()),
            fence: 0,
            local_copy: Arc::new(val),
        };

        return s;
    }

    /// Commit config entity value changes for next category update.
    pub fn commit(&self, value: T) { unimplemented!(); }

    /// Commit config entity in-place.
    pub fn set(&self, value: T) { unimplemented!(); }

    /// Get reference to original data.
    /// > **warning** It may not
    pub fn refer(&self) -> &T { self.local_copy.deref() }

    /// Check if there's any active update. Dirty flag is consumed after invocation.
    /// - Fetches local copy from base
    pub fn consume_update(&mut self) -> bool { unimplemented!(); }

    /// Mark this config entity as dirty state.
    pub fn mark_dirty(&mut self) { unimplemented!(); }
}

mod __test {
    use std::any::Any;
    use std::ops::Deref;
    use std::sync::Arc;
    use crate::front::ConfigEntity;

    #[test]
    fn test_compilation() {
        let s;
        {
            let _r = ConfigEntity::<i32>::__test_create(3);
            let _v = 3;
            let _ = _r.clone();

            s = _r.refer();

            let _s = Arc::<i32>::new(3);
            let _g: Arc::<dyn Any> = _s.clone();
            let _f = || 3;

            assert_eq!(_g.deref().type_id(), _s.deref().type_id());
            assert_eq!(_f(), 3);
        }
    }
}

///
///
/// User should create registry to control over storages
///
pub struct ConfigRegistry {
    /// TODO: All registered entities

    /// TODO: Backend event listners
}

///
///
/// User has basic control over config category
///
pub trait ConfigStorage: Drop {
    /// Register this storage. If any other storage with same name exist, it'll wait for previous
    ///  instance is disposed, which may cause deadlock!


    /// Updates internal state.
    fn update(&mut self) -> bool;

    /// Install observer for external update (Backend, config reload, etc ...)
    fn observe_external_update(&mut self, bound: Weak<dyn Any>, handler: Box<&dyn FnMut()>);
}

///
///
/// User directly
///
pub trait ConfigSet {
    fn get_storage(&self) -> Arc<Mutex<dyn ConfigStorage>>;
}

///
///
/// Basic config object.
///
struct ConfigEntityBase {
    id: u64,
    data: RwLock<Arc<dyn Any>>,
    meta: Arc<ConfigMetadata>,
}
