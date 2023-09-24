pub mod entity;
pub mod group;
pub mod noti;
pub mod storage;

/// Macro helper
#[doc(hidden)]
pub mod __lookup {
    #[doc(hidden)]
    pub fn __default_ref_ptr<T>() -> &'static T {
        // SAFETY: We won't do anything with this reference.
        unsafe {
            #[allow(deref_nullptr)]
            &*std::ptr::null()
        }
    }

    trait AnyType {}
    impl<T> AnyType for T {}

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
}
