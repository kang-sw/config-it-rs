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

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use smol_str::SmolStr;
use crate::entity::{EntityData, Metadata};

///
///
/// Base trait that is automatically generated
///
pub trait CollectPropMeta: Default + Clone {
    /// Returns table mapping to <offset_from_base:property_metadata>
    fn impl_prop_desc_table__() -> Arc<HashMap<usize, PropData>>;
}

pub struct PropData {
    index: usize,
    meta: Arc<Metadata>,
}

///
/// May storage implement this
///
pub struct SetCoreContext {
    register_id: u64,
    sources: Vec<Arc<EntityData>>,
    source_update_fence: AtomicUsize,
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
}

#[derive(Default, Clone)]
struct PropLocalContext {
    /// Locally cached update fence.
    update_fence: u64,
}

impl<T: CollectPropMeta> Set<T> {
    pub(crate) fn create_with__(core: Arc<SetCoreContext>) -> Self {
        Self {
            core,
            body: T::default(),
            local: vec![PropLocalContext::default(); T::impl_prop_desc_table__().len()],
        }
    }

    // TODO: Check update from entity address
    // TODO: Commit (silently) entity address
}



