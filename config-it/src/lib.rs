//!
//! A crate for asynchronous centralized configuration management.
//!
//! # Usage
//!
//! You can define 'Template' which defines set of grouped properties,
//!  which should be updated at once. Only `config_it` decorated properties will be counted
//!  as part of given group and will be managed.
//!
//! `Template` must implement `Clone` trait.
//!
//! You should create `Storage` to create config group instances(`Group<T:Template>`).
//!  `Storage` is the primary actor for centralized configuration management. Config groups can
//!  be instantiated based on storage instance.
//!
//! Any property implements `serde::Serialize`, `serde::DeserializeOwned` can be used as
//! configuration property.
//!
//! # Attributes
//!
//! Any field decorated with attribute `#[config_it]` or `#[config]` will be treated as
//! configuration property. You can specify additional constraints for the property by
//! adding additional attributes inside parenthesis.
//!
//! - `default = <value>`
//!     - Specify default value for the property. as the <value> expression is converted into
//!       field type using `.try_into().unwrap()` expression, you should specify un-fallible
//!       expression here. This is to support convenient value elevation from `&str` to `String`,
//!       or similar owning conversions.
//! - `default_expr = "<expr>"`
//!     - Specify complicated value expression here. To specify string literal here, you have
//!       to escape double quotes(`"`) with backslash(`\`). As this attribute converts given
//!       expression into token tree directly, you can write any valid rust expression here.
//! - `alias = "<alias>"`
//!     - Specify alias name for the property. This is useful when you want to use different
//!       name for the property in config file, but want to use original name in code.
//! - `one_of(<value>, <value>, ...)`
//!     - Specify set of allowed values for the property. This is useful when you want to
//!       restrict the value of the property to specific set of values. You can also specify
//!       default value as one of the allowed values.
//!     - Default value can be out of the allowed set, and can be excluded from the allowed set.
//!       In this case, setting value back to default value will not be allowed.
//! - `min = <value>`, `max=<value>`
//!     - Constrain the value of the property to be within given range. Any type which implements
//!       `Ord` can have min/max constraints.
//! - `env = "<env_var>"`
//!     - Specify environment variable name to import value from. If the environment variable
//!       is not set, the default value will be used. `TryParse` trait is used to convert
//!       environment variable value into property type.
//! - `no_import`
//!     - Do not update value from imported archive. This is useful when mixed with `env` flag,
//!       which will keep its value as imported environment variable even after the archive is
//!       imported.   
//! - `no_export`
//!     - Do not export value to archive.
//! - `transient`
//!     - Value won't be archived, and won't be imported from archive.
//! - `hidden`
//!     - Hints to monitoring system that this property should not be visible.
//!
//! Any field that are not marked with attribute `#[config_it]` or `#[config]` will not be
//! treated as configuration property, however, to construct a valid `Template`, all fields
//! must be config-default-constructive.
//!
//! To specify default value for the field, you can use `#[nocfg = "<value_expr>"]` attribute.
//! This uses same rule with `default_expr` attribute, and you can use any valid rust expression
//! here.
//!
//! # Usage
//!
//! ## Creating config template
//! ```
//! use config_it::Template;
//!
//! // Every config template must implement `Clone` trait.
//! #[derive(Template, Clone)]
//! struct Profile {
//!     /// Doc comment will be used as description for the property. This will be included in
//!     /// the config property's metadata.
//!     #[config]
//!     pub name: String,
//!
//!     #[config(max = 250)]
//!     pub age: u32,
//!
//!     #[config(default = "unspecified", one_of("left", "right", "up", "down"))]
//!     pub position: String,
//! }
//!
//! // Before doing anything with your config template, you should create storage instance.
//! // Storage is the primary actor for centralized configuration management.
//! let (storage, runner) = config_it::create_storage();
//!
//! // To run the storage, you should spawn a task with runner (the second return value)
//! std::thread::spawn(move || futures::executor::block_on(runner));
//!
//! // Assume that you have a config file with following content:
//! // (Note that all 'group's are prefixed with '~'(this is configurable) to be distinguished
//! //  from properties)
//! let content = serde_json::json!({
//!     "~profile": {
//!         "~scorch": {
//!             "name": "Scorch",
//!             "age": 25,
//!             "position": "left"
//!         },
//!         "~john": {
//!             "name": "John",
//!             "age": 30,
//!             "position": "invalid-value-here"
//!         }
//!     }
//! });
//!
//! let archive = serde_json::from_value(content).unwrap();
//!
//! // It is recommended to manipulate config group within async context.
//! futures::executor::block_on(async {
//!     // You can import config file into storage.
//!     // NOTE: storage is thread-safe handle to the inner storage actor, as you can freely
//!     //       clone it and send it to use it from multiple different threads.
//!     storage.import(archive, Default::default()).await.unwrap();
//!
//!     // As the import operation simply transmits request to the actor, you should wait
//!     // for the actual import job to be done.
//!     storage.fence().await;
//!
//!     // A `Template` can be instantiated as `Group<T:Template>` type.
//!     let mut scorch = storage
//!         .create::<Profile>(["profile", "scorch"])
//!         .await
//!         .unwrap();
//!     let mut john = storage
//!         .create::<Profile>(["profile", "john"])
//!         .await
//!         .unwrap();
//!
//!     // Before calling 'update' method on group, every property remain in default.
//!     assert_eq!(scorch.name, "");
//!     assert_eq!(scorch.age, 0);
//!     assert_eq!(scorch.position, "unspecified");
//!
//!     // Calling 'update' method will update the property to the value in archive.
//!     assert!(scorch.update() == true);
//!     assert!(john.update() == true);
//!
//!     // You can check dirty flag of individual property.
//!     assert!(scorch.consume_update(&scorch.name) == true);
//!     assert!(scorch.consume_update(&scorch.name) == false);
//!
//!     // Now the property values are updated.
//!     assert_eq!(scorch.name, "Scorch");
//!     assert_eq!(scorch.age, 25);
//!     assert_eq!(scorch.position, "left");
//!     assert_eq!(john.name, "John");
//!     assert_eq!(john.age, 30);
//!     assert_eq!(john.position, "unspecified", "invalid value is ignored");
//!
//!     storage.close().unwrap();
//! });
//! ```
//!
//! ## Config property with serde
//!
//! Any type implements `Clone`, `serde::Serialize` and `serde::Deserialize` can be used as
//! config property. Default trait can be omitted if you provide default value for the property.
//!
//! ```
//! #[derive(config_it::Template, Clone)]
//! struct Outer {
//!     #[config(default_expr = "Inner{name:Default::default(),age:0}")]
//!     inner: Inner,
//! }
//!
//! #[derive(serde::Serialize, serde::Deserialize, Clone)]
//! struct Inner {
//!     name: String,
//!     age: u32,
//! }
//!
//! let (storage, runner) = config_it::create_storage();
//! let task = async {
//!     let mut outer = storage.create::<Outer>(["outer"]).await.unwrap();
//!     outer.inner.name = "John".to_owned();
//!     outer.inner.age = 30;
//!
//!     // You can feedback local change to the storage using `commit_elem`
//!     outer.commit_elem(&outer.inner, false);
//!
//!     // You can retrieve the archive from storage using `export`
//!     let archive = storage.export(Default::default()).await.unwrap();
//!
//!     let dump = serde_json::to_string(&archive).unwrap();
//!     assert_eq!(dump, r#"{"~outer":{"inner":{"age":30,"name":"John"}}}"#);
//!
//!     storage.close().unwrap();
//! };
//!
//! futures::executor::block_on(async {
//!     futures::join!(runner, task);
//! });
//! ```
//!
pub mod beacon;
pub mod core;

