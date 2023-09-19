use bitflags::bitflags;
use serde::de::DeserializeOwned;
use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::common::ItemID;

///
/// Config entity type must satisfy this constraint
///
pub trait EntityTrait: Send + Sync + Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn as_serialize(&self) -> &dyn erased_serde::Serialize;
    fn deserialize(
        &mut self,
        de: &mut dyn erased_serde::Deserializer,
    ) -> Result<(), erased_serde::Error>;
}

impl<T> EntityTrait for T
where
    T: Send + Sync + Any + serde::Serialize + DeserializeOwned,
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
    /// Creates default value for this config entity.
    fn create_default(&self) -> Arc<dyn EntityTrait>;

    /// Create new deserialized entity instance from given deserializer
    fn create_from(
        &self,
        de: &mut dyn erased_serde::Deserializer,
    ) -> Result<Arc<dyn EntityTrait>, erased_serde::Error>;

    /// Copy one value from another. Panics when called with unmatched type!
    fn clone_in_place(&self, src: &dyn Any, dst: &mut dyn Any);

    /// Returns None if validation failed. Some(false) when source value was corrected.
    ///  Some(true) when value was correct.
    fn validate(&self, value: &mut dyn Any) -> Option<bool>;
}

pub struct MetadataVTableImpl<T> {
    _x: std::marker::PhantomData<T>,
}

impl<T: Send + Sync + 'static> MetadataVTable for MetadataVTableImpl<T> {
    fn create_default(&self) -> Arc<dyn EntityTrait> {
        todo!()
    }

    fn create_from(
        &self,
        de: &mut dyn erased_serde::Deserializer,
    ) -> Result<Arc<dyn EntityTrait>, erased_serde::Error> {
        todo!()
    }

    fn clone_in_place(&self, src: &dyn Any, dst: &mut dyn Any) {
        todo!()
    }

    fn validate(&self, value: &mut dyn Any) -> Option<bool> {
        todo!()
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
        Self { type_id: TypeId::of::<T>(), props, vtable: todo!() }
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
    id: ItemID,

    meta: Arc<Metadata>,
    fence: AtomicUsize,
    value: Mutex<Arc<dyn EntityTrait>>,

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
            fence: AtomicUsize::new(0),
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
    pub fn update_value_from<'a, T>(&self, de: T) -> Result<bool, EntityUpdateError>
    where
        T: serde::Deserializer<'a>,
    {
        let meta = &self.meta;
        let vt = &meta.vtable;
        let mut erased = <dyn erased_serde::Deserializer>::erase(de);
        let mut built = vt.create_default();
        let built_mut = Arc::get_mut(&mut built).unwrap();

        match built_mut.deserialize(&mut erased) {
            Ok(_) => {
                let clean = match vt.validate(built_mut.as_any_mut()) {
                    Some(clean) => clean,
                    None => return Err(EntityUpdateError::ValueValidationFailed),
                };

                let built: Arc<dyn EntityTrait> = built.into();
                self.__apply_value(built);

                Ok(clean)
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
