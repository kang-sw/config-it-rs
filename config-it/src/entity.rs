use bitflags::bitflags;
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::common::ItemID;

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
pub struct Metadata {
    pub type_id: TypeId,

    pub props: MetadataProps,
    pub vtable: Box<dyn MetadataVTable>,
}

impl std::ops::Deref for Metadata {
    type Target = MetadataProps;

    fn deref(&self) -> &Self::Target {
        &self.props
    }
}

pub trait MetadataVTable: Send + Sync + 'static {
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
    fn validate(&self, value: &mut dyn Any) -> Option<bool>;
}

pub struct MetadataVTableImpl<T: 'static> {
    impl_copy: bool,
    fn_default: Cow<'static, fn() -> T>,
    fn_validate: Cow<'static, fn(&mut T) -> Option<bool>>,
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

    fn validate(&self, value: &mut dyn Any) -> Option<bool> {
        let value = value.downcast_mut::<T>().unwrap();
        (self.fn_validate)(value)
    }
}

bitflags! {
    #[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq)]
    pub struct MetaFlag: u32 {
        /// Disable import from `import` operation
        const NO_IMPORT = 1 << 0;

        /// Disable export to `export` operation
        const NO_EXPORT = 1 << 1;

        /// Hint monitor that this variable should be hidden from user.
        const HIDDEN = 1 << 2;

        /// Hint monitor that this variable should only be read by admin.
        const ADMIN_READ = 1 << 3;

        /// Hint monitor that this variable should only be written by admin.
        const ADMIN_WRITE = 1 << 4 | Self::ADMIN_READ.bits();

        /// Hint monitor that this is admin-only variable.
        const ADMIN = Self::ADMIN_READ.bits() | Self::ADMIN_WRITE.bits();

        /// Hint monitor that this variable is transient, and should not be saved to storage.
        const TRANSIENT = MetaFlag::NO_EXPORT.bits() | MetaFlag::NO_IMPORT.bits();
    }
}

/// Hint for backend editor. This is not used by config-it itself.
///
/// This is used by remote monitor to determine how to edit this variable.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataEditorHint {
    /// For color in range [0.0, 1.0]
    ///
    /// - [number; 3] -> RGB
    /// - [number; 4] -> RGBA
    ColorRgba255,

    /// For color in range [0, 255]
    ///
    /// - [number; 3] -> RGB
    /// - [number; 4] -> RGBA
    /// - string -> hex color
    /// - integer -> 32 bit hex color `[r,g,b,a] = [0,8,16,24].map(|x| 0xff & (color >> x))`
    ColorRgbaReal,

    /// Any string type will be treated as multiline text.
    MultilineText,

    /// Any string type will be treated as code, with given language hint.
    Code(Cow<'static, str>),
}

/// Shared generic properties of this metadata entity.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MetadataProps {
    /// Identifier for this config entity.
    pub name: &'static str,

    /// Typename for this config entity.
    pub type_name: Cow<'static, str>,

    ///
    pub flags: MetaFlag,

    /// Hint for monitoring editor. This is not directly used by this crate, but exists for hinting
    /// remote monitor how to edit this variable.
    pub editor_hint: Option<MetadataEditorHint>,

    /// Optional schema. Will be used by remote monitor to manage this variable.
    pub schema: Option<crate::Schema>,

    /// Source variable name. Usually same as 'name' unless another name is specified for it.
    pub varname: Cow<'static, str>,

    ///
    pub description: Cow<'static, str>,

    /// Environment variable name
    pub env: Option<Cow<'static, str>>,
}

pub mod lookups {
    pub trait HasSchema {
        fn get_schema(&self) -> Option<crate::Schema>;
    }

    impl<T: schemars::JsonSchema> HasSchema for T {
        fn get_schema(&self) -> Option<crate::Schema> {
            Some(schemars::schema_for!(T))
        }
    }

    pub trait NoSchema {
        fn get_schema(&self) -> Option<crate::Schema> {
            None
        }
    }

    trait AnyType {}
    impl<T> AnyType for T {}

    impl<T: AnyType> NoSchema for &T {}
}

impl Metadata {
    pub fn create_for_base_type<T>(init: MetadataVTableImpl<T>, props: MetadataProps) -> Self
    where
        T: EntityTrait + Clone + serde::de::DeserializeOwned + serde::ser::Serialize,
    {
        Self { type_id: TypeId::of::<T>(), props, vtable: Box::new(init) }
    }
}

/* ---------------------------------------- Entity Value ---------------------------------------- */
#[derive(Clone)]
pub enum EntityValue {
    Trivial(TrivialEntityValue),
    Complex(Arc<dyn EntityTrait>),
}

type ReinterpretInput<'a> = Result<&'a [usize], &'a mut [usize]>;
type ReinterpretOutput<'a> = Result<&'a dyn EntityTrait, &'a mut dyn EntityTrait>;

#[derive(Clone, Copy)]
pub struct TrivialEntityValue(for<'a> unsafe fn(ReinterpretInput) -> ReinterpretOutput, [usize; 2]);

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
        if std::mem::size_of::<T>() <= std::mem::size_of::<usize>() * 2 {
            let mut buffer = [0usize; 2];
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
pub struct EntityData {
    /// Unique entity id for program run-time
    id: ItemID,

    meta: Arc<Metadata>,
    version: AtomicUsize,
    value: Mutex<EntityValue>,

    hook: Arc<dyn EntityEventHook>,
}

#[derive(Debug, thiserror::Error)]
pub enum EntityUpdateError {
    #[error("Validation failed")]
    ValueValidationFailed,

    #[error("Deserialization failed")]
    DeserializeFailed(#[from] erased_serde::Error),
}

impl EntityData {
    pub(crate) fn new(meta: Arc<Metadata>, hook: Arc<dyn EntityEventHook>) -> Self {
        Self {
            id: ItemID::new_unique(),
            version: AtomicUsize::new(0),
            value: Mutex::new(meta.vtable.create_default()),
            meta,
            hook,
        }
    }

    pub fn get_id(&self) -> ItemID {
        self.id
    }

    pub fn get_meta(&self) -> &Arc<Metadata> {
        &self.meta
    }

    pub fn get_update_fence(&self) -> usize {
        self.version.load(Ordering::Relaxed)
    }

    pub fn get_value(&self) -> (&Arc<Metadata>, EntityValue) {
        (&self.meta, self.value.lock().clone())
    }

    /// If `silent` option is disabled, increase config set and source argument's fence
    ///  by 1, to make self and other instances of config set which shares the same core
    ///  be aware of this change.
    pub fn __apply_value(&self, value: EntityValue) {
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
    pub fn update_value_from<'a, T>(&self, de: T) -> Result<bool, EntityUpdateError>
    where
        T: serde::Deserializer<'a>,
    {
        let meta = &self.meta;
        let vt = &meta.vtable;
        let mut erased = <dyn erased_serde::Deserializer>::erase(de);

        match vt.deserialize(&mut erased) {
            Ok(mut built) => {
                let is_perfect = match vt.validate(built.as_any_mut()) {
                    Some(clean) => clean,
                    None => return Err(EntityUpdateError::ValueValidationFailed),
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

    pub fn __notify_value_change(&self, make_storage_dirty: bool) {
        self.hook.on_value_changed(self, !make_storage_dirty);
    }
}

pub(crate) trait EntityEventHook: Send + Sync {
    fn on_value_changed(&self, data: &EntityData, silent: bool);
}
