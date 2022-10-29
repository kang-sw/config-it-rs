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

///
/// Represents entity type.
pub trait EntityValue {
    //
}

impl<T: Any + Serialize + DeserializeOwned> EntityValue for T {
    //
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
