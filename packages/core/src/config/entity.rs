use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::core::meta::MetadataProps;
use crate::core::ItemID;

/// Number of available words for trivial entity value.
///
/// Value `5` makes [`EntityData`] in 32-byte align (96 byte size).
const TRIVIAL_ENTITY_NUM_WORDS: usize = 5;

///
/// Config entity type must satisfy this constraint
///
pub trait EntityTrait: Send + Sync + Any + 'static {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn as_serialize(&self) -> &dyn erased_serde::Serialize;
    fn deserialize(
        &mut self,
        de: &mut dyn erased_serde::Deserializer,
    ) -> Result<(), erased_serde::Error>;
    fn duplicated(&self) -> Arc<dyn EntityTrait>;
}

impl<T> EntityTrait for T
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

    fn duplicated(&self) -> Arc<dyn EntityTrait> {
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
pub struct Metadata {
    pub type_id: TypeId,

    pub props: MetadataProps,
    pub vtable: &'static dyn MetadataVTable,
}

impl std::ops::Deref for Metadata {
    type Target = MetadataProps;

    fn deref(&self) -> &Self::Target {
        &self.props
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
pub struct MetadataVTableImpl<T: 'static> {
    impl_copy: bool,
    fn_default: fn() -> T,
    fn_validate: fn(&mut T) -> ValidationResult,
}

impl<T: EntityTrait + Clone> MetadataVTable for MetadataVTableImpl<T> {
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

#[doc(hidden)]
pub mod generic_lookup {
    trait AnyType {}
    impl<T> AnyType for T {}

    /* -------------------------------------- Lookup Schema ------------------------------------- */
    #[cfg(feature = "jsonschema")]
    pub trait HasSchema {
        fn get_schema(&self) -> Option<crate::Schema>;
    }

    #[cfg(feature = "jsonschema")]
    impl<T: schemars::JsonSchema> HasSchema for T {
        fn get_schema(&self) -> Option<crate::Schema> {
            Some(schemars::schema_for!(T))
        }
    }

    #[cfg(feature = "jsonschema")]
    pub trait NoSchema {
        fn get_schema(&self) -> Option<crate::Schema> {
            None
        }
    }

    #[cfg(feature = "jsonschema")]
    impl<T: AnyType> NoSchema for &T {}

    /* ------------------------------------- Detect If Copy ------------------------------------- */
    pub trait IsCopy {
        fn is_copy(&self) -> bool;
    }

    impl<T: Copy> IsCopy for T {
        fn is_copy(&self) -> bool {
            true
        }
    }

    pub trait IsNotCopy {
        fn is_copy(&self) -> bool;
    }

    impl<T: AnyType> IsNotCopy for &T {
        fn is_copy(&self) -> bool {
            false
        }
    }
}

impl Metadata {
    pub fn create_for_base_type<T>(
        init: &'static MetadataVTableImpl<T>,
        props: MetadataProps,
    ) -> Self
    where
        T: EntityTrait + Clone + serde::de::DeserializeOwned + serde::ser::Serialize,
    {
        Self { type_id: TypeId::of::<T>(), props, vtable: init }
    }
}

/* ---------------------------------------- Entity Value ---------------------------------------- */
#[derive(Clone)]
pub enum EntityValue {
    Trivial(TrivialEntityValue),
    Complex(Arc<dyn EntityTrait>),
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
type ReinterpretOutput<'a> = Result<&'a dyn EntityTrait, &'a mut dyn EntityTrait>;

/// Pair of function pointer to retrieve entity trait from given pointer and actual payload.
#[derive(Clone, Copy)]
#[doc(hidden)]
pub struct TrivialEntityValue(
    for<'a> unsafe fn(ReinterpretInput) -> ReinterpretOutput,
    [usize; TRIVIAL_ENTITY_NUM_WORDS],
);

impl EntityTrait for EntityValue {
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

    fn duplicated(&self) -> Arc<dyn EntityTrait> {
        self.as_entity().duplicated()
    }
}

impl EntityValue {
    pub fn as_entity(&self) -> &dyn EntityTrait {
        match self {
            EntityValue::Trivial(t) => unsafe { (t.0)(Ok(&t.1)).unwrap_unchecked() },
            EntityValue::Complex(v) => v.as_ref(),
        }
    }

    pub fn as_entity_mut(&mut self) -> &mut dyn EntityTrait {
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

    pub fn from_trivial<T: Copy + EntityTrait>(value: T) -> Self {
        // SAFETY: This is safe as long as `T` is trivially copyable.
        unsafe { Self::from_trivial_unchecked(value) }
    }

    pub unsafe fn from_trivial_unchecked<T: EntityTrait>(value: T) -> Self {
        if std::mem::size_of::<T>() <= std::mem::size_of::<usize>() * TRIVIAL_ENTITY_NUM_WORDS {
            let mut buffer = [0usize; TRIVIAL_ENTITY_NUM_WORDS];
            unsafe {
                std::ptr::copy_nonoverlapping(&value, buffer.as_mut_ptr() as _, 1);
            }

            unsafe fn retrieve_function<T: EntityTrait>(i: ReinterpretInput) -> ReinterpretOutput {
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

    pub fn from_complex<T: EntityTrait>(value: T) -> Self {
        Self::Complex(Arc::new(value))
    }

    pub(crate) unsafe fn from_value<T: EntityTrait>(value: T, implements_copy: bool) -> Self {
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
    id: ItemID,

    meta: &'static Metadata,
    version: AtomicU64,
    value: Mutex<EntityValue>,

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
    pub(crate) fn new(meta: &'static Metadata, hook: Arc<dyn EntityEventHook>) -> Self {
        Self {
            id: ItemID::new_unique(),
            version: AtomicU64::new(0),
            value: Mutex::new(meta.vtable.create_default()),
            meta,
            hook,
        }
    }

    pub fn get_id(&self) -> ItemID {
        self.id
    }

    pub fn get_meta(&self) -> &'static Metadata {
        self.meta
    }

    pub fn get_version(&self) -> u64 {
        self.version.load(Ordering::Relaxed)
    }

    pub fn get_value(&self) -> (&'static Metadata, EntityValue) {
        (self.meta, self.value.lock().clone())
    }

    /// If `silent` option is disabled, increase config set and source argument's fence
    ///  by 1, to make self and other instances of config set which shares the same core
    ///  be aware of this change.
    pub(crate) fn __apply_value(&self, value: EntityValue) {
        debug_assert!(self.meta.type_id == value.as_any().type_id());

        *self.value.lock() = value;
        self.version.fetch_add(1, Ordering::Release);
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
            Err(e) => {
                log::error!(
                    "(Deserialization Failed) {}(var:{}) \n\nERROR: {e:#?}",
                    meta.props.name,
                    meta.props.varname,
                );
                Err(e.into())
            }
        }
    }

    pub(crate) fn __notify_value_change(&self, make_storage_dirty: bool) {
        self.hook.on_value_changed(self, !make_storage_dirty);
    }
}

pub(crate) trait EntityEventHook: Send + Sync {
    fn on_value_changed(&self, data: &EntityData, silent: bool);
}