// Just re-exported, for compatibility.
pub extern crate serde;

// Just re-exported, for any case if you want to use it.
pub use compact_str::CompactString;

// [`core::archive::Archive`] utilizes this.
pub use serde_json::Value as ArchiveValue;

#[cfg(feature = "jsonschema")]
pub extern crate schemars;

#[cfg(feature = "jsonschema")]
pub use schemars::schema::{RootSchema as Schema, SchemaObject};

#[cfg(feature = "beacon")]
pub use beacon_export::*;

#[cfg(feature = "beacon")]
mod beacon_export {
    use crate::beacon::*;
    use crate::core::*;

    #[doc(hidden)]
    pub use memoffset::offset_of;

    /// Primary macro for defining configuration group template.
    #[cfg(feature = "beacon")]
    pub use macros::Template;

    pub use entity::{Validation, ValidationResult};

    pub use group::{Group, Template};
    pub use storage::Storage;

    pub type BroadcastReceiver = noti::Receiver;
    pub type ArchiveCategoryRule<'a> = archive::CategoryRule<'a>;

    /// Shorthand macro for consuming multiple updates.
    ///
    /// With bracket syntax, this macro returns array of bool values, which indicates whether each
    /// elements has been updated. With simple variadic syntax, this macro returns boolean value which
    /// indicates whether any of given elements has been updated.
    ///
    /// If elements are wrapped inside parenthesis, this macro returns temporary struct which has
    /// boolean fields names each elements.
    ///
    /// In both cases, all supplied arguments will be evaluated.
    #[macro_export]
    macro_rules! consume_update{
        ($group: expr, [$($elems:ident),*]) => {
            {
                let __group = ($group).__macro_as_mut();
                [$(__group.consume_update(&__group.$elems)), *]
            }
        };

        ($group: expr, (($($elems:ident),*))) => {
            {
                #[derive(Debug, Clone, Copy)]
                struct Updates {
                    $($elems: bool),*
                }

                let __group = ($group).__macro_as_mut();
                Updates {
                    $($elems: __group.consume_update(&__group.$elems)),*
                }
            }
        };

        ($group: expr, $($elems:ident),*) => {
            {
                let __group = ($group).__macro_as_mut();
                $(__group.consume_update(&__group.$elems)) | *
            }
        };
    }

    /// Shorthand macro for marking multiple elements dirty.
    #[macro_export]
    macro_rules! mark_dirty{
    ($group: expr, $($elems:ident),+) => {
            {
                let __group = ($group).__macro_as_mut();
                $(__group.mark_dirty(&__group.$elems)); *
            }
        }
    }

    /// Shorthand macro for committing multiple elements.
    #[macro_export]
    macro_rules! commit_elem{
        ($group: expr, ($($elems:ident),+)) => {
            {
                let __group = ($group).__macro_as_mut();
                $(__group.commit_elem(&__group.$elems, false)); *
            }
        };

    ($group: expr, notify($($elems:ident),+)) => {
            {
                let __group = ($group).__macro_as_mut();
                $(__group.commit_elem(&__group.$elems, true)); *
            }
        };
    }
}
