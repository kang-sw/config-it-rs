use erased_serde::{Deserializer, Serialize, Serializer};
use serde::de::DeserializeOwned;
use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

///
/// Config entity type must satisfy this constraint
///
pub trait EntityTrait: Send + Sync + Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn serialize(&self, se: &mut dyn erased_serde::Serializer) -> Result<(), erased_serde::Error>;
    fn deserialize(
        &mut self,
        de: &mut dyn erased_serde::Deserializer,
    ) -> Result<(), erased_serde::Error>;
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

    fn serialize(&self, se: &mut dyn erased_serde::Serializer) -> Result<(), erased_serde::Error> {
        Serialize::erased_serialize(self, se).map(|_| ())
    }

    fn deserialize(
        &mut self,
        de: &mut dyn erased_serde::Deserializer,
    ) -> Result<(), erased_serde::Error> {
        *self = T::deserialize(de)?;
        Ok(())
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
    pub name: &'static str,
    pub type_id: TypeId,

    pub v_default: Arc<dyn EntityTrait>,
    pub v_min: Option<Box<dyn EntityTrait>>,
    pub v_max: Option<Box<dyn EntityTrait>>,
    pub v_one_of: Vec<Box<dyn EntityTrait>>,

    pub props: MetadataProps,

    pub fn_default: fn() -> Box<dyn EntityTrait>,
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

#[derive(Debug)]
pub struct MetadataProps {
    pub disable_export: bool,
    pub disable_import: bool,
    pub hidden: bool,

    /// Source variable name. Usually same as 'name' unless another name is specified for it.
    pub varname: &'static str,
    pub description: &'static str,
}

impl Metadata {
    pub fn create_for_base_type<T>(
        name: &'static str,
        init: MetadataValInit<T>,
        props: MetadataProps,
    ) -> Self
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
            name,
            type_id: TypeId::of::<T>(),
            v_default: Arc::new(init.v_default),
            v_min,
            v_max,
            v_one_of,
            props,
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
    value: Mutex<Arc<dyn EntityTrait>>,

    hook: Arc<dyn EntityEventHook>,
}

impl EntityData {
    pub(crate) fn new(meta: Arc<Metadata>, hook: Arc<dyn EntityEventHook>) -> Self {
        static ID_GEN: AtomicU64 = AtomicU64::new(0);

        Self {
            id: 1 + ID_GEN.fetch_add(1, Ordering::Relaxed),
            fence: AtomicUsize::new(0), // This forces initial
            value: Mutex::new(meta.v_default.clone()),
            meta,
            hook,
        }
    }

    pub fn get_id(&self) -> u64 {
        self.id
    }

    pub fn get_meta(&self) -> &Arc<Metadata> {
        &self.meta
    }

    pub fn get_update_fence(&self) -> usize {
        self.fence.load(Ordering::Relaxed)
    }

    pub fn get_value(&self) -> (&Arc<Metadata>, Arc<dyn EntityTrait>) {
        (&self.meta, self.value.lock().unwrap().clone())
    }

    /// If `silent` option is disabled, increase config set and source argument's fence
    ///  by 1, to make self and other instances of config set which shares the same core
    ///  be aware of this change.
    pub fn __apply_value(&self, value: Arc<dyn EntityTrait>) {
        debug_assert!(self.meta.type_id == value.as_any().type_id());

        {
            let mut lock = self.value.lock().unwrap();
            *lock = value;

            self.fence.fetch_add(1, Ordering::Release);
        }
    }

    ///
    /// Update config entity's central value by parsing given deserializer.
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Deserialization successful, validation successful.
    /// * `Ok(false)` - Deserialization successful, validation unsuccessful, as value was modified
    ///                 to satisfy validator constraint
    /// * `Err(_)` - Deserialization or validation has failed.
    ///
    pub fn update_value_from<'a, T>(&self, de: T) -> Result<bool, crate::core::Error>
    where
        T: serde::Deserializer<'a>,
    {
        let meta = &self.meta;
        let mut erased = <dyn erased_serde::Deserializer>::erase(de);
        let mut built = (meta.fn_default)();

        match built.deserialize(&mut erased) {
            Ok(_) => {
                let clean = match (meta.fn_validate)(&*meta, built.as_any_mut()) {
                    Some(clean) => clean,
                    None => return Err(crate::core::Error::ValueValidationFailed),
                };

                let built: Arc<dyn EntityTrait> = built.into();
                self.__apply_value(built);

                Ok(clean)
            }
            Err(e) => {
                log::error!(
                    "(Deserialization Failed) {}(var:{}) \n\nERROR: {e:#?}",
                    meta.name,
                    meta.props.varname,
                );
                Err(e.into())
            }
        }
    }

    pub fn __notify_value_change(&self, make_storage_dirty: bool) {
        self.hook.on_value_changed(self, !make_storage_dirty);
    }
}

pub(crate) trait EntityEventHook: Send + Sync {
    fn on_value_changed(&self, data: &EntityData, silent: bool);
}
