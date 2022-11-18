use std::any::{Any};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize};
use erased_serde::{Deserializer, Serialize, Serializer};

///
///
/// Metadata for configuration entity. This can be used as template for multiple config entity
///  instances.
///
///
pub struct Metadata {
    pub name: String,
    pub description: String,

    pub v_default: Box<dyn Any>,
    pub v_min: Option<Box<dyn Any>>,
    pub v_max: Option<Box<dyn Any>>,
    pub v_one_of: Vec<Box<dyn Any>>,

    pub env_var_name: Option<String>,
    pub disable_write: bool,
    pub disable_read: bool,
    pub hidden: bool,

    pub fn_default: fn() -> Box<dyn Any>,
    pub fn_copy_to: fn(&dyn Any, &mut dyn Any),
    pub fn_serialize_to: fn(&dyn Any, &mut dyn Serializer) -> Result<(), erased_serde::Error>,
    pub fn_deserialize_from: fn(&mut dyn Any, &mut dyn Deserializer) -> Result<(), erased_serde::Error>,

    /// Returns None if validation failed. Some(false) when source value was corrected.
    ///  Some(true) when value was correct.
    pub fn_validate: fn(&Metadata, &mut dyn Any) -> Option<bool>,
}

pub struct MetadataValInit<T> {
    pub v_default: T,
    pub v_min: Option<T>,
    pub v_max: Option<T>,
    pub v_one_of: Vec<T>,

    // Should be generated through proc macro
    pub fn_validate: fn(&Metadata, &mut dyn Any) -> Option<bool>,
}

impl Metadata {
    pub fn create_for_base_type<T>(name: String, init: MetadataValInit<T>) -> Self
        where T: Any + Default + Clone + serde::de::DeserializeOwned + serde::ser::Serialize,
    {
        let s: &dyn Any = &init.v_default;
        let retrive_opt_minmax =
            |val| {
                if let Some(v) = val {
                    Some(Box::new(v) as Box<dyn Any>)
                } else {
                    None
                }
            };

        let v_min = retrive_opt_minmax(init.v_min);
        let v_max = retrive_opt_minmax(init.v_max);
        let v_one_of: Vec<_> = init.v_one_of.iter().map(|v| Box::new(v.clone()) as Box<dyn Any>).collect();

        Self {
            name,
            description: Default::default(),
            v_default: Box::new(init.v_default),
            v_min,
            v_max,
            v_one_of,
            env_var_name: Default::default(),
            disable_write: false,
            disable_read: false,
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

    hook: Box<dyn EntityEventHook>,
    meta: Arc<Metadata>,
    fence: AtomicUsize,
    value: Mutex<Arc<dyn Any>>,
}

impl EntityData {
    // TODO: get_value() -> EntityValuePtr
    // TODO: try_commit(value_ptr) -> bool; validates type / etc
    // TODO: try_commit_silent(value_ptr) -> bool; same as above / does not trigger on_commit event.
}

pub(crate) trait EntityEventHook {
    fn on_committed(&self, data: &Arc<EntityData>);
    fn on_value_changed(&self, data: &Arc<EntityData>);
}