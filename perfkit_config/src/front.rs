/*
   1. Config Entity
   2. Config Subset -> NOT a physical concept ... just user-defined struct of various entities!
   3. Config Class -> Collection of entities. Classes are set of entities with prefix.
   4. Config Registry -> Collection of categories. File save/load root
 */
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::{Arc, Mutex};

///
///
///
///

///
///
/// User may interact with this config entity.
///
#[derive(Clone)]
pub struct ConfigEntity<T> {
    __p0: PhantomData<T>,
    fence: u64,
    local_copy: Arc<T>,
    base: Arc<Mutex<()>>,
}


impl<T> ConfigEntity<T> {
    pub fn __test_create(val: T) -> ConfigEntity<T> {
        let s = ConfigEntity::<T> {
            __p0: PhantomData::default(),
            base: Arc::new(Mutex::new(())),
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
    pub fn refer(&self) -> &T { self.local_copy.deref() }

    /// Check if there's any active update.
    ///
    /// # returns
    /// Clears
    pub fn consume_update(&mut self) -> bool { unimplemented!(); }

    /// Mark this config entity as dirty state.
    pub fn mark_dirty(&mut self) { unimplemented!(); }
}

mod __test {
    use crate::front::ConfigEntity;

    #[test]
    fn test_compilation() {
        let s;
        {
            let _r = ConfigEntity::<i32>::__test_create(3);
            let _v = 3;
            let _ = _r.clone();

            s = _r.refer();
        }
    }
}

///
///
/// User has basic control over config category
///
pub trait ConfigSetBehavior {
    /// Updates
    fn update(&mut self) -> bool;
}


