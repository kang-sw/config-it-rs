//! Primary control flow is:
//!
//! Usage example:
//!
//! ``` ignore
//! #[derive(ConfigDataReflect)]
//! struct MyConfigData {
//!   #[perfkit(one_of(3,4,5,6,7)
//!   value1: i32,
//!
//!   #[perfkit(min=2, max=5)]
//!   value2: float,
//! }
//!
//! impl Default for ConfigData {
//!   fn default() -> Self {
//!     Self {
//!       value1: 0,
//!       value2: 34f32
//!     }
//!   }
//! }
//!
//! fn my_code() {
//!
//! }
//! ```

use crate::entity::{EntityData, Metadata};
use std::any::Any;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

///
///
/// Base trait that is automatically generated
///
pub trait CollectPropMeta: Default + Clone {
    /// Returns table mapping to <offset_from_base:property_metadata>
    fn impl_prop_desc_table__() -> Arc<HashMap<usize, PropData>>;

    /// Returns element at index as Any
    fn elem_at_mut__(&mut self, index: usize) -> &mut dyn Any;

    /// Convenient wrapper for element value update
    fn update_elem_at__(&mut self, index: usize, value: &dyn Any, meta: &Metadata) {
        let mut data = self.elem_at_mut__(index);
        (meta.fn_copy_to)(value, data);
    }
}

pub struct PropData {
    index: usize,
    meta: Arc<Metadata>,
}

///
/// May storage implement this
///
pub(crate) struct SetCoreContext {
    pub(crate) register_id: u64,
    pub(crate) sources: Vec<Arc<EntityData>>,
    pub(crate) source_update_fence: AtomicU64,

    /// Broadcast subscriber to receive updates from backend.
    pub(crate) update_receiver_channel: async_mutex::Mutex<async_broadcast::Receiver<()>>,
}

///
///
/// Primary interface that end user may interact with
///
/// Wrap `ReflectData` derivative like `Set<MyData>`
///
#[derive(Clone)]
pub struct Set<T> {
    /// Cached local content
    body: T,

    /// Collects each property context.
    local: Vec<PropLocalContext>,

    /// List of managed properties. This act as source container
    core: Arc<SetCoreContext>,

    /// Unregistration hook anchor.
    ///
    /// It will unregister this config set from owner storage automatically, when all
    ///  instances of config set disposed.
    _unregister_hook: Arc<dyn Any>,
}

#[derive(Default, Clone)]
struct PropLocalContext {
    /// Locally cached update fence.
    update_fence: u64,
}

impl<T: CollectPropMeta> Set<T> {
    pub(crate) fn create_with__(core: Arc<SetCoreContext>, hook: Arc<dyn Any>) -> Self {
        Self {
            core,
            body: T::default(),
            local: vec![PropLocalContext::default(); T::impl_prop_desc_table__().len()],
            _unregister_hook: hook,
        }
    }

    // TODO: Check update from entity address
    pub fn check_update<U>(&mut self, e: &mut U) {}

    // TODO: Commit (silently) entity address

    // TODO: Get update receiver
}

#[cfg(test)]
mod tests {
    use lazy_static::lazy_static;

    use super::*;

    #[derive(Default, Clone)]
    struct MyStruct {
        my_int: i32,
        my_string: String,
    }

    impl CollectPropMeta for MyStruct {
        fn impl_prop_desc_table__() -> Arc<HashMap<usize, PropData>> {
            lazy_static! {
                static ref TABLE: Arc<HashMap<usize, PropData>> = { Default::default() };
            }
            const R: u32 = 3;

            TABLE.clone()
        }

        fn elem_at_mut__(&mut self, index: usize) -> &mut dyn Any {
            match index {
                0 => &mut self.my_int,
                1 => &mut self.my_string,
                _ => panic!(),
            }
        }
    }

    #[test]
    fn try_compile() {}
}
