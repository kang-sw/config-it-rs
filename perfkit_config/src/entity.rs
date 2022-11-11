use serde::de::DeserializeOwned;
use serde::Serialize;
use std::any::{Any, TypeId};
use std::borrow::Borrow;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicUsize;

pub type ValuePtr = Arc<dyn EntityValue>;

///
///
/// Metadata for configuration entity. This can be used as template for multiple config entity
///  instances.
///
///
pub struct Metadata {
    pub name: String,
    pub description: String,

    pub v_default: ValuePtr,
    pub v_min: Option<ValuePtr>,
    pub v_max: Option<ValuePtr>,
    pub v_one_of: Vec<ValuePtr>,

    pub env_var_name: Option<String>,
    pub disable_write: bool,
    pub disable_read: bool,
    pub hidden: bool,
}

impl Metadata {
    pub fn create_base<T>(name: String, init_val: T) -> Self
        where
            T: EntityValue,
    {
        Self {
            name,
            description: Default::default(),
            v_default: Arc::new(init_val),
            v_min: Default::default(),
            v_max: Default::default(),
            v_one_of: Default::default(),
            env_var_name: Default::default(),
            disable_write: false,
            disable_read: false,
            hidden: false,
        }
    }
}

pub trait EntityValue: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;

    fn clone_to(&self, s: &mut dyn Any);
    fn validate(&mut self, meta: Metadata) -> bool {
        todo!()
    }

    fn entity_deserialize(&mut self, desrl: &dyn erased_serde::Deserializer) {
        unimplemented!()
    }
    fn entity_serialize(&self) -> &dyn erased_serde::Serialize {
        unimplemented!()
    }
}

impl<T> EntityValue for T
    where
        T: Any + Send + Sync + Serialize + DeserializeOwned + Clone,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_to(&self, s: &mut dyn Any) {
        if let Some(target) = s.downcast_mut::<T>() {
            self.clone_into(target);
        }
    }
}

///
///
/// Basic config object.
///
pub struct EntityBase {
    pub unique_id: u64,
    pub fence: AtomicUsize,
    pub meta: Arc<Metadata>,
    data: Mutex<ValuePtr>,
}

impl EntityBase {
    pub fn create<T>(meta: Arc<Metadata>) -> Arc<EntityBase> {
        todo!()
    }
    pub fn get_cached_data(&self) -> ValuePtr { self.data.lock().unwrap().clone() }
}
