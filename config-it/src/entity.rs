use async_mutex::Mutex;
use erased_serde::{Deserializer, Serialize, Serializer};
use serde::de::DeserializeOwned;
use std::any::Any;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

///
/// Config entity type must satisfy this constraint
///
pub trait EntityTrait: Send + Sync + Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T> EntityTrait for T
where
    T: Send + Sync + Any + Serialize + DeserializeOwned,
{
    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self as &mut dyn Any
    }
}

///
///
/// Metadata for configuration entity. This can be used as template for multiple config entity
///  instances.
///
///
pub struct Metadata {
    /// Identifier for this config entity.
    pub name: String,

    /// Source variable name. Usually same as 'name' unless another name is specified for it.
    pub varname: String,
    pub description: String,

    pub v_default: Box<dyn EntityTrait>,
    pub v_min: Option<Box<dyn EntityTrait>>,
    pub v_max: Option<Box<dyn EntityTrait>>,
    pub v_one_of: Vec<Box<dyn EntityTrait>>,

    pub disable_export: bool,
    pub disable_import: bool,
    pub hidden: bool,

    pub fn_default: fn() -> Box<dyn Any>,
    pub fn_copy_to: fn(&dyn Any, &mut dyn Any),
    pub fn_serialize_to: fn(&dyn Any, &mut dyn Serializer) -> Result<(), erased_serde::Error>,
    pub fn_deserialize_from:
        fn(&mut dyn Any, &mut dyn Deserializer) -> Result<(), erased_serde::Error>,

    /// Returns None if validation failed. Some(false) when source value was corrected.
    ///  Some(true) when value was correct.
    pub fn_validate: fn(&Metadata, &mut dyn Any) -> Option<bool>,
}

pub struct MetadataValInit<T> {
    pub v_default: T,

    pub v_min: Option<T>,
    pub v_max: Option<T>,
    pub v_one_of: Vec<T>,

    // Should be generated through derive macro
    pub fn_validate: fn(&Metadata, &mut dyn Any) -> Option<bool>,
}

impl Metadata {
    pub fn create_for_base_type<T>(name: String, init: MetadataValInit<T>) -> Self
    where
        T: EntityTrait + Default + Clone + serde::de::DeserializeOwned + serde::ser::Serialize,
    {
        let retrive_opt_minmax = |val| {
            if let Some(v) = val {
                Some(Box::new(v) as Box<dyn EntityTrait>)
            } else {
                None
            }
        };

        let v_min = retrive_opt_minmax(init.v_min);
        let v_max = retrive_opt_minmax(init.v_max);
        let v_one_of: Vec<_> = init
            .v_one_of
            .iter()
            .map(|v| Box::new(v.clone()) as Box<dyn EntityTrait>)
            .collect();

        Self {
            varname: name.clone(),
            name,
            description: Default::default(),
            v_default: Box::new(init.v_default),
            v_min,
            v_max,
            v_one_of,
            disable_export: false,
            disable_import: false,
            hidden: false,
            fn_default: || Box::new(T::default()),
            fn_copy_to: |from, to| {
                let from: &T = from.downcast_ref().unwrap();
                let to: &mut T = to.downcast_mut().unwrap();

                *to = from.clone();
            },
            fn_serialize_to: |from, to| {
                let from: &T = from.downcast_ref().unwrap();
                from.erased_serialize(to)?;

                Ok(())
            },
            fn_deserialize_from: |to, from| {
                let to: &mut T = to.downcast_mut().unwrap();
                *to = erased_serde::deserialize(from)?;

                Ok(())
            },
            fn_validate: init.fn_validate,
        }
    }
}

///
/// Helper methods for proc macro generated code
///
pub mod gen_helper {
    use std::any::Any;

    use crate::entity::Metadata;

    pub fn validate_min_max<T: 'static + Clone + Ord>(meta: &Metadata, val: &mut dyn Any) -> bool {
        let to: &mut T = val.downcast_mut().unwrap();
        let mut was_in_range = true;

        if let Some(val) = &meta.v_min {
            let from: &T = val.as_any().downcast_ref().unwrap();
            if *to < *from {
                was_in_range = false;
                *to = from.clone();
            }
        }

        if let Some(val) = &meta.v_max {
            let from: &T = val.as_any().downcast_ref().unwrap();
            if *from < *to {
                was_in_range = false;
                *to = from.clone();
            }
        }

        was_in_range
    }

    pub fn verify_one_of<T: 'static + Eq>(meta: &Metadata, val: &dyn Any) -> bool {
        if meta.v_one_of.is_empty() {
            return true;
        }

        let to: &T = val.downcast_ref().unwrap();
        meta.v_one_of
            .iter()
            .map(|v| v.as_any().downcast_ref::<T>().unwrap())
            .any(|v| *v == *to)
    }
}

///
///
/// Events are two directional ...
///
/// 1. Remote commit entity / Local commit entity
///     - 'on update' user observer hooked
///     - 'any value update' observer hooked
///
/// 2. Local commit entity silent
///     - 'any value update' observer hooked
///
/// 3. Local set retrieves entity update
///
pub struct EntityData {
    /// Unique entity id for program run-time
    id: u64,

    meta: Arc<Metadata>,
    fence: AtomicUsize,
    value: Mutex<Arc<dyn Any>>,

    hook: Box<dyn EntityEventHook>,
}

impl EntityData {
    pub fn update_fence(&self) -> usize {
        self.fence.load(Ordering::Relaxed)
    }

    pub async fn with_values(&self, pred: impl FnOnce(&Arc<Metadata>, Arc<dyn Any>)) {
        pred(&self.meta, self.value.lock().await.clone());
    }

    // TODO: get_value() -> EntityValuePtr
    // TODO: try_commit(value_ptr) -> bool; validates type / etc
    // TODO: try_commit_silent(value_ptr) -> bool; same as above / does not trigger on_commit event.
}

pub(crate) trait EntityEventHook {
    fn on_committed(&self, data: &Arc<EntityData>);

    fn on_value_changed(&self, data: &Arc<EntityData>);
}
