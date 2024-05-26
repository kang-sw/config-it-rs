use parking_lot::RwLock;
use serde::de::DeserializeOwned;
use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::shared::meta::Metadata;
use crate::shared::ItemId;

/// Number of available words for trivial entity value.
///
/// Value `5` makes [`EntityData`] in 32-byte align (96 byte size).
const TRIVIAL_ENTITY_NUM_WORDS: usize = 5;

///
/// Config entity type must satisfy this constraint
///
pub trait Entity: Send + Sync + Any + 'static {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn as_serialize(&self) -> &dyn erased_serde::Serialize;
    fn deserialize(
        &mut self,
        de: &mut dyn erased_serde::Deserializer,
    ) -> Result<(), erased_serde::Error>;
    fn duplicated(&self) -> Arc<dyn Entity>;
}

impl<T> Entity for T
where
    T: Send + Sync + Any + serde::Serialize + DeserializeOwned + Clone + 'static,
{
    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self as &mut dyn Any
    }

    fn deserialize(
        &mut self,
        de: &mut dyn erased_serde::Deserializer,
    ) -> Result<(), erased_serde::Error> {
        T::deserialize_in_place(de, self)?;
        Ok(())
    }

    fn as_serialize(&self) -> &dyn erased_serde::Serialize {
        self as &dyn erased_serde::Serialize
    }

    fn duplicated(&self) -> Arc<dyn Entity> {
        Arc::new(self.clone())
    }
}

///
///
/// Metadata for configuration entity. This can be used as template for multiple config entity
///  instances.
///
///
#[derive(Debug)]
pub struct PropertyInfo {
    pub(crate) type_id: TypeId,
    pub(crate) index: usize,
    pub(crate) metadata: Metadata,
    pub(crate) vtable: &'static dyn MetadataVTable,
}

impl PropertyInfo {
    #[doc(hidden)]
    pub fn new(
        type_id: TypeId,
        index: usize,
        metadata: Metadata,
        vtable: &'static dyn MetadataVTable,
    ) -> Self {
        Self { type_id, index, metadata, vtable }
    }
}

impl std::ops::Deref for PropertyInfo {
    type Target = Metadata;

    fn deref(&self) -> &Self::Target {
        &self.metadata
    }
}

/// Validation result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validation {
    /// Data was valid. No change was made.
    Valid,

    /// Data was valid. Value was modified to satisfy validator constraint.
    Modified,
}

/// Validation result. Error type is plain string to inform user.
pub type ValidationResult = Result<Validation, std::borrow::Cow<'static, str>>;

/// Signature of function to validate config entity
pub type ValidateFn<T> = fn(&mut T) -> ValidationResult;

pub trait MetadataVTable: Send + Sync + 'static + std::fmt::Debug {
    /// Does implement `Copy`?
    fn implements_copy(&self) -> bool;

    /// Creates default value for this config entity.
    fn create_default(&self) -> EntityValue;

    /// Create new deserialized entity instance from given deserializer
    fn deserialize(
        &self,
        de: &mut dyn erased_serde::Deserializer,
    ) -> Result<EntityValue, erased_serde::Error>;

    /// Copy one value from another. Panics when called with unmatched type!
    fn clone_in_place(&self, src: &dyn Any, dst: &mut dyn Any);

    /// Returns None if validation failed. Some(false) when source value was corrected.
    ///  Some(true) when value was correct.
    fn validate(&self, value: &mut dyn Any) -> ValidationResult;
}

#[derive(cs::Debug)]
#[doc(hidden)]
pub struct MetadataVTableImpl<T: 'static> {
    pub impl_copy: bool,
    pub fn_default: fn() -> T,
    pub fn_validate: ValidateFn<T>,
}

impl<T: Entity + Clone> MetadataVTable for MetadataVTableImpl<T> {
    fn implements_copy(&self) -> bool {
        self.impl_copy
    }

    fn create_default(&self) -> EntityValue {
        // SAFETY: We know that `vtable.implements_copy()` is strictly managed.
        unsafe { EntityValue::from_value((self.fn_default)(), self.impl_copy) }
    }

    fn deserialize(
        &self,
        de: &mut dyn erased_serde::Deserializer,
    ) -> Result<EntityValue, erased_serde::Error> {
        let mut default = (self.fn_default)();
        default.deserialize(de)?;

        // SAFETY: We know that `vtable.implements_copy()` is strictly managed.
        Ok(unsafe { EntityValue::from_value(default, self.impl_copy) })
    }

