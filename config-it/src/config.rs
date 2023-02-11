use crate::entity::{EntityData, EntityTrait, Metadata};
use compact_str::CompactString;
use parking_lot::Mutex;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::iter::zip;
use std::mem::replace;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

///
/// Base trait that is automatically generated
///
pub trait Template: Default + Clone {
    /// Returns table mapping to <offset_from_base:property_metadata>
    fn prop_desc_table__() -> &'static HashMap<usize, PropData>;

    /// Get path of this config template (module path, struct name)
    fn struct_path() -> (&'static str, &'static str);

    /// Fill defaulted values
    fn fill_default(&mut self);

    /// Returns element at index as Any
    fn elem_at_mut__(&mut self, index: usize) -> &mut dyn Any;

    /// Convenient wrapper for element value update
    fn update_elem_at__(&mut self, index: usize, value: &dyn Any, meta: &Metadata) {
        let data = self.elem_at_mut__(index);
        (meta.fn_copy_to)(value, data);
    }
}

pub struct PropData {
    pub index: usize,
    pub type_id: TypeId,
    pub meta: Arc<Metadata>,
}

///
/// May storage implement this
///
pub struct GroupContext {
    pub group_id: u64,
    pub sources: Arc<Vec<EntityData>>,
    pub path: Arc<Vec<CompactString>>,
    pub(crate) source_update_fence: AtomicUsize,

    /// Broadcast subscriber to receive updates from backend.
    pub(crate) update_receiver_channel: async_broadcast::InactiveReceiver<()>,
}

///
/// Primary interface that end user may interact with
///
/// Wrap `ReflectData` derivative like `Group<MyData>`
///
pub struct Group<T> {
    /// Cached local content
    pub __body: T,

    /// Cached update fence
    fence: usize,

    /// Property-wise contexts
    local: Mutex<Vec<PropLocalContext>>,

    /// List of managed properties. This act as source container
    core: Arc<GroupContext>,

    /// Unregister hook anchor.
    ///
    /// It will unregister this config set from owner storage automatically, when all
    ///  instances of config set disposed.
    _unregister_hook: Arc<dyn Any + Send + Sync>,
}

impl<T: Clone> Clone for Group<T> {
    fn clone(&self) -> Self {
        Self {
            __body: self.__body.clone(),
            fence: self.fence.clone(),
            local: self.local.lock().clone().into(),
            core: self.core.clone(),
            _unregister_hook: self._unregister_hook.clone(),
        }
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Group<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Group")
            .field("__body", &self.__body)
            .field("fence", &self.fence)
            .finish()
    }
}

#[derive(Default, Clone)]
struct PropLocalContext {
    /// Locally cached update fence.
    update_fence: usize,

    /// Updated recently
    dirty_flag: bool,
}

impl<T: Template> Group<T> {
    pub(crate) fn create_with__(
        core: Arc<GroupContext>,
        unregister_anchor: Arc<dyn Any + Send + Sync>,
    ) -> Self {
        let mut gen = Self {
            core,
            __body: T::default(),
            fence: 0,
            local: vec![PropLocalContext::default(); T::prop_desc_table__().len()].into(),
            _unregister_hook: unregister_anchor,
        };

        gen.fill_default();
        gen
    }

    ///
    /// Update this storage
    ///
    pub fn update(&mut self) -> bool {
        // Perform quick check: Does update fence value changed?
        match self.core.source_update_fence.load(Ordering::Relaxed) {
            v if v == self.fence => return false,
            v => self.fence = v,
        }

        let mut local_ctx = self.local.lock();
        debug_assert_eq!(
            local_ctx.len(),
            self.core.sources.len(),
            "Logic Error: set was not correctly initialized!"
        );

        let mut has_update = false;

        for ((index, local), source) in
            zip(zip(0..local_ctx.len(), &mut *local_ctx), &*self.core.sources)
        {
            // Perform quick check to see if given config entity has any update.
            match source.get_update_fence() {
                v if v == local.update_fence => continue,
                v => local.update_fence = v,
            }

            has_update = true;
            local.dirty_flag = true;

            let (meta, value) = source.get_value();
            self.__body.update_elem_at__(index, value.as_any(), &*meta);
        }

        has_update
    }

    ///
    /// Check update from entity address
    ///
    pub fn check_elem_update<U: 'static>(&self, e: &U) -> bool {
        let Some(index) = self.get_index_by_ptr(e) else { return false };
        let ref_dirty_flag = &mut (*self.local.lock())[index].dirty_flag;

