use serde::de::DeserializeOwned;
use serde::Serialize;
use std::any::{Any};
use std::borrow::Borrow;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

pub type EntityValuePtr = Arc<dyn EntityValue>;

///
///
/// Metadata for configuration entity. This can be used as template for multiple config entity
///  instances.
///
///
pub struct Metadata {
    pub name: String,
    pub description: String,

    pub v_default: EntityValuePtr,
    pub v_min: Option<EntityValuePtr>,
    pub v_max: Option<EntityValuePtr>,
    pub v_one_of: Vec<EntityValuePtr>,

    pub env_var_name: Option<String>,
    pub disable_write: bool,
    pub disable_read: bool,
    pub hidden: bool,

    pub create_empty: Box<dyn Fn() -> Box<dyn EntityValue>>,
}

pub struct MetadataValInit<T> {
    pub v_default: T,
    pub v_min: Option<T>,
    pub v_max: Option<T>,
    pub v_one_of: Vec<T>,
}

impl Metadata {
    pub fn create_base<T>(name: String, init: MetadataValInit<T>) -> Self
        where T: EntityValue + Default + Clone,
    {
        let s: &dyn EntityValue = &init.v_default;
        let retrive_opt_minmax =
            |val| {
                if let Some(v) = val {
                    Some(Arc::new(v) as Arc<dyn EntityValue>)
                } else {
                    None
                }
            };

        let v_min = retrive_opt_minmax(init.v_min);
        let v_max = retrive_opt_minmax(init.v_max);
        let v_one_of: Vec<_> = init.v_one_of.iter().map(|v| Arc::new(v.clone()) as Arc<dyn EntityValue>).collect();

        Self {
            name,
            description: Default::default(),
            v_default: Arc::new(init.v_default),
            v_min,
            v_max,
            v_one_of,
            env_var_name: Default::default(),
            disable_write: false,
            disable_read: false,
            hidden: false,
            create_empty: Box::new(|| Box::new(T::default())),
        }
    }
}

///
///
/// Every field of config set must satisfy `EntityValue` trait
///
pub trait EntityValue: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;

    // TODO: deserialize_from(),
    // TODO: serialize_into(),
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
    value: Mutex<EntityValuePtr>,
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