    fn clone_in_place(&self, src: &dyn Any, dst: &mut dyn Any) {
        let src = src.downcast_ref::<T>().unwrap();
        let dst = dst.downcast_mut::<T>().unwrap();

        dst.clone_from(src);
    }

    fn validate(&self, value: &mut dyn Any) -> ValidationResult {
        let value = value.downcast_mut::<T>().unwrap();
        (self.fn_validate)(value)
    }
}

/* ---------------------------------------- Entity Value ---------------------------------------- */
#[derive(Clone)]
pub enum EntityValue {
    Trivial(TrivialEntityValue),
    Complex(Arc<dyn Entity>),
}

impl std::fmt::Debug for EntityValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trivial(_) => f.debug_struct("Trivial"),
            Self::Complex(_) => f.debug_struct("Complex"),
        }
        .finish()
    }
}

type ReinterpretInput<'a> = Result<&'a [usize], &'a mut [usize]>;
type ReinterpretOutput<'a> = Result<&'a dyn Entity, &'a mut dyn Entity>;

/// Pair of function pointer to retrieve entity trait from given pointer and actual payload.
#[derive(Clone, Copy)]
#[doc(hidden)]
pub struct TrivialEntityValue(
    for<'a> unsafe fn(ReinterpretInput) -> ReinterpretOutput,
    [usize; TRIVIAL_ENTITY_NUM_WORDS],
);

impl Entity for EntityValue {
    fn as_any(&self) -> &dyn Any {
        self.as_entity().as_any()
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self.as_entity_mut().as_any_mut()
    }

    fn as_serialize(&self) -> &dyn erased_serde::Serialize {
        self.as_entity().as_serialize()
    }

    fn deserialize(
        &mut self,
        de: &mut dyn erased_serde::Deserializer,
    ) -> Result<(), erased_serde::Error> {
        self.as_entity_mut().deserialize(de)
    }

    fn duplicated(&self) -> Arc<dyn Entity> {
        self.as_entity().duplicated()
    }
}

impl EntityValue {
    pub fn as_entity(&self) -> &dyn Entity {
        match self {
            EntityValue::Trivial(t) => unsafe { (t.0)(Ok(&t.1)).unwrap_unchecked() },
            EntityValue::Complex(v) => v.as_ref(),
        }
    }

    pub fn as_entity_mut(&mut self) -> &mut dyn Entity {
        match self {
            EntityValue::Trivial(t) => unsafe { (t.0)(Err(&mut t.1)).unwrap_err_unchecked() },
            EntityValue::Complex(v) => {
                if Arc::strong_count(v) == 1 {
                    Arc::get_mut(v).unwrap()
                } else {
                    *v = v.duplicated();
                    Arc::get_mut(v).unwrap()
                }
            }
        }
    }

    pub fn from_trivial<T: Copy + Entity>(value: T) -> Self {
        // SAFETY: This is safe as long as `T` is trivially copyable.
        unsafe { Self::from_trivial_unchecked(value) }
    }

    #[doc(hidden)]
    pub(crate) unsafe fn from_trivial_unchecked<T: Entity>(value: T) -> Self {
        if std::mem::size_of::<T>() <= std::mem::size_of::<usize>() * TRIVIAL_ENTITY_NUM_WORDS {
            let mut buffer = [0usize; TRIVIAL_ENTITY_NUM_WORDS];

            // SAFETY: This is safe as long as `T` is trivially copyable.
            unsafe {
                std::ptr::copy_nonoverlapping(&value, buffer.as_mut_ptr() as _, 1);
            }

            // SAFETY: Won't be used outside of delivered context.
            unsafe fn retrieve_function<T: Entity>(i: ReinterpretInput) -> ReinterpretOutput {
                match i {
                    Ok(x) => Ok(&*(x.as_ptr() as *const T)),
                    Err(x) => Err(&mut *(x.as_mut_ptr() as *mut T)),
                }
            }

            Self::Trivial(TrivialEntityValue(retrieve_function::<T>, buffer))
        } else {
            Self::from_complex(value)
        }
    }

    pub fn from_complex<T: Entity>(value: T) -> Self {
        Self::Complex(Arc::new(value))
    }

    pub(crate) unsafe fn from_value<T: Entity>(value: T, implements_copy: bool) -> Self {
        if implements_copy {
            // SAFETY: `implements_copy` must be managed very carefully to make this safe.
            unsafe { Self::from_trivial_unchecked(value) }
        } else {
            Self::from_complex(value)
        }
    }
}

/* ---------------------------------------------------------------------------------------------- */
/*                                           ENTITY DATA                                          */
/* ---------------------------------------------------------------------------------------------- */

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
#[derive(cs::Debug)]
pub struct EntityData {
    /// Unique entity id for program run-time
    pub id: ItemId,
    pub meta: &'static PropertyInfo,

