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