        replace(ref_dirty_flag, false)
    }

    ///
    /// Get index of element
    ///
    pub fn get_index_by_ptr<U: 'static>(&self, e: &U) -> Option<usize> {
        if let Some(prop) = self.get_prop_by_ptr(e) {
            Some(prop.index)
        } else {
            None
        }
    }

    pub fn get_prop_by_ptr<U: 'static>(&self, e: &U) -> Option<&PropData> {
        let ptr = e as *const _ as *const u8 as isize;
        let base = &self.__body as *const _ as *const u8 as isize;

        match ptr - base {
            v if v < 0 => None,
            v if v >= std::mem::size_of::<T>() as isize => None,
            v => {
                if let Some(prop) = T::prop_desc_table__().get(&(v as usize)) {
                    debug_assert_eq!(prop.type_id, TypeId::of::<U>());
                    debug_assert!(prop.index < self.local.lock().len());
                    Some(prop)
                } else {
                    None
                }
            }
        }
    }

    ///
    /// Commit entity value to storage (silently)
    ///
    pub fn commit_elem<U: Clone + EntityTrait + Send>(&self, e: &U, notify: bool) {
        // Create new value pointer from input argument.
        let cloned_value = Arc::new(e.clone()) as Arc<dyn EntityTrait>;

        // Replace source argument with created ptr
        let elem = &(*self.core.sources)[self.get_index_by_ptr(e).unwrap()];
        elem.__apply_value(cloned_value);
        elem.__notify_value_change(notify)
    }

    ///
    /// Get update receiver
    ///
    pub fn watch_update(&self) -> async_broadcast::Receiver<()> {
        self.core.update_receiver_channel.clone().activate()
    }

    ///
    /// Mark all elements dirty
    ///
    pub fn mark_all_elem_dirty(&self) {
        // Raising dirty flag does not incur remote reload.
        self.local
            .lock()
            .iter_mut()
            .for_each(|e| e.dirty_flag = true);
    }

    ///
    /// Mark this group dirty.
    ///
    pub fn mark_group_dirty(&mut self) {
        self.fence = 0;
    }

    ///
    /// Mark given element dirty.
    ///
    pub fn mark_dirty<U: 'static>(&self, elem: &U) {
        let index = self.get_index_by_ptr(elem).unwrap();
        self.local.lock()[index].dirty_flag = true;
    }

    ///
    /// Get generated metadata of given element
    ///
    pub fn get_metadata<U: 'static>(&self, elem: &U) -> Arc<Metadata> {
        self.get_prop_by_ptr(elem).unwrap().meta.clone()
    }

    ///
    /// Get allocated group prefix
    ///
    pub fn get_path(&self) -> Arc<Vec<CompactString>> {
        self.core.path.clone()
    }
}

impl<T> std::ops::Deref for Group<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.__body
    }
}

impl<T> std::ops::DerefMut for Group<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.__body
    }
}

#[test]
fn _verify_send_impl() {
    #[derive(Clone, Default)]
    struct Example {}
    impl Template for Example {
        fn prop_desc_table__() -> &'static HashMap<usize, PropData> {
            unimplemented!()
        }

        fn fill_default(&mut self) {
            unimplemented!()
        }

        fn elem_at_mut__(&mut self, _: usize) -> &mut dyn Any {
            unimplemented!()
        }

        fn struct_path() -> (&'static str, &'static str) {
            unimplemented!()
        }
    }

    fn _assert_send<T: Send + Sync>() {}
    _assert_send::<Group<Example>>();
}

#[cfg(any())]
mod emulate_generation {
    use futures::executor;
    use lazy_static::lazy_static;
    use std::thread;

    use super::*;
    use crate::*;

    #[derive(Default, Clone)]
    struct MyStruct {
        my_int: i32,
        my_string: String,
    }

    impl Template for MyStruct {
        fn prop_desc_table__() -> &'static HashMap<usize, PropData> {
            use entity::{MetadataProps, MetadataValInit};

            lazy_static! {
                static ref TABLE: Arc<HashMap<usize, PropData>> = {
                    let mut s = HashMap::new();

                    {
                        type Type = i32;

                        let offset = unsafe {
                            let owner = 0 as *const MyStruct;
                            &(*owner).my_int as *const _ as *const u8 as usize
                        };
                        let identifier = "#ident_as_string";
                        let varname = "#varname_or_ident";
                        let doc_string = "#doc_str";
                        let index = 1;
                        let default_value: Type = 13;

                        let init = MetadataValInit::<Type> {
                            fn_validate: |_, _| -> Option<bool> { Some(true) },
                            v_default: default_value,
                            v_one_of: Default::default(),
                            v_max: Default::default(),
                            v_min: Default::default(),
                        };

                        let props = MetadataProps {
                            description: doc_string,
                            varname,
                            disable_import: false,
                            disable_export: false,
                            hidden: false,
                        };

                        let meta = Metadata::create_for_base_type(identifier, init, props);

                        let prop_data = PropData {
                            index,
                            type_id: TypeId::of::<Type>(),
                            meta: Arc::new(meta),
                        };

                        s.insert(offset, prop_data);
                    }

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

        fn fill_default(&mut self) {}
    }

    #[test]
    fn try_compile() {
        println!("{}", std::env::var("MY_VAR").unwrap());
        let (st, work) = Storage::new();
        thread::spawn(move || futures::executor::block_on(work));

        let mut group: Group<MyStruct> =
            executor::block_on(st.create_group(["RootCategory".into()].to_vec())).unwrap();

        assert!(group.update());
        assert!(!group.update());
        assert!(group.check_elem_update(&group.my_string));
        assert!(!group.check_elem_update(&group.my_string));
    }
}
