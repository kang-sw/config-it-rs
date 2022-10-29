use std::any::Any;
use std::sync::{Arc, RwLock};
use serde::de::DeserializeOwned;
use serde::Serialize;

///
///
/// Metadata for configuration entity. This can be used as template for multiple config entity
///  instances.
///
///
pub struct Metadata {
    name: Arc<str>,
    description: Arc<str>,
}

pub trait EntityValue: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
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
    data: RwLock<Arc<dyn EntityValue>>,
    meta: Arc<Metadata>,
}
