use std::any::Any;
use std::sync::{Arc, RwLock};
use serde::de::DeserializeOwned;
use serde::Serialize;
use crate::__all::JsonObject;

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
}

pub trait EntityValue: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;

    fn clone_deep(&self) -> ValuePtr { todo!() }
    fn load_from(&mut self, raw: &JsonObject) { todo!() }
    fn validate(&mut self, meta: Metadata) -> bool { todo!() }
}

impl<T: Any + Send + Sync + Serialize + DeserializeOwned> EntityValue for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

///
///
/// Basic config object.
///
pub struct EntityBase {
    unique_id: u64,
    data: RwLock<ValuePtr>,
    meta: Arc<Metadata>,
}
