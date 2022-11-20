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
use futures::executor;
use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::iter::zip;
use std::mem::replace;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

///
///
/// Base trait that is automatically generated
///
pub trait CollectPropMeta: Default + Clone {
    /// Returns table mapping to <offset_from_base:property_metadata>
    fn prop_desc_table__() -> &'static HashMap<usize, PropData>;

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
    type_id: TypeId,
    meta: Arc<Metadata>,
}

///
/// May storage implement this
///
pub(crate) struct SetCoreContext {
    pub(crate) register_id: u64,
    pub(crate) sources: Arc<Vec<EntityData>>,
    pub(crate) source_update_fence: AtomicUsize,

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

    /// Cached update fence
    fence: usize,

    /// Collects each property context.
    local: RefCell<Vec<PropLocalContext>>,

    /// List of managed properties. This act as source container
    core: Arc<SetCoreContext>,

    /// Unregister hook anchor.
    ///
    /// It will unregister this config set from owner storage automatically, when all
    ///  instances of config set disposed.
    _unregister_hook: Arc<dyn Any>,
}

#[derive(Default, Clone)]
struct PropLocalContext {
    /// Locally cached update fence.
    update_fence: usize,

    /// Updated recently
    dirty_flag: bool,
}

impl<T: CollectPropMeta> Set<T> {
    pub(crate) fn create_with__(core: Arc<SetCoreContext>, hook: Arc<dyn Any>) -> Self {
        Self {
            core,
            body: T::default(),
            fence: 0,
            local: RefCell::new(vec![PropLocalContext::default(); T::prop_desc_table__().len()]),
            _unregister_hook: hook,
        }
    }

    ///
    /// Update this storage
    ///
    pub async fn update_async(&mut self) -> bool {
        // Perform quick check: Does update fence value changed?
        match self.core.source_update_fence.load(Ordering::Relaxed) {
            v if v == self.fence => return false,
            v => self.fence = v,
        }

        debug_assert_eq!(
            self.local.borrow().len(),
            self.core.sources.len(),
            "Logic Error: set was not correctly initialized!"
        );

        let mut has_update = false;

        for ((index, local), source) in zip(
            zip(0..self.local.borrow().len(), &mut *self.local.borrow_mut()),
            &*self.core.sources,
        ) {
            // Perform quick check to see if given config entity has any update.
            match source.update_fence() {
                v if v == local.update_fence => continue,
                v => local.update_fence = v,
            }

            has_update = true;
            local.dirty_flag = true;

            source
                .with_values(|meta, value_any| {
                    self.body.update_elem_at__(index, &*value_any, &*meta)
                })
                .await;
        }

        has_update
    }

    ///
    /// Synchronously call update_async
    ///
    pub fn update(&mut self) -> bool {
        executor::block_on(self.update_async())
    }

    ///
    /// Check update from entity address
    ///
    pub fn check_elem_update<U: 'static>(&self, e: &U) -> bool {
        let Some(index) = self.get_index(e) else { return false };
        let ref_dirty_flag = &mut (*self.local.borrow_mut())[index].dirty_flag;

        replace(ref_dirty_flag, false)
    }

    ///
    /// Get index of element
    ///
    pub fn get_index<U: 'static>(&self, e: &U) -> Option<usize> {
        let ptr = e as *const _ as *const u8 as isize;
        let base = &self.body as *const _ as *const u8 as isize;

        match ptr - base {
            v if v < 0 => None,
            v if v >= std::mem::size_of::<T>() as isize => None,
            v => {
                if let Some(prop) = T::prop_desc_table__().get(&(v as usize)) {
                    debug_assert_eq!(prop.type_id, TypeId::of::<U>());
                    debug_assert!(prop.index < self.local.borrow().len());
                    Some(prop.index)
                } else {
                    None
                }
            }
        }
    }

    ///
    /// Commit entity value to storage (silently)
    ///
    pub fn commit_elem<U>(&self, e: &U, notify: bool) {
        // Create new value pointer from input argument.

        // Replace source argument with created ptr

        // If `notify` option is active, increase config set and source argument's fence
        //  by 1, to make self and other instances of config set which shares the same core
        //  be aware of this change.
        todo!()
    }

    ///
    /// Get update receiver
    ///
    pub async fn subscribe_update(&self) -> async_broadcast::Receiver<()> {
        self.core.update_receiver_channel.lock().await.clone()
    }
}

impl<T> std::ops::Deref for Set<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.body
    }
}

impl<T> std::ops::DerefMut for Set<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.body
    }
}

#[cfg(test)]
mod emulate_generation {
    use lazy_static::lazy_static;
    use std::thread;

    use super::*;
    use crate::entity::MetadataValInit;
    use crate::Storage;

    #[derive(Default, Clone)]
    struct MyStruct {
        my_int: i32,
        my_string: String,
    }

    impl CollectPropMeta for MyStruct {
        fn prop_desc_table__() -> &'static HashMap<usize, PropData> {
            lazy_static! {
                static ref TABLE: Arc<HashMap<usize, PropData>> = {
                    let mut s = HashMap::new();

                    let init = MetadataValInit::<i32> {
                        fn_validate: |meta, target| -> Option<bool> { todo!() },
                        v_default: 13,
                        v_one_of: Default::default(),
                        v_max: Default::default(),
                        v_min: Default::default(),
                    };

                    let mut meta = Metadata::create_for_base_type("hello".into(), init);
                    meta.name = "override-if-exist".into();
                    meta.description = "Docstring may placed here".into();
                    meta.hidden = false;
                    meta.disable_import = false;
                    meta.disable_export = false;

                    s.insert(
                        0usize,
                        PropData {
                            index: 0,
                            type_id: TypeId::of::<i32>(),
                            meta: Arc::new(meta),
                        },
                    );

                    Arc::new(s)
                };
            }

            &*TABLE
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
    fn try_compile() {
        println!("{}", std::env::var("MY_VAR").unwrap());
        let (st, work) = Storage::new();
        thread::spawn(move || futures::executor::block_on(work));

        let mut set: Set<MyStruct> = st.create_set("RootCategory".into(), Default::default());

        assert!(set.update());
        assert!(!set.update());
        assert!(set.check_elem_update(&set.my_string));
        assert!(!set.check_elem_update(&set.my_string));
    }
}