    version: AtomicU64,
    value: RwLock<EntityValue>,

    #[debug(skip)]
    hook: Arc<dyn EntityEventHook>,
}

#[derive(Debug, thiserror::Error)]
pub enum EntityUpdateError {
    #[error("Validation failed: {0}")]
    ValueValidationFailed(Cow<'static, str>),

    #[error("Deserialization failed")]
    DeserializeFailed(#[from] erased_serde::Error),
}

impl EntityData {
    pub(crate) fn new(
        property_info: &'static PropertyInfo,
        hook: Arc<dyn EntityEventHook>,
    ) -> Self {
        Self {
            id: ItemId::new_unique_incremental(),
            version: AtomicU64::new(0),
            value: RwLock::new(property_info.vtable.create_default()),
            meta: property_info,
            hook,
        }
    }

    pub(crate) fn version(&self) -> u64 {
        self.version.load(Ordering::Relaxed)
    }

    pub(crate) fn property_value(&self) -> (&'static PropertyInfo, EntityValue) {
        (self.meta, self.value.read().clone())
    }

    /// Serialize this property into given serializer.
    pub fn serialize_into<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&self.value.read().as_serialize(), ser)
    }

    /// If `silent` option is disabled, increase config set and source argument's fence
    ///  by 1, to make self and other instances of config set which shares the same core
    ///  be aware of this change.
    pub(crate) fn __apply_value(&self, value: EntityValue) {
        debug_assert!(self.meta.type_id == value.as_any().type_id());

        *self.value.write() = value;
        self.version.fetch_add(1, Ordering::Release);
    }

    /// Attempts to update the central value of a config entity by deserializing the provided input.
    ///
    /// This function first deserializes the input to the expected data structure. After successful
    /// deserialization, it validates the value to ensure it conforms to the expected constraints.
    /// The method offers three potential outcomes:
    ///
    /// 1. Successful deserialization and validation: the value is perfectly valid and requires no
    ///    alterations.
    /// 2. Successful deserialization but failed validation: the value needed adjustments to meet
    ///    the validator's constraints.
    /// 3. Failed deserialization or validation: an error occurred during the process.
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Both deserialization and validation were successful without the need for any
    ///   modifications.
    /// * `Ok(false)` - Deserialization succeeded, but the value was adjusted during validation to
    ///   meet constraints.
    /// * `Err(_)` - Either the deserialization process or validation failed.
    ///
    /// # Type Parameters
    ///
    /// * `T`: Represents the type of the deserializer.
    ///
    /// # Parameters
    ///
    /// * `de`: An instance of the deserializer used to update the central value.
    pub fn update_value_from<'a, T>(&self, de: T) -> Result<Validation, EntityUpdateError>
    where
        T: serde::Deserializer<'a>,
    {
        let meta = &self.meta;
        let vt = &meta.vtable;
        let mut erased = <dyn erased_serde::Deserializer>::erase(de);

        match vt.deserialize(&mut erased) {
            Ok(mut built) => {
                let is_perfect = match vt.validate(built.as_any_mut()) {
                    Ok(clean) => clean,
                    Err(e) => return Err(EntityUpdateError::ValueValidationFailed(e)),
                };

                self.__apply_value(built);
                Ok(is_perfect)
            }
            Err(error) => {
                tr::debug!(
                    %error,
                    name = meta.varname,
                    r#type = meta.type_name,
                    "(Deserialization Failed)",
                );
                Err(error.into())
            }
        }
    }

    /// Notifies the underlying storage that a field within this group has been updated.
    ///
    /// The `touch` method serves as a mechanism to propagate changes to the appropriate parts of
    /// the system. Depending on the `make_storage_dirty` flag:
    ///
    /// - If set to `true`, the notification of change will be broadcasted to all group instances
    ///   that share the same group context, ensuring synchronization across shared contexts.
    ///
    /// - If set to `false`, only the monitor will be notified of the value update, without
    ///   affecting other group instances.
    ///
    /// # Arguments
    ///
    /// * `make_storage_dirty`: A boolean flag that determines the scope of the update notification.
    ///   When set to `true`, it affects all group instances sharing the same context. When `false`,
    ///   only the monitor is alerted of the change.
    pub fn touch(&self, make_storage_dirty: bool) {
        self.hook.on_value_changed(self, !make_storage_dirty);
    }
}

pub(crate) trait EntityEventHook: Send + Sync {
    fn on_value_changed(&self, data: &EntityData, silent: bool);
}